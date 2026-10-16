# ADR-001: Renderer: skia-safe in WASM everywhere, not CanvasKit in TypeScript

- **Status:** Accepted (closes delta D1 of ADR-000). Frame numbers on an M1 and a mid-range Windows laptop are still
  owed; see "Not measured". ARCHITECTURE synchronized on the owner's instruction (2026-10-16).
- **Date:** 2026-10-11
- **Written by:** Prompt 4 (Spike A). Prompt 5 appended the text results (see "Text (Prompt 5)" at the end).
- **Evidence:** `spikes/renderer/` (code), `spikes/renderer/results/results.json` (every number below, including each
  fixture's pixel-diff report), and `docs/runbooks/renderer-spike.md` (how to rebuild and re-measure). The fixture
  captures and diff images are not committed: `pnpm spike:measure` regenerates them in `spikes/renderer/results/fixtures/`.

## Context

ARCHITECTURE §3.2 has the editor drawing with CanvasKit from TypeScript and the daemon drawing with skia-safe natively,
matched by goldens. Guide §6.1 D1 proposes one Rust renderer everywhere instead: skia-safe compiled to WASM with
emscripten, so the editor and the headless renderer are the same code. The spike built both and measured them.

- **Renderer A:** Rust (`wasm32-unknown-unknown`, plain C exports, no wasm-bindgen) builds a flat `f32` display list;
  TypeScript walks it every frame and draws with canvaskit-wasm 0.42.0 into WebGL2.
- **Renderer B:** one emscripten module holds the scene, the same display list and skia-safe 0.153.3; Rust draws into
  WebGL2 through Ganesh. JS passes the view transform once per frame. The same drawing crate (`spike-skia-draw`) renders
  PNGs natively.

Both draw the same display list with the same Skia calls, without culling or tiling, so the comparison isolates the
JS/WASM boundary and the build. Documents are generated, 1,000 / 5,000 / 10,000 nodes of frames, text, rects, ellipses,
polygons, gradients and images (`spikes/renderer/crates/scene`).

## Numbers

Machine: Apple M5 Pro, 48 GB, macOS 27.0.1, Chrome 155.0.8059.39 headless (`--headless=new`), WebGL2 on ANGLE Metal,
canvas 1280 x 735 device px, DPR 1, vsync and the frame-rate limit off so frame time measures the renderer. Medians of
3 runs of 5 s each after 1 s of warm-up, continuously panning and zooming between "whole document" and 4x that scale.

### Frames per second and frame time

| Nodes | A fps | B fps | B / A | A frame p50 / p95 (ms) | B frame p50 / p95 (ms) |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 572 | 1,100 | 1.9x | 1.8 / 2.9 | 0.8 / 2.1 |
| 5,000 | 169 | 418 | 2.5x | 5.6 / 9.2 | 1.6 / 5.5 |
| 10,000 | 88.5 | 232 | 2.6x | 9.9 / 18.7 | 3.0 / 10.9 |

In every row the p50 time spent inside the draw call is within 0.1 ms of the p50 frame time (`drawCallMs` in
results.json), so both renderers are bound by issuing draw calls on the CPU, not by the GPU. Renderer A crosses the JS/WASM boundary for every draw call and paint
setter; Renderer B crosses it once per frame. At 10,000 nodes A misses a 60 Hz frame at p95 (18.7 ms) and B does not
(10.9 ms).

With SwiftShader (software WebGL, `--software`), where the GPU process is the bottleneck, the two are equal: A 368 /
121 / 60.6 fps and B 401 / 120 / 62.2 fps for 1,000 / 5,000 / 10,000 nodes.

### Drag input to frame latency

From the pointer event's timestamp to GPU completion of the frame that applied it (a 1-pixel `readPixels` forces
completion; display scan-out is not included). 90 synthetic drag moves per run through the DevTools protocol, 270 per
row.

| Nodes | A p50 / p95 (ms) | B p50 / p95 (ms) |
|---:|---:|---:|
| 1,000 | 2.0 / 2.4 | 1.2 / 3.8 |
| 5,000 | 6.4 / 7.6 | 2.9 / 3.6 |
| 10,000 | 14.7 / 15.9 | 6.7 / 7.3 |

### WASM size and cold load

