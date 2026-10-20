# Sync service (Prompt 63)

This optional service is separate from the local daemon. No local editing command starts it or sends
it a file. The service has no account requirement in self-host mode. Public deployment requires an
HTTPS/WSS reverse proxy; bind the backend to loopback or a private network and disable proxy access
logs for Authorization/cookies. It accepts bearer capabilities, never query-string tokens or cookies.
Browser Origin headers must exactly match an explicit `--origin`; native clients omit Origin.

Build: `cargo build -p vellum-sync-server`. Run `target/debug/vellum-sync-server --help` for commands.
No real credentials belong in shell arguments, SQLite, snapshots, sync frames or logs. Administrative
`--key NAME` reads an existing credential from the OS keychain service named by
`vellum_types::brand::KEYCHAIN_SERVICE`. Provision a random capability of at least 32 characters there
using your normal secret administration tooling. Provision the same capability to the authorized
client's keychain. Each capability binds exactly one room, actor, role and expiry. One actor/device
capability cannot upload newly unseen changes authored by another actor.

## Single-node local storage

```sh
vellum-sync-server --data /private/service/sync seed --room document-id --file /repo/design.vellum
vellum-sync-server --data /private/service/sync grant --room document-id --key device-capability \
  --actor '{"kind":"person","device":"YOUR_DEVICE_ULID"}' --role editor --expires-ms UTC_MILLISECONDS
vellum-sync-server --data /private/service/sync serve --listen 127.0.0.1:7310 --instance unique-node-id
```

Actor JSON uses `kind: "person"` with a device ULID, or `kind: "agent"` with an agent ULID, from the
shared Actor type. The illustrative actor/id/expiry above are placeholders, not usable fixture identities. Roles are `viewer`, `commenter`
and `editor`. Only editor capabilities can submit document updates or request administrative compaction.
Prompt 64 adds comment operations, rendered presence and owner-managed roles. Use `--manage` on the
person Editor grant that owns the room. See [multiplayer.md](multiplayer.md) for the role matrix and image.
The private directory holds `sync.sqlite`/WAL and immutable `<sha256>.vellum` snapshots. Back up both.
The service makes the directory private on Unix. Use a private ACL on Windows. Do not share the data
directory with other writers or replace files while serving.

## Cloud adapters

Add `--cloud` before the subcommand. Configure `DATABASE_URL` to a private Postgres connection or a
local TLS tunnel, `VELLUM_SYNC_REDIS_URL`, `VELLUM_SYNC_PRODUCTS` (comma-separated entitled product ids),
`VELLUM_SYNC_S3_ENDPOINT` (HTTPS R2/S3 origin), and `VELLUM_SYNC_S3_BUCKET` (private pre-existing bucket).
Prompt 64 also requires `VELLUM_TEAM_PRODUCTS` for hosted comments/presence; include those products
in the sync product allowlist too. Self-host mode has no entitlement requirement.
Put storage credentials in OS keychain entries `sync-storage-access` and `sync-storage-secret`.
No AWS metadata discovery, public bucket, signed public URL or public snapshot endpoint is used.
Postgres/Redis connections are intended for a private service network; terminate encryption at the
service boundary or use a local encrypted tunnel for remote database connections.

Cloud `grant` also requires `--owner ACCOUNT_USER_ID`. Every request checks that owner's active,
unexpired subscription in Prompt 61's `entitlements` table against the configured products. A webhook
revocation therefore prevents subsequent reads/writes on existing sockets. These grants are scoped
capabilities, separate from login sessions; no account UI issues them yet. Use trusted administration
to issue/revoke grants until that integration is built. Never expose the administrative CLI as an API.
SQL tables use the `vellum_sync_` prefix and initialize idempotently on startup. Self-host SQLite uses
`sync_` tables. Run one startup migration before admitting concurrent replicas. The current schema
is version 1; a later incompatible schema must add a migration, not silently change stored rows.

## Protocol and recovery

`GET /health` gives a credential-free health check. `GET /v1/rooms/{room}` upgrades to a WebSocket
only after authorization. Each JSON frame has `v:1`, `adapter:"loro-1"`, `epoch` and a request:
`{"type":"pull","after":revision}` or `{"type":"push","bytes":[opaque update bytes]}`. See `schema` for
exact generated schemas and `@vellum/types-ts` for generated types. Send a pull first, with epoch 0
on first connection. Bootstrap contains canonical `.vellum` bytes plus the ordered accepted update tail.
Construct the DocStore from that canonical document, then import tail updates in order. Within the
epoch, persist the returned revision only after applying the entire state response. An ack confirms a
push is durable; it does not mean other concurrent updates have been received. The socket continues
to deliver state after ack. Reconnect with the last fully applied epoch/revision **and its matching native
history/genesis**. A canonical local file alone cannot recreate that cursor: bootstrap with epoch 0 and
preserve/rebase local edits before replacing its native state.

