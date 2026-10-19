# ADR-025: CLI engines with daemon-owned document targets

Status: accepted for Prompt 59 implementation (2026-10-19).

The CLI must share the existing writer when an editor holds a file, preserve turn attribution,
and remain interruptible while a provider streams. Running a model inside the daemon's synchronous
RPC handler would block every other client. Creating another local document store in the CLI
would bypass the authoritative writer.

A native CLI process runs the existing `AgentEngine` adapters. One persistent `vellum mcp` child
relays its tool calls to the existing registry and targets. Native-only `vellum/cli` lifecycle
requests start/join/log/end a turn, prepare reservations and save/create documents. These methods
are outside the advertised model tools and require an initialized, workspace-scoped connection.
Only an active owner connection for the same document can accept a lane join. The daemon generates
agent/session/turn IDs. Steps, usage and messages use the existing SessionLog and engine session kind.

Split validation and lane instructions use core plans/parallel functions. The CLI polls independent
engine futures concurrently; Explore polls them in order. The daemon alone places reservations,
applies leases and mutates documents through targets and attributed core Ops. A disconnected owner
fails the turn and stops its lanes. An explicit Ctrl-C records the person's Stop and interrupts
engines; a scoped native control monitor propagates daemon Pause/Stop while provider streams are idle.
Normal engine or control-connection failure does not masquerade as a person stopping it. Completed edits stay.

The stdio bridge has its own process group so a terminal interrupt can be handled by the CLI before
its control connection disappears. Signal handling stays in the native IPC adapter, using its
existing Tokio dependency. No new vendor dependency or document format is introduced.

This is a native CLI protocol, not the missing remote EditorLink implementation. Real editor
attachment still needs Prompt 55's integration. Catalog/service availability is reported explicitly;
no live model list or unavailable design-system content is fabricated. See `docs/runbooks/cli.md`.
