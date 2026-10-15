# Renderers

The production renderer is shared Rust/Skia, as selected by ADR-001 and detailed in ADR-016.
No account or platform font installation is required. ParleyFonts bundles Inter and Noto Color Emoji.

## Native

Install Rust from rust-toolchain.toml, Python 3, ninja, a C++ compiler and libclang. Linux additionally
needs OpenGL development libraries. Windows needs LLVM/libclang and the native MSVC build environment.
The first Skia source build fetches its pinned sources; later builds reuse the target cache.

```sh
node scripts/skia-build.mjs test -p vellum-render-skia -p vellum-text-parley
node scripts/skia-build.mjs run --release -p vellum --example renderer-fixtures -- target/renderer-fixtures
```

This creates TEST DATA PNG, two-page PDF, SVG, binary display list, side fonts/image and native RGBA.
The API takes encoded asset bytes and FontSource bytes; it never reads filenames or fetches URLs.
See the Rust tests for the Renderer/Resources/Page construction. Raster exports are transparent.

Source builds are always used. `VELLUM_SKIA_FROM_SOURCE=1` is accepted for compatibility with the guide.
`SKIA_SOURCE_DIR` may point to a synchronized source tree for fully offline builds (`--offline` also
keeps Cargo offline). No binary archive download is enabled. Original library notices for release
packaging can be assembled with:

```sh
node scripts/skia-notices.mjs /absolute/path/to/synchronized/skia target/skia-notices
```

## Editor and parity

Install emsdk **4.0.6** at ~/emsdk or set EMSDK. The editor uses wasm32-unknown-emscripten; the old
unknown-unknown artifact is only an import-free version smoke test. Builds need writable toolchain
cache directories. No wasm-bindgen or CanvasKit package is needed.

```sh
pnpm wasm:build
node scripts/renderer-gpu.mjs
node scripts/renderer-gpu.mjs --software
```

The hardware command requires Chrome with a working hardware GPU (set VELLUM_CHROME if necessary).
The second command explicitly selects SwiftShader for hosted-CI integration and is not GPU-performance
evidence. Both start a loopback-only test server, compare raw RGBA against native output, exercise
actual WebGL context loss/restoration, then run the Renderer contract. The matching tagged test is:

```sh
VELLUM_LANES=gpu node --test scripts/__tests__/renderer-gpu.test.mjs
```

Generate the native parity inputs before these commands. Artifacts/results are in target/renderer-fixtures.
Comparisons preserve ADR-001's limits: CPU-to-CPU channel 2 with no pixels exceeding it; CPU-to-GPU
channel 2 with at most 2% exceeding it; p95 at most 1 in both. The current fixture includes variable
text, emoji, paths, per-side dashed strokes and an image. The Prompt 23 feature suite below supplements this compatibility fixture.

Hosts import `createRenderer` from `@vellum/renderer-skia` and the built loader from
`@vellum/renderer-skia/module`. Supply a canvas and factory, then draw a Frame with the core-produced
binary display list and byte resources. Viewport dimensions are logical pixels; DPR sets the device
surface once. The display list must already use that device scale; the renderer adds no second zoom.
Use resize for size/DPR changes and dispose when closing the canvas. Rebuild before packaging: dist
is generated and gitignored. Initialization can select the Canvas2D fallback described below.

## CI and open verification

.github/workflows/renderer.yml has six native source-build jobs and a software browser integration job.
Native caches are keyed by target, skia-safe version, features, flags and patch-directory contents.
Timings are written to target/skia-measurements and uploaded with render artifacts.
The manual hardware_gpu option requires preprovisioned self-hosted runners labeled vellum-gpu plus
macOS/Windows/Linux and X64/ARM64, with Chrome, emsdk and native build prerequisites.
Hosted jobs do not pretend to supply hardware GPUs. Remote target runs, fresh-clone download timing
and billed CI minutes remain unverified until the workflow runs on the repository.

## Prompt 23: full appearance and Basic renderer

ADR-017 defines normalized gradient coordinates, Coons mesh handles, shadow spread/sigma, cache keys
and limits. Runtime shaders remain explicitly unavailable until v1.1. Filtered viewport scenes use
the shared CPU compositor and cached tiles to preserve editor/headless parity. SVG embeds the complete
viewport for advanced appearance; PDF uses Skia's page canvas. Inspect the ADR before assuming vector
SVG or GPU-native blur performance.

