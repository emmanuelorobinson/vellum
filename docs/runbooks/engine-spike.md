# Engine spike (Prompt 5, ADR-004)

How to rebuild and re-measure the Loro and QuickJS half of Spike B behind
[ADR-004](../architecture/decisions/ADR-004-loro.md). The text half lives in the renderer spike
([renderer-spike.md](renderer-spike.md)). The spike is `spikes/engine/`, a separate Cargo workspace and pnpm package,
so loro and rquickjs never enter the product dependency graph before their adapters exist (Prompts 14 and 25). Spike
quality; those prompts replace it.

## What is in it

| Path | What it is |
|---|---|
| `crates/loro-bench` (`spike-loro`) | The renderer spike's generated documents as a Loro movable tree; tests; `loro-bench`, the native measurement |
| `crates/loro-wasm` | The same steps as exports of a `wasm32-unknown-emscripten` module (ADR-001's editor target), timed from Node |
| `crates/script` (`spike-script`) | rquickjs with a toy `doc` API and limits; tests; the `wasm32-unknown-unknown` cdylib (vellum-wasm plus QuickJS); `script-bench` |
| `scripts/` | `build-wasm.sh`, `measure-loro-wasm.mjs`, `measure-script.mjs` |
| `test/script.wasm.test.mjs` | lane `wasm`: the 500-op script inside the wasm32 module |
| `results/` | `loro.json`, `loro-wasm.json`, `script.json`: the committed measurements behind ADR-004 |

## Setup

emsdk 4.0.6 in `~/emsdk` (or `EMSDK`), as for the renderer spike. rquickjs compiles QuickJS's C for wasm32 and needs a
clang with the WebAssembly backend (Apple's has none), so `build-wasm.sh` uses emsdk's.

## Commands

Run from `spikes/engine/`:

| Command | Does |
|---|---|
| `cargo test` | the Loro and QuickJS tests (native, no lane) |
| `pnpm spike:build:wasm` | builds the wasm32 modules and the `vellum-wasm` size baseline |
| `pnpm spike:measure:loro` | native Loro numbers into `results/loro.json` (about a minute) |
| `pnpm spike:measure:loro-wasm` | Loro numbers in WASM into `results/loro-wasm.json` |
| `pnpm spike:measure:script` | QuickJS size and run times into `results/script.json` |
| `VELLUM_LANES=wasm pnpm spike:test:wasm` | the wasm-tagged test |

Renderer B's size with QuickJS comes from the renderer spike's build (`dist/quickjs/`, a `quickjs` feature of
`spike-skia-wasm`) and is recorded in `spikes/renderer/results/text.json`.

## Notes

- Loro is measured on emscripten, not `wasm32-unknown-unknown`: there loro-internal pulls in wasm-bindgen and
  getrandom's `js` backend, and the module would need wasm-bindgen's glue.
- Timing lives only in the benchmark binaries and the Node scripts; the libraries take no clock. The QuickJS interrupt
  is a budget of interrupt-handler calls, not a deadline.