| Bundle | Raw | gzip -9 |
|---|---:|---:|
| A: canvaskit.wasm + canvaskit.js (default build) + the 37 KB Rust display-list module + glue | 7.48 MB | 3.02 MB |
| B: skia_wasm.wasm + emscripten JS + glue (no paragraph layout) | 3.98 MB | 1.55 MB |
| B with skia-safe `textlayout` in use (SkParagraph, SkShaper, HarfBuzz, ICU data) | 6.55 MB | 2.65 MB |
| CanvasKit "full" build, for reference | 8.37 MB | 3.35 MB |

B's default build has `pdf` and `jpeg` (skia-safe defaults) and `gl`. The `textlayout` row is the fair comparison with
CanvasKit, whose default build ships paragraph layout; text needs it (Prompt 5).

Cold load, from navigation start to the first frame complete on the GPU with a fresh browser profile, the cache
disabled and the files served from localhost (so it is download-free; it measures fetch, compile, instantiate and the
first draw of 1,000 nodes), median of 5: **A 243 ms, B 224 ms.**

### Native vs WASM pixels (20 non-text fixtures, 256 x 256)

Compared with `vellum-testkit`'s pixel diff (Prompt 4); a pixel counts as different when any channel moves by more than 2.

| Pair | Identical | Within ±2 | Worst max delta | Worst p95 | Worst / mean differing share |
|---|---:|---:|---:|---:|---:|
| A (WebGL2) vs B (WebGL2) | 20 | 20 | 0 | 0 | 0 / 0 |
| native CPU raster vs B's CPU raster inside WASM | 17 | 20 | 1 | 1 | 0 / 0 |
| native CPU raster vs A or B on the GPU | 5 | 11 | 85 | 1 | 1.85% / 0.33% |

A and B are pixel-identical: both are Ganesh over WebGL2. Native and WASM Skia agree to within 1 on the same backend.
Every larger difference is between Skia's CPU rasterizer and its GPU backend, at anti-aliased edges only (rounded
corners, ellipses, rotated rects, the star, subpixel grids; `pnpm spike:measure` writes the diff images to `results/fixtures/diffs/`). Fills, gradients, images,
blend modes and opacity layers match within 1. So the parity question is CPU raster vs GPU, and it is the same for both
renderers.

## Decision

1. **Accept D1.** The editor renders with skia-safe compiled to WASM (`wasm32-unknown-emscripten`), drawing into
   WebGL2 from Rust. The daemon, CLI and workers run the same drawing code natively through `vellum-render-skia`.
   CanvasKit is not used in product code: no `renderer-canvaskit` package, and canvaskit-wasm stays a spike-only
   dependency. The `Renderer` port stays; `renderer-canvas2d` remains the fallback.
2. **One browser module.** The engine (`vellum-core`) and the renderer ship as one emscripten module, so a frame never
   crosses the JS boundary per draw call; that crossing is what makes A 2-2.6x slower. Exports are plain C functions
   with a thin hand-written JS layer, as in the spike; wasm-bindgen is not needed for the renderer. `vellum-wasm`, today
   built for `wasm32-unknown-unknown`, moves to `wasm32-unknown-emscripten` when Prompts 22-23 build the real renderer.
3. **Goldens compare like with like.** Pixel goldens run CPU raster against CPU raster (native vs WASM agree within 1)
   and GPU against GPU (gpu lane). A comparison across backends, such as an agent's headless screenshot against the
   editor's canvas, uses an edge tolerance: per-channel 2, at most 2% of pixels differing, p95 delta at most 1. Prompts
   22-23 decide whether headless renders should use a GPU backend instead, which would remove the edge difference.
4. **Build Skia from source for emscripten.** rust-skia's prebuilt emscripten binaries for 0.153.3 use JS-based
   setjmp/longjmp, while Rust 1.99's emscripten target links with `-fwasm-exceptions`, so they fail to link ("invoke_
   functions exported but exceptions and longjmp are both disabled"). The spike sets `FORCE_SKIA_BUILD=1` with
   `-sSUPPORT_LONGJMP=wasm` (`spikes/renderer/scripts/emscripten-env.sh`): about one to two minutes on an M5 Pro after a
   1 GB source download. Prompt 72's CI either caches that build or publishes Vellum's own binaries through
   `SKIA_BINARIES_URL`. The native path keeps rust-skia's prebuilt binaries.

