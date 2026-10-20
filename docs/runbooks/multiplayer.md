# Multiplayer, comments and self-hosting (Prompt 64)

The room service, native transport, editor controller and UI are implemented. The installed desktop
and VS Code editors still open through Prompt 55's read-only runtime boundary. They do not yet attach
RoomSyncClient to a live editable DocStore. Browser fixtures test UI behavior; real Rust sockets test
storage, authorization and convergence. Neither substitutes for that missing host acceptance.

## Connect an editable host

After the person explicitly enables live collaboration, supply `RoomSyncClient` from `@vellum/host-api`
with the shared engine's `EngineSyncBindings` and a host-owned `SyncWire`. Native hosts can use
`NativeSyncWire` from `@vellum/host-api/native-sync` with an absolute bundled binary path, WSS origin,
room id and OS-keychain entry name. Its subprocess is `vellum sync --endpoint ORIGIN --room ID --key NAME`.
Production WSS requires normal certificate trust; plaintext WS is restricted to loopback development.
No capability enters React, command output, the document or the recovery queue.

Use the same engine for bootstrap/import, local accepted-update subscription, edits, undo, document
snapshots and scene queries. Remote imports must not echo as new local transactions. Create the editor
collaboration controller with an injected clock/id generator, a live document subscription, local
presence and camera accessors. Pass it to EditorWorkspace, and explicitly connect the client after the
opt-in. Dispose the controller, room-sharing adapter and client when closing the document.

`RoomLiveSharing` implements role/removal/live controls over the authenticated room protocol. Configure
the actual server label and actor-name resolver; fixture names and domains are TEST DATA. The real room
adapter hides invitations because no account capability issuer/delivery flow exists yet. Provision new
members through trusted administration. Agents appear as participants, not email invitees.

An optional `SyncOutbox` atomically stores the queue and its matching engine history/cursor. Do not
restore a saved cursor over a canonical file that lacks that native history. Without persistence, pending
work survives only while the session remains open. Reconnect after transport loss, but stop and retain
work after a room reset, failed import, access loss or protocol mismatch. Supply host copy/local-copy/
review-rebase/update actions; the rebase control stays disabled without a real host implementation.

## Provision rooms and roles

Follow [sync.md](sync.md) for initial seed, keychain capabilities, cloud configuration and compaction.
Add `--manage` when granting the person Editor capability that owns room-management controls. There
is one owner actor; it cannot be removed or downgraded through the room UI. Other grants use Viewer,
Commenter or Editor. Changing/removing an actor affects all their grants in that room.

Viewer reads the document and comments. Commenter posts and resolves/reopens their own threads.
Editor also edits the canvas and resolves/reopens any thread. Agents edit only under Editor grants;
human comments reject agent authors. There is no comment-deletion control. Turning live off denies
other members' subsequent requests; the owner retains management access. Snapshot links are separate.

Cloud mode additionally requires `VELLUM_TEAM_PRODUCTS`, a comma-separated list of entitled Team product
ids. Those products must also be included in `VELLUM_SYNC_PRODUCTS`. Pro sync alone does not enable
comment or presence operations. Self-host mode bypasses account entitlements while retaining role checks.

## Run the self-host image

Build from the repository root:

```sh
docker build -f crates/sync-server/Dockerfile -t vellum-sync:local .
```

Provision a private host directory using the native server CLI and its OS keychain, seed the document
and issue grants, then stop that process before mounting its data into Docker. Set `VELLUM_SYNC_DATA`
to that directory and `VELLUM_SYNC_UID`/`VELLUM_SYNC_GID` to its owner. The compose defaults are 10001.
Use a private directory on a local disk with working SQLite locks, and keep credentials outside it.

```sh
docker compose -f crates/sync-server/compose.yml up --build -d
curl --fail http://127.0.0.1:7310/health
```

Compose publishes loopback only. Use an HTTPS/WSS reverse proxy for remote use and suppress bearer
headers in logs. The container has no account or keychain dependency during serving: stored grants are
hashes. Back up the SQLite database/WAL and referenced canonical snapshots together. Do not run a second
writer or replace the volume's files while serving. Keep the data volume when replacing the image.
The image runs as a non-root uid with a read-only root filesystem and dropped Linux capabilities.

## Verify

Tagged native tests need local services and loopback networking. Use disposable test services, never a
production database or bucket. `.github/workflows/multiplayer.yml` sets up the matching CI fixtures.

```sh
cargo test -p vellum-core --test comments
cargo test -p vellum-sync-server --test comments
cargo test -p vellum-sync-sqlite --test sharing
cargo test -p vellum-sync-server --test multiplayer -- --include-ignored
VELLUM_TEST_SYNC_IMAGE=vellum-sync:local cargo test -p vellum-sync-server --test image -- --include-ignored
pnpm --filter @vellum/host-api test
pnpm --filter @vellum/editor-core test
pnpm --filter @vellum/editor-react test:e2e multiplayer.spec.ts
```

The multiplayer test always exercises SQLite/disk/memory. Its cloud scenario needs all
`VELLUM_TEST_DATABASE_URL`, `VELLUM_TEST_REDIS_URL`, `VELLUM_TEST_S3_ENDPOINT`, `VELLUM_TEST_S3_BUCKET`,
`VELLUM_TEST_S3_ACCESS` and `VELLUM_TEST_S3_SECRET`. Both scenarios and the image reuse one test with
two people and two agents, real attributed edits, cursor presence, comments and converged documents.
Cloud tests use an S3-compatible fixture; live R2, real OS-keychain/WSS and Windows/Linux editor
acceptance require their deployment environments. See reports.md for current visual and design failures.
