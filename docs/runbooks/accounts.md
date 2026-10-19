# Accounts and cloud services (Prompt 61)

Local editing, files, exports, CLI, MCP, agents, Git and self-hosted sync never call these services or
read cloud credentials. `@vellum/billing` names four Pro capabilities: `cloud_sync`, `share_links`,
`pr_previews`, `premium_systems`. Share/sync/catalog implementations and account/plan pages belong to
later prompts. This prompt supplies auth, device login, entitlements, cloud jobs and mail.

## Configure and launch

Use Node 24/pnpm 12 for builds, Bun for the API/worker, Postgres and Redis. Inject service secrets from
the deployment secret manager; do not commit environment files or place provider credentials in jobs.
Both the Next account app and Hono API use the same account origin, authentication secret and Postgres.
Proxy `/v1/*` to Hono and all remaining paths to Next on that HTTPS origin. Only the explicitly started
cloud API listens on TCP; its default bind is loopback. The local daemon is unchanged.

| Process | Configuration |
| --- | --- |
| Next account | `DATABASE_URL`, `BETTER_AUTH_SECRET` (at least 32 characters), `VELLUM_ACCOUNT_URL` (HTTPS origin), `AWS_REGION`, `VELLUM_MAIL_FROM`; SES IAM through the AWS credential chain |
| Hono API | Account configuration plus `REDIS_URL`, `POLAR_ACCESS_TOKEN`, `POLAR_WEBHOOK_SECRET`, `POLAR_PRO_PRODUCT_ID`, `VELLUM_WORKER_ROOT`; optional `PORT`, `HOST`, `POLAR_SANDBOX=true` |
| Worker | `DATABASE_URL`, `REDIS_URL`, `POLAR_PRO_PRODUCT_ID`, `VELLUM_WORKER_ROOT`, `VELLUM_WORKER_BINARY` (absolute native executable); no mail or Polar access token |
| Migration command | `DATABASE_URL` only, with DDL rights |

Run `pnpm --filter @vellum/api migrate` explicitly before startup. It applies the auth and billing SQL
in one transaction with a migration lock and checksums; reruns do nothing, edited migrations fail.
The service never migrates automatically. Use database TLS and separate runtime roles in deployment;
the worker only needs to read entitlements. Keep auth tables private and omit URLs/query strings,
authorization and cookies from proxy/access logs. The app does not log requests or vendor exceptions.

Build/start the account app with `pnpm --filter @vellum/account build` and `start`. Start Hono with
`pnpm --filter @vellum/api start`; run a separate `pnpm --filter @vellum/api worker` process. The account
route uses email links through SES, hashed verification tokens and database rate limits. Mail links
expire in five minutes. Device grants expire in ten minutes, require explicit browser approval,
are redeemed once, and issue revocable sessions lasting at most 30 days. Better Auth telemetry is off.

Set Polar's webhook to `/v1/billing/webhook`, API version **2026-10**. The SDK verifies raw bytes,
signature and delivery freshness before Vellum maps subscription state. Checkout uses the authenticated
user ID as `external_customer_id`; neither a caller-supplied user ID nor a return URL is trusted.
The API never grants access because a checkout redirected successfully.

`entitlements` stores one subscription-derived grant per vendor subscription. Billing deliveries and
conditional updates commit atomically. Duplicate event IDs are ignored; resource modification times
order updates; equal-time denial wins. Separate subscriptions remain independent. Only the configured
Pro product grants access, and the current period/end deadline is checked on every read, so a lost
expiry webhook cannot grant access forever. Cancel-at-period-end keeps active paid time; canceled,
revoked and other inactive states grant nothing. Expiry fails closed; there is no offline Pro cache.

## Device login

```sh
vellum login --account-url https://your-account-service.example
vellum whoami
vellum logout
```

There is no guessed production URL. The first login requires `--account-url` or `VELLUM_ACCOUNT_URL`;
subsequent logins can use the origin saved with the session in the OS keychain. Visit the printed URL,
sign in by email, compare the displayed code with your CLI, and approve. The CLI obeys polling/slow-down
and expiry; Ctrl-C ends the request. `--json` emits waiting/completion records, never device or access
tokens. All credential persistence uses the existing OS-only SecretStore. Local sessions/documents and
runtime metadata never receive the account token. Logout deletes it locally even offline and reports
whether server revocation was confirmed. Sessions can also be listed/revoked through the API. Desktop
and extension account controls still need native host integration; their local features remain signed out.