Clients should send causal deltas since the last acknowledged store cursor. Missing-dependency payloads
are rejected atomically; retry with the complete needed history. Duplicates are accepted without another
log entry. Changes are validated on a disposable replica, through core diff/Op planning and a canonical encode/decode
check, before SQL append. A document outside the JSON codec's nesting/readability limits is rejected
before it can become an unreopenable checkpoint.
A failed append never updates the room or broadcasts. Another server's accepted update is delivered from
the same durable log (250 ms polling), so Redis pub/sub loss cannot lose document changes.

There are at most 32 connections and 8 blocking workers per process (16 MiB stacks for bounded nested
model validation), 64 cached rooms, 2 MiB per incoming
update, 32 MiB per canonical document, and 256 updates/8 MiB per room tail. Input frames, output buffers, dependency changes, decoded model depth and node count are bounded.
WebSocket sends/idle time, SQL statement/lock time and Redis/S3 I/O have explicit bounds. Clients answer
WebSocket pings; inactive/dead clients are closed. Connection membership expires after 30 seconds and
is removed on disconnect. Opt-in v1.1 clients publish bounded presence state and receive attributed
membership/role frames. Comments and owner management use generated `comment`/`sharing` requests;
they never enter the replaceable presence channel.

Before reaching a tail limit, coordinate a maintenance boundary with writers, retain their unsent edits,
then run:

```sh
vellum-sync-server --data /private/service/sync compact --room document-id --key editor-capability --epoch CURRENT_EPOCH
```

Compaction stores canonical `.vellum` bytes before atomically switching the head, incrementing epoch
and pruning the tail. Existing sockets receive a fresh state; stale pushes return `reset_required`.
Clients must preserve unsent edits and explicitly rebase them onto the new document before resubmitting.
Native history and undo caches reset at this boundary. The service does not silently discard unsent edits
and does not claim offline convergence across compaction.
Replicas applying the same admitted, valid updates converge within an epoch. Concurrent edits that
would violate document invariants are rejected; preserve and rebase that client's pending Ops. See ADR-027 for Loro's shallow-history boundary.
Keep the latest canonical snapshot even if all native update data is lost. To rebuild from snapshots
alone, provision a **new room id** with `seed --file <snapshot>.vellum`, grant capabilities there, and
bootstrap clients. Reusing an old room id with reset counters can confuse old cursors and is unsupported.
The snapshot has no native CRDT payload and opens with the normal JSON codec. Assets/libraries referenced
by the document must be managed by the host; this sync protocol covers the document, not repository files.

Snapshots published before a failed SQL CAS, and snapshots superseded by compaction, remain immutable.
Garbage collection is an operator task: retain every key referenced by a backed-up/current SQL head.
Do not delete a referenced object or prune updates independently of the checkpoint transaction.

## Verification

Default tests use pure deterministic memory ports, local private temporary disk/SQLite, and Loro contract
and convergence suites. `crates/contracts/src/suites/sync.rs` is shared by every storage implementation.
No cloud services are needed for the normal test lane. Tagged tests explicitly require loopback networking:

```sh
cargo test -p vellum-sync-ws --test transport -- --include-ignored
cargo test -p vellum-sync-server --test network -- --include-ignored
# All VELLUM_TEST_DATABASE/REDIS/S3 values below configured together:
cargo test -p vellum-sync-server --test cloud_network -- --include-ignored
VELLUM_TEST_DATABASE_URL=... cargo test -p vellum-sync-postgres --test contract -- --include-ignored
VELLUM_TEST_REDIS_URL=... cargo test -p vellum-sync-redis --test contract -- --include-ignored
VELLUM_TEST_S3_ENDPOINT=... VELLUM_TEST_S3_BUCKET=... VELLUM_TEST_S3_ACCESS=... VELLUM_TEST_S3_SECRET=... \
  cargo test -p vellum-assets-s3 --test snapshots -- --include-ignored
```

Use disposable local Postgres/Redis and an S3-compatible private test bucket. The environment credential
path above belongs only to tagged test code. Production object and client capabilities use SecretStore.
Do not use a production bucket/database for contract tests. CI runs the native loopback/SQL/Redis lanes;
private R2 and signed deployment acceptance require the operator's environment.
