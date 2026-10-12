# Renderer spike (Prompt 4, ADR-001)

How to rebuild, view and re-measure the renderer spike behind
[ADR-001](../architecture/decisions/ADR-001-renderer.md). The spike lives in `spikes/renderer/`, a separate Cargo
workspace and pnpm package, so skia-safe and emscripten never enter the product dependency graph. Spike quality:
Prompts 22-23 rewrite the chosen path as the real `Renderer` adapter and then delete the spike.

## What is in it

| Path | What it is |
|---|---|
| `crates/scene` | Generated documents (1,000 / 5,000 / 10,000 nodes), the 20 non-text fixtures, the display list |
| `crates/scene-wasm` | Renderer A's Rust half: builds the display list in `wasm32-unknown-unknown` |
| `web/src/renderer-a.js` | Renderer A: walks the display list in JS and draws with CanvasKit 0.42.0 |
| `crates/skia-draw` | The skia-safe drawing code shared by Renderer B and the native path |
| `crates/skia-wasm` | Renderer B: skia-safe in WASM (emscripten), drawing into WebGL2 from Rust |
| `crates/skia-native` | The native path: fixtures and documents to PNG, and `compare` (vellum-testkit pixel diff) |
| `web/` | The spike page, its server, `measure.mjs` and the gpu-tagged test |
| `results/` | `results.json`, the committed measurements behind ADR-001; `fixtures/` (gitignored) holds the regenerated captures and diffs |

## One-time setup (macOS)

1. emsdk 4.0.6: `git clone https://github.com/emscripten-core/emsdk ~/emsdk && ~/emsdk/emsdk install 4.0.6 && ~/emsdk/emsdk activate 4.0.6`.
   Set `EMSDK` if it lives elsewhere.
2. `brew install ninja`. Skia is built from source for emscripten (see below); it also needs python3 and the
   Xcode command line tools (libclang).
3. `pnpm install` (adds canvaskit-wasm to the spike package).

Why Skia is built from source for Renderer B: Rust 1.99's `wasm32-unknown-emscripten` target links with
`-fwasm-exceptions`, and rust-skia's prebuilt emscripten binaries for 0.153.3 use JS-based setjmp/longjmp, so the link
fails ("invoke_ functions exported but exceptions and longjmp are both disabled"). `scripts/emscripten-env.sh` sets
`FORCE_SKIA_BUILD=1` and `-sSUPPORT_LONGJMP=wasm`. The first build downloads the Skia sources (about 1 GB) and compiles
for one to two minutes on an M5 Pro; later builds reuse `target/`. The native path uses rust-skia's prebuilt binaries.

## Commands

Run from `spikes/renderer/web/`:

| Command | Does |
|---|---|
| `pnpm spike:build` | builds Renderer A's wasm, Renderer B (emscripten) and the native binary into `web/dist/` and `target/` |
| `pnpm spike:serve` | serves the page on `http://127.0.0.1:4173/` (`?renderer=a` or `b`, `&nodes=1000|5000|10000` or `&fixture=1..20`) |
| `pnpm spike:measure` | runs every ADR-001 measurement and rewrites `../results/` (about five minutes; add `-- --software` for SwiftShader) |
| `VELLUM_LANES=gpu pnpm spike:test:gpu` | the gpu-tagged test: fixture #1 through the native path and both browser paths |

The page has a renderer and document picker, drag to pan, wheel to zoom, an FPS meter and a Screenshot button.

## Test lanes

Tests that need hardware or tools CI's default runners lack are tagged by lane (guide §11 rule 19). A lane runs only
when `VELLUM_LANES` (comma-separated) names it; otherwise the test reports as skipped with the reason, never as passed.
The `gpu` lane needs Chrome with a hardware GPU (`VELLUM_CHROME` if Chrome is not at its default path) and the spike
build above. The test fails, rather than passing, if Chrome falls back to SwiftShader.

## Fonts

The documents contain text, so the page and the native path need a font. Both use Geist Regular (OFL-1.1) from the
installed `next` package (`server.mjs` resolves it); no font file is committed. The 20 pixel fixtures contain no text.
Text parity is Prompt 5's spike.