## Public endpoints

| Endpoint | Behavior |
| --- | --- |
| `/api/auth/*` GET/POST | Better Auth email/device/session protocol; same handler in Next and Hono |
| `GET /v1/me` | Authenticated public identity; no session token |
| `GET /v1/entitlements` | Effective capability list; empty signed out |
| `GET /v1/sessions`, `DELETE /v1/sessions/:id` | Own safe session IDs and revocation |
| `POST /v1/billing/checkout` | Authenticated checkout for the configured product |
| `POST /v1/billing/webhook` | Verified Polar delivery; no cookie authentication |
| `POST /v1/jobs`, `GET /v1/jobs/:id` | Pro job submission and owner-only status/artifact ID |

Requests have a 1 MiB body limit. Cookie mutations require the account origin. Bearer requests never
fall back to cookies. Unexpected errors return fixed text; there is no request-body or credential logging.

## Worker staging

This is an internal, operator-owned mount, not an upload endpoint. Prompt 62 supplies ingestion/object
storage. Only a trusted ingestion service may create `inputs/<inputId>/owner.json` containing
`{"userId":"<authenticated-user-id>"}` and a complete `input.vellum` file/folder with its local dependencies.
For PR jobs, stage a complete, trusted Git repository in `repo/`. Never accept client-supplied paths,
Git configuration, remote URLs or an archive extracted without validation as a staging bundle.
Run workers as a dedicated unprivileged user with only their staging/artifact mounts and necessary services.

Submit `{inputId,kind:"thumbnail"}`, `{inputId,kind:"export",format}` (png/jpeg/webp/pdf/svg), or
`{inputId,kind:"pr_preview",base,head}` (full hex commits). The API creates the job ID and user ID,
checks ownership and Pro access, then enqueues. The worker rechecks access before execution.
Both queue implementations preserve job identity and owner-only reads. Redis payloads contain only these
bounded fields. Three attempts use exponential backoff; completed/failed jobs are retained up to one day
and 1,000 entries. Queue and process errors are sanitized before being persisted.

Every run copies its bundle to a private temporary workspace. Symlinks/special files, over 10,000 files,
over 256 MiB and nesting over 32 levels are rejected. A fixed absolute `vellum` binary runs with a
credential-free environment, fixed argv, no shell, a 120-second limit and bounded output. Its isolated
daemon uses a short private socket path and is stopped during cleanup. Exports/thumbnails publish a
file (ZIP when appropriate); PR previews publish their manifest/PNGs. Artifacts move atomically into
`artifacts/<userId>/<jobId>/`; only the opaque job ID is returned. There is no public artifact download
or hosted share link yet. Stage/queue/retention capacity should be sized for deployment concurrency.

## Verification

Ordinary wrapper/account/API unit tests mock vendor boundaries. `apps/api/src/cloud.integration.test.ts`
is tagged `@cloud`: set `VELLUM_TEST_DATABASE_URL` to an isolated Postgres DB and
`VELLUM_TEST_REDIS_URL` to isolated Redis. It creates/drops private schemas, tests real Drizzle device
redemption/revocation and concurrent billing updates, and consumes real Redis jobs.
`render.integration.test.ts` is tagged `@native`: set `VELLUM_TEST_BINARY` to a scene-enabled native
`vellum` binary and allow local sockets. It verifies real PNG, PDF and staged Git preview artifacts.
Both suites explicitly skip absent prerequisites. Cloud CI runs DB/Redis; the export lane runs native jobs.

`cargo test -p vellum-account-http -- --include-ignored` includes the tagged loopback HTTP transport
fixture. No test writes a real keychain entry or calls SES/Polar. The account browser check needs Chrome
and the account app at `VELLUM_TEST_ACCOUNT_URL` (default loopback port 3100):
`pnpm --filter @vellum/account test:browser`. Set `VELLUM_CHROME` when using an installed executable.
It intercepts mail requests, checks success/failure, and compares A05 in both themes at the existing
1%/48-channel gate when the design PNGs are present; missing PNGs are reported as skipped.
Live SES delivery, paid checkout/webhooks, real keychain login, Windows/Linux native execution and remote
CI/deployment acceptance still require their configured environments.
