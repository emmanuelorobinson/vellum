# ADR-033: live editors are replicas of the daemon's engine; the editor WASM module carries the engine

Status: accepted (2026-10-22, decided for the owner, who delegated the choice). Implements the document runtime
Prompt 55 left open and the editor wire ADR-020 left to "the host relay".

## Context

Prompt 55 shipped both hosts read-only: the WASM module exported the renderer only, and `EditorLink` (ADR-020)
was an in-process trait. `LiveTarget` forwards a Rust `Transaction` to the editor holding a file and waits for its
answer. Over IPC that means a serialisable op format and a synchronous call from the daemon's single RPC thread
into an editor whose replies arrive on that same thread: a deadlock unless the daemon grows a second event loop.

## Decision

**One engine, many replicas.** The daemon's `HeadlessTarget` stays the one writer for a file. An editor that opens
the file becomes a *replica* of that engine: it starts from the daemon's genesis document plus every Loro update
since, applies its person's edits in its own engine at once (the canvas never waits on IPC), pushes them as sync
updates, and pulls everyone else's (agents', other editors', the file's) on an interval. CRDT merge does what
op forwarding would have done; no op wire format exists. Undo stays per actor in each replica.

- `DocStore::import_updates_as(payload, actor)`: an editor's push is accepted only when every change it adds
  carries that editor's person (`Actor::Person { device }`). A compromised webview cannot write as an agent.
- While an editor is attached the target does not save on its own and reports `TargetMode::Live`; the editor's
  Save writes the file (the daemon's `save`, or the host's own write followed by `saved`). Closing the last
  editor without saving drops the engine, so the next tool call reads the file again (the person chose Discard).
- Recovery copies, the watcher's merge of external changes and Guard checks of agents' writes are unchanged.
- Host-only RPC `vellum/editor` (`open`, `push`, `pull`, `saved`, `snapshot`, `save`, `revert`, `replace`,
  `close`), outside the agent tool registry. Editors are named per connection; a connection acts only for its
  own editors, and closing it closes them. Paths are scoped to the connection's workspace like
  `vellum/recovery`, so a file outside the host's workspace opens read-only. Requests stay under the 1 MiB
  IPC frame: revert reads the file in the daemon, and only a recovery backup travels as bytes.
- Each target counts a `revision` (transactions, accepted pushes, undone turns); hosts derive dirty state from it.
- `LiveTarget`/`EditorLink` remain for tests of ADR-020's forwarding contract; production no longer uses them.

**The engine in the browser.** `crates/vellum-wasm` gains an `engine` feature: a Loro store, `vellum_core::ops`,
the components resolver, Taffy layout, Parley text (fonts registered by the host, `ParleyFonts::empty`), the
R-tree scene and the display list, behind a JSON-plus-bytes C ABI (`abi.rs`). `@vellum/renderer-skia`'s
`WasmDocumentEngine` implements `EngineBindings` and the canvas `ContentSource`; `createLiveEditor` in
`@vellum/editor-react` composes session, commands, properties panel and canvas over it. Hosts hand the module's
bytes and Inter to the page (`wasmBinary`), so the webview's CSP gains only `'wasm-unsafe-eval'`.

**Two modules.** Parley's `complex-scripts` dictionaries (CJK, Thai, Lao, Khmer, Myanmar line breaking) are about
2.4 MB gzipped. Dropping them in the browser would make browser and native line breaks differ (Invariant 6), so
they stay. `vellum-editor` (engine plus renderer, LTO, `opt-level = "s"`, loaded from disk by VS Code and the
desktop) has its own gate, `editorWasmGzip` under 6.5 MiB (measured 6.27 MB). `vellum-viewer`
(`--no-default-features`, renderer only, speed-optimized) is what the share pages download, and keeps ADR-031's
2.4 MiB gate (measured 1.90 MB).

## Consequences

- No serialisable `Op` format, no daemon-to-editor call, no second event loop. The daemon's RPC thread only answers.
- Agents' edits reach an open editor within one pull interval (250 ms), not instantly.
- A file outside the host's workspace, or any file while the daemon is unreachable, opens read-only (guide §7).
- Desktop tabs attach in the background; a tab's engine waits for that before choosing live or read-only.
- Not done here: library documents and script outputs in the browser scene (instances of libraries show as
  missing), image fills and document fonts in browser frames, and the editor feature ports of Prompts 49–54
  (agent feed, presence, ledger hosts).

## Reference note

ADR-028's room client already advances a cursor only after an atomic import into the same Rust store. The
replica loop reuses that import path and keeps its explicit failure model: a refused push makes the editor
read-only and says so rather than retrying or rebasing silently. No reference code was ported.