## Costs accepted

- The emscripten toolchain (emsdk 4.0.6, ninja, libclang) on every machine and CI runner that builds the editor.
- A 1.55 MB gzip renderer, 2.65 MB with text layout. That is 0.4 MB smaller than CanvasKit with text, but larger than
  no renderer at all.
- GPU context handling in Rust: creating the Ganesh context over the page's WebGL2 context took about 20 lines; context
  loss, resize and DPR changes are not handled yet (Prompts 22-23).
- `unsafe` FFI exports in the composition root, which the workspace's `unsafe_code = "deny"` will need an allowance for
  in that one crate.

## Not measured

- **M1 and a mid-range Windows laptop.** The guide asks for both, or the GPU CI lane, which does not exist yet. Only
  the M5 Pro above was available. B's lead comes from CPU-side boundary crossings, which should cost relatively more on
  slower CPUs, but that is an expectation, not a measurement. `node measure.mjs --frames-only` runs on another machine
  with only `web/dist`, node and Chrome; reports.md lists it for a person.
- Headed Chrome with vsync on, high-DPI canvases, and network download time.
- Text: measured by Prompt 5, see "Text (Prompt 5)" below.

## Revisit when

- Frame numbers from an M1 or a mid-range Windows laptop show B slower than A, or B's 10,000-node frame p95 above
  16.7 ms on that laptop after Prompts 22-23 add culling and tiling.
- A pinned Rust or emsdk upgrade breaks the emscripten Skia build and it cannot be fixed within one prompt.
- **Vello:** reconsider `render-vello` (behind a flag) when all three hold: Vello ships a stable release whose CPU and
  GPU renderers agree within the tolerance above on these 20 fixtures and Prompt 5's text fixtures; Vellum's hosts
  (Chrome, VS Code webviews, Electron) enable WebGPU by default, or Vello's WebGL2 path is stable; and it beats B's
  10,000-node frame p95 on the same machine.

## Reference reading

- `refs/penpot/render-wasm` (docs on rendering architecture, tiles and serialization; `render.rs`, `tiles.rs`,
  `render/surfaces.rs`, `render/fills.rs`) shows Rust plus Skia on emscripten in production: a GPU path and a CPU/PDF
  path that must agree, tiles cached at 512 px, and shapes passed to WASM as packed binary rather than objects. Vellum
  keeps the shape of that (one module, packed data across the boundary, Rust owns the GPU context) and changes the
  scope: the spike has no tiles, and parity is checked by goldens rather than by sharing composition code. Penpot is
  MPL-2.0, so nothing was copied; its build notes also confirmed the wasm longjmp requirement.
- `refs/graphite/frontend/wrapper/src/{editor_wrapper,wasm_value}.rs` marshal messages to JS as serializable values
  through wasm-bindgen. Vellum avoids per-message marshalling on the hot path: the frame needs only the view transform,
  so plain C exports are enough.

## Text (Prompt 5)

- **Date:** 2026-10-12. **Evidence:** `spikes/renderer/results/text.json` (every number below, each fixture's diff
  report), the fixtures in `spikes/renderer/crates/scene/src/text_fixtures.rs`, captures and diff images regenerated by
  `pnpm --filter @vellum/spike-renderer spike:measure:text` into `results/text-fixtures/` (not committed).

### What was compared

30 text fixtures (256 x 256): 6 Latin (sizes 11 to 48 px, kerning and ligatures, accents, centred), 5 Arabic (wrapping,
Arabic-Indic digits, tashkeel), 5 CJK (Chinese, Japanese, CJK punctuation, mixed with Latin), 4 emoji (single, inline,
ZWJ sequences, skin tones and flags), 7 mixed bidi (Latin in Arabic and the reverse, Hebrew with numbers, end-aligned,
wrapping, Arabic + CJK + Latin + emoji) and 3 wrapping. Fonts are Noto Sans, Noto Sans Arabic, Noto Sans Hebrew, Noto
Sans SC and Noto Color Emoji (CBDT), pinned by SHA-256 in `spikes/renderer/fonts.json` and downloaded, not committed.

