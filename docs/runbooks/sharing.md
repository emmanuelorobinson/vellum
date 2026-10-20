# Share snapshots (Prompt 62)

Sharing is optional and explicit. Opening an editor never reads the account keychain or contacts the account
service. The native Share dialog checks only the document and its referenced files. Create share link reads
the existing CLI account session from the OS keychain and requires Pro. See `accounts.md` for device login.

Run the API with the existing auth/billing configuration and these additional server-only values:

- `R2_ENDPOINT`: `https://<account-id>.r2.cloudflarestorage.com` (the regular R2 S3 endpoint).
- `R2_BUCKET`: a **private** bucket. Do not enable public access or a public bucket domain.
- `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`: bucket-scoped server credentials; never renderer inputs.
- `VELLUM_API_URL` in both Next hosts: the API origin. Production uses HTTPS. Loopback HTTP is allowed for
  local acceptance. Serve native login, `/v1/` and the account viewer under the configured account origin;
  published native links resolve to `/share/<id>` there. The separate web app serves the same viewer.

Build the renderer with `pnpm wasm:build` before directly building either Next package. Root `pnpm build`
orders these through the actual Turborepo task graph. `scripts/web-assets.mjs account|web` stages only that
renderer and the existing licensed fonts. Do not deploy a Next viewer without both engine assets. The cloud
workflow builds the module; the renderer workflow includes native/Chrome sharing acceptance. Remote CI is
not verified by the local run.

The daemon's private `shares/` spool contains immutable snapshot files and public progress, not session
credentials. Closing the dialog leaves its worker running. Retry after failure/restart reuses the immutable
bundle and the server's received-chunk list. Editing afterward requires Update snapshot; it is never an
automatic upload. Stored shared status is the last publication known to this daemon; external revocation is
authoritative on every viewer/download request. Native status does not continuously poll cloud metadata.

Share anyway marks missing dependencies explicitly. The current conservative viewer omits derived frames
when any dependency is missing. An invalid script or incomplete render fails the upload; it is never
presented as a complete visual. Script-generated image references must already be among the captured model
assets; otherwise preparation fails closed. Scripts are included for the downloaded document and evaluated
only in the native sandbox, never as web programs. There is no Team invitation or live sync flow here.

Limits: 64 MiB transport envelope including base64, 2,048 files, 256 libraries, 100 visible top-level roots,
32 script evaluations (each retains its existing sandbox limits), four concurrent native jobs, and 1,000
share records per owner. Each chunk is 512 KiB except the last. Uploads expire after 24 hours. R2 conditional
writes are required; an S3-compatible substitute must pass the same CAS contract, not just get/put tests.
Configure bucket lifecycle expiration for `chunks/` after at least two days. Do **not** put expiration on
all `shares/` or `snapshots/`: active links depend on them. Superseded/revoked snapshot bytes and abandoned
local spools currently need operator cleanup after checking the live record; revocation itself disables
public access immediately but does not promise physical erasure. No automatic garbage collector is shipped.

The portable ZIP includes the canonical `.vellum` document at its repo-relative path, its libraries/assets,
script files and inert viewer data. Extract it to a new folder and open its `.vellum` entry in Vellum. The
Open in Vellum action invokes the packaged native protocol. Its native folder picker displays the source
host before any network or filesystem write. It downloads public bytes without signing in and installs
them in a new exclusive folder. ZIP and native imports include an empty host root marker so repo-relative
sibling libraries work outside Git. Development tests inject the OS event; actual installed registration
and Windows/Linux handling remain release acceptance. The browser refuses write operations and exposes no daemon endpoint.

Local acceptance (telemetry opt-outs for pnpm):

```sh
node scripts/skia-build.mjs test -p vellum --test share
node scripts/skia-build.mjs test -p vellum --lib daemon::share
packages/storage/node_modules/.bin/vitest run --root packages/storage src
packages/host-browser/node_modules/.bin/vitest run --root packages/host-browser src
apps/api/node_modules/.bin/vitest run --root apps/api src/shares.test.ts
VELLUM_TEST_SHARE_OUTPUT="$PWD/target/share-acceptance.json" node scripts/skia-build.mjs test -p vellum --test share
bun apps/api/test/share-server.ts
# In another terminal, after building apps/web with VELLUM_API_URL=http://127.0.0.1:3112:
VELLUM_API_URL=http://127.0.0.1:3112 apps/web/node_modules/.bin/next start apps/web -H 127.0.0.1 -p 3111
VELLUM_CHROME=/path/to/chrome node apps/web/test/browser.mjs
```

These use memory cloud boundaries and a real native-rendered fixture, no keychain/vendor credentials.
`browser.mjs` revokes its fixture at the end; restart the fixture API for a repeat. The browser artifacts
are under `target/screen-goldens/`. The W02/W03 gallery and `goldens.mjs` compare actual product components
against unchanged references, retaining the 1%/48-channel gate. Those design comparisons are not accepted;
see `reports.md`. The original design/specification and comparison tolerance are unchanged.