For S02, create a dedicated editor module with `noInitialRun: true`, without creating a GL context.
Pass it to `createCompiler` from `@vellum/renderer-canvas2d`, then supply that compiler to that package's
`createRenderer(canvas, compiler, viewport)`. Select this when Automatic cannot initialize WebGL2 or
when the person selects Basic. The returned `kind` is `basic`; use `@vellum/config`'s `basicRenderer`
copy for the host's S02 note. `simplifications` explicitly reports omitted effects, averaged meshes,
transparent shader paints, and linear burn represented as normal. Exports use the full native renderer.
The compiler callback can also return deterministic BasicScene fixtures as a reference fake.

The compiler shares geometry/font lowering but Canvas2D draws independently; it never calls the WebGL
renderer. Text is rasterized from supplied fonts to preserve hinting and emoji; other shapes remain
Path2D fills. Gradient sampling supports affine maps, capped at 2,048 pixels per axis; glyph scale is
capped at 8×. Requires Canvas2D, Path2D, OffscreenCanvas and the WASM module used by the supported hosts.

```sh
node scripts/skia-build.mjs run --release -p vellum --example renderer-fixtures -- target/renderer-fixtures
node scripts/skia-build.mjs run --release -p vellum --example renderer-goldens
node scripts/skia-build.mjs run --release -p vellum --example renderer-budget
pnpm wasm:build
node scripts/renderer-gpu.mjs
node scripts/renderer-goldens.mjs
node scripts/renderer-goldens.mjs --software
```

Run the generators in order. The new suite needs Chrome; its hardware command fails if only software
rendering is available. `VELLUM_LANES=gpu node --test scripts/__tests__/renderer-goldens.test.mjs` is the
tagged equivalent. Ordinary tests check corpus completeness and comparator corruption without Chrome.
The workflow runs native goldens on six platforms and browser cross-products on hosted software WebGL2,
plus the existing opt-in hardware runners. These remote jobs have not been run from this session.

Every 256×256 fixture has explicit browser CPU/GPU/Basic thresholds in
`fixtures/rendering/tolerances.json`. Comparison measures premultiplied RGB and alpha, keeping low-alpha
RGB quantization from overwhelming the visual metric. Basic compares against the documented simplified
native projection; the full renderer always compares against full native output. The old P22 raw-RGBA
fixture still runs, with its original thresholds. Failed comparisons leave actual RGBA and magenta diff
PNGs; native PNGs and the JSON result table are alongside them in `target/renderer-goldens/`.

To deliberately record **reviewed** changes to native expectations, run the renderer-goldens example
with `-- --record`; ordinary runs never record. Keep the prior analytic pixel tests and inspect artifacts.
A different backend failing a golden is a renderer failure, not a reason to record new expectations.

### Architecture §12 measurement scope

Local measurements: Apple M5 Pro, macOS arm64, Chrome 155/ANGLE Metal, release Rust/WASM. Browser
samples include transfer, prepare, drawing and `gl.finish()`, excluding HTTP fixture fetch, core ops,
layout, user-input dispatch and window scheduling. 60 changing pan/zoom frames and 60 repeated tile
frames at 1,000×600; 30 single-node-move frames and 30 distinct 200-node batches. The native open test
reads an exactly 2,097,152-byte synthetic 5,000-rectangle document (long TEST DATA labels), decodes,
resolves, lays out, builds/culls the scene, encodes/prepares VDL and reads 1,000×600 pixels; 20 warm runs.
It excludes process startup, OS cold file cache, fonts and editor initialization.

| §12 target | Local result | Acceptance scope |
|---|---|---|
| 5,000-node pan/zoom, 60 fps | p95 7.10 ms changing; 0.20 ms cached tiles | Renderer timing on M5 Pro; M1/mid-range Windows remains unverified |
| Drag input → frame <16 ms | Renderer p95 5.20 ms | Host input/dispatch and core incremental work not included |
| Open 2 MB <500 ms | Native engine p50 64.86 ms, p95 86.50 ms | Warm native engine only; host first paint remains unverified |
| 200 nodes applied and painted <100 ms | Renderer p95 0.40 ms | Operation application and host work not included |
| Core WASM <2 MB gzip | Shared core + renderer WASM 1,880,119 bytes gzip | Actual final bytes in browser-results.json; loader measured separately there |

No M1/Windows, complex 5,000-effect-scene, or end-to-end host budget claim follows from these samples.
Re-run the supplied harnesses on the guide's target machines and later with the host UI in the loop.
