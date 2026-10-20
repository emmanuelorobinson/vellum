# ADR-027: durable sync rooms and canonical compaction epochs

Status: implemented by Prompt 63; deployment acceptance is recorded in reports.md.

The sync composition root uses the existing DocStore/Loro adapter. UpdateLog, SnapshotStore,
AccessControl, Presence and SyncEndpoint are pure ports in vellum-core, re-exported by
sync-server::ports so adapters never import a composition root. Vendor libraries stay in adapters:
Axum/Tokio WebSockets; Postgres; Redis; bundled SQLite; disk; and object_store for private R2/S3.
The daemon starts no sync listener. Serving requires an explicit `--listen` on the separate binary.

An update is staged on a disposable replica. Every newly imported CRDT change needs a valid Origin
and must belong to the capability's actor. Missing dependencies are rejected for retry with a complete
causal update, never parked past a permission change. Model decoding is fallible; core diff/Op planning
and document invariants validate the result. Existing remote changes are replicated, not re-authored.
An immutable candidate becomes live only after an optimistic revision append is durable. Duplicate
updates do not advance the revision. Read and write authorization is checked on every request.

The SQL room head and ordered update tail are read consistently. Concurrent servers use compare-and-swap;
a loser rebuilds from durable state and retries. WebSocket delivery follows this same durable log,
including updates accepted on another server; Redis is ephemeral membership, not a reliable message bus.
Clients get clear envelope/adapter upgrade errors and a bounded read/write protocol generated from Rust.

Compaction writes an immutable, SHA-addressed canonical `.vellum`, then atomically publishes its key,
advances epoch/revision and prunes the old tail. The canonical file is the next genesis. No Loro binary
snapshot is the durable recovery authority. A failed object write leaves the old head/tail intact; a
failed SQL publication can leave an unreferenced snapshot that an operator may remove.
An offline client from an earlier epoch receives `reset_required` for writes and the canonical file
plus current tail on pull. It must preserve unsent edits and explicitly rebase before retrying. The
server never silently discards them or pretends to merge across a pruned history. Compaction is an
explicit maintenance operation so an operator can coordinate that boundary with connected clients.

Reference note (R4): Loro's encoding docs specify version vectors, checksums and pending dependency
ranges; the adapter delegates that encoding to Loro. Its shallow-concurrency note shows that a peer
forked before a shallow root can never merge into that replica. Vellum keeps causal update exchange
within an epoch and uses an explicit canonical reset at compaction, making snapshot-only recovery
independent of a vendor format. No reference source was ported. Sibling outpost's epoch/reset/catch-up
path informed the durable-cursor boundary; its source was read, not copied.

Permissions use bounded bearer capabilities scoped to room, actor, role and expiry. SQL stores only
SHA-256 capability digests. Administration reads credentials from SecretStore, not command arguments
or document files. Cloud composition additionally checks the room owner's current cloud_sync grant
through the existing entitlements table. Self-host composition has no account or billing dependency.
Capability issuance from an account UI and multiplayer UI/presence rendering belong to later integration.

Revisit when coordinated compaction resets are disruptive, when history tails need pagination beyond
the bounded window, or before multi-region/large-room scaling. A future full-history cache may accelerate
restart but must remain disposable and retain canonical snapshot-only recovery. No dependency tier for
the pure crates changes. The dependency policy adds only the named sync adapter wrappers.