Text is laid out the way Prompt 19 will do it: **Parley 0.12.0 shapes, applies bidi and breaks lines in Rust**, inside
each module (native, Renderer A's Rust module, Renderer B's emscripten module), and every path draws the same
positioned glyphs (`drawGlyphs` / `draw_glyphs_at`; anti-aliased, subpixel positioning, no hinting, linear metrics).
So the comparison is glyph rasterization only. As a separate check, the same text was laid out by Skia's own
SkParagraph (native) and compared with Parley's.

### Pixels (a pixel differs when a channel moves by more than 2)

| Pair | Identical | Within ±2 | Worst max delta | Worst p95 | Worst differing share |
|---|---:|---:|---:|---:|---:|
| A (WebGL2) vs B (WebGL2) | 30 | 30 | 0 | 0 | 0 |
| native CPU raster vs B's CPU raster in WASM | 25 | 30 | 1 | 0 | 0 |
| native CPU raster vs A or B on the GPU | 0 | 28 | 3 | 1 | 0.0015% (1 pixel) |
| Parley layout vs SkParagraph layout (native) | 0 | 0 | 254 | 53 | 8.4% |

Every category (Latin, Arabic, CJK, emoji, bidi, wrapping) stays within a max delta of 3 between native and the GPU.
Unlike shapes (ADR-001's fixtures: up to 85 at anti-aliased edges), text barely differs across backends: Ganesh draws
glyphs from masks rasterized on the CPU by the same FreeType code, so CPU and GPU agree.

Parley and SkParagraph break all 30 fixtures into the same number of lines, but place glyphs differently (line-height
and baseline model, shaping and fallback details), so their pixels differ by up to 8% of the canvas. That is a layout
difference, not a rasterization one, and the reason only one layout engine may exist.

### Size

| Bundle | Raw | gzip -9 |
|---|---:|---:|
| B without text layout (Prompt 4) | 3.98 MB | 1.55 MB |
| **B with Parley** (layout in Rust, Skia draws glyphs) | 4.96 MB | **1.95 MB** |
| B with Skia's `textlayout` (SkParagraph, SkShaper, HarfBuzz, ICU; Prompt 4) | 6.55 MB | 2.65 MB |
| A's Rust module with Parley (was 37 KB) | 1.15 MB | 0.41 MB |

### Decision for text

1. **Prefer Renderer B for text too,** with layout in Rust: Parley lays out (Prompt 19's `TextEngine`), Skia only
   draws positioned glyphs. **Do not enable skia-safe's `textlayout`** in the product: it would be a second layout
   engine that disagrees with Parley by up to 8% of pixels, and it costs 0.7 MB gzip more than Parley.
2. **Every renderer uses Skia's FreeType font manager** (`FontMgr::custom_empty`) with fonts passed in as bytes, never
   the platform's (CoreText, DirectWrite) and never system fonts. The emscripten build already does. rust-skia's
   prebuilt macOS binaries do not include FreeType (`embed-freetype` is dropped as redundant on macOS), and CoreText
   both rasterizes differently and cannot read CBDT colour emoji, so the native build compiles Skia from source with
   `skia_use_freetype=true skia_enable_fontmgr_custom_empty=true` (`spikes/renderer/scripts/native-env.sh`). Prompt 22
   decides whether Vellum publishes those binaries (`SKIA_BINARIES_URL`) or builds them in CI; Windows needs the same
   check (rust-skia says its Windows builds lack FreeType too).
3. **Text goldens** can use a tight cross-backend tolerance: per-channel 3, at most 0.01% of pixels differing. Prompt
   23 should still keep CPU-vs-CPU and GPU-vs-GPU suites for shapes (Decision 3 above).

### Costs and caveats

- The native from-source build hits a bindgen problem with the macOS 27 SDK's libc++ (an alias with an unbound type
  parameter, `std___hash_table___node_allocator`). `spikes/renderer/scripts/build-native.sh` patches that one line in
  the generated bindings and rebuilds; a skia-bindings release or a pinned SDK should remove the need.
- Color emoji are bitmap glyphs (CBDT at 109 px) scaled down; they match across paths here, but large emoji will look
  soft. COLRv1 is the alternative Prompt 19 should try.
- Only the M5 Pro, headless Chrome 155 on ANGLE Metal, DPR 1.
