# EngineClient

The editor consumes a typed, synchronous facade over the Rust engine and Scene. Tool geometry and
Person transaction request types now live here; `editor-core` re-exports them for compatibility.
They remain in-process requests, not a wire format or an alternate document store.

`WasmEngineClient` takes injected `EngineBindings`, shields snapshots and requests with copies,
forwards transactions once, preserves rejection, forwards actor-scoped undo/redo, and owns its
subscriptions and binding lifetime. The Emscripten composition must supply the facade; the current
Rust module exports rendering only. This prompt defines/tests the port and adapter boundary, and
does not add a document-operation WASM ABI or implement document semantics in TypeScript. Scene
services, path preparation and Rust operation application must be connected in the canvas host.

`FakeEngineClient` accepts scripted snapshots and apply/undo/redo responses. Unscripted writes return
`unavailable`, read-only snapshots refuse writes, and each scripted revision must increase. It does
not apply operations or simulate undo. This keeps stories deterministic without building another
document engine. Tests against the WASM adapter inject this fake boundary; no compiled WASM engine
has been exercised by these TypeScript contracts.
