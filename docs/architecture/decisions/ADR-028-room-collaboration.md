# ADR-028: attributed comments and credential-free editor sync

Status: implementation present for Prompt 64; host and visual acceptance remain open in reports.md.

RoomSyncClient exchanges generated v1/loro-1 frames through a host-owned wire. Its engine import
must use the same Rust DocStore as editing, undo and scene queries. A cursor advances only after
an atomic import; an acknowledgement removes an outbox item only after recovery storage accepts
the new queue. An epoch reset, incompatible protocol or failed import preserves pending work and
requires explicit review. A denied design update stays blocked in the queue while authorized
comment and room-management requests can continue. No TypeScript document store is introduced.

NativeSyncWire starts the bundled `vellum sync` command with fixed arguments. Only the native
process reads a capability from SecretStore and places it in the WebSocket Authorization header.
There are no query tokens, shell interpolation or authenticated redirects. Recovery storage is an
optional host port: without it, the UI describes unsent work as held in the current session.
The existing native hosts still need Prompt 55's shared document runtime before attaching this
client; the optional EngineSyncBindings boundary does not manufacture that missing implementation.

Comments are optional canonical Document data, omitted when empty. This is an additive extension
to the unreleased v1 format; old canonical fixtures retain their bytes. Each bounded create, reply,
resolve or reopen request becomes an ordinary core transaction with an authenticated person Origin,
injected time and caller-provided ids. Missing anchors retain the conversation and last position.
Comments currently use SetShell, so concurrent shell changes have a coarser conflict boundary than
node properties. The server stages, validates and durably appends the resulting Loro update through
the same CAS path as edits. Raw update uploads cannot change comments and bypass their role rules.

Viewer reads; Commenter creates/replies and resolves their own threads; Editor also edits and
resolves others' threads. Agents publish attributed presence and edit under their grants, but do not
create human comments. Only the configured person owner manages live access and roles. Disabling
live access or removing a member affects subsequent requests on existing connections. Cloud comments
and presence check the configured Team entitlement independently of Pro document sync. Self-host
SQLite/disk/memory uses the same protocol and needs no account, billing, Postgres or Redis service.

Presence remains ephemeral with a server expiry. People are circles and agents rounded squares;
the controller hides disconnected cursors, retains last-seen labels and restores a saved local camera
when following ends. Comments stay separate from agent prompts. Snapshot publication and live access
remain separate controls. Email invitations are unavailable on the room adapter until an account-side
capability issuer and delivery flow exist; the UI does not pretend to send them.

The self-host image runs without root, with a read-only filesystem and a provisioned private data
volume. The same four-actor WebSocket scenario exercises SQLite/disk, Postgres/Redis/S3-compatible
storage and the built image. Canonical snapshots remain the recovery authority across compaction.

Reference note: sibling outpost's bridge sync code uses an epoch/reset/catch-up boundary and retains
pending work across reconnects. Vellum keeps that explicit reset model and advances its cursor only
after importing into the shared engine. Credential handling stays in the native host, and compaction
requires a reviewed rebase rather than silently replacing local work. No reference code was copied.
