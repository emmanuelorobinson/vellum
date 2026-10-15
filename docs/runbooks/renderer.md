# Renderer I

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
text, emoji, paths, per-side dashed strokes and an image. Full feature goldens belong to Prompt 23.

Hosts import `createRenderer` from `@vellum/renderer-skia` and the built loader from
`@vellum/renderer-skia/module`. Supply a canvas and factory, then draw a Frame with the core-produced
binary display list and byte resources. Viewport dimensions are logical pixels; DPR sets the device
surface once. The display list must already use that device scale; the renderer adds no second zoom.
Use resize for size/DPR changes and dispose when closing the canvas. Rebuild before packaging: dist
is generated and gitignored. Initialization errors can select the later Canvas2D fallback (Prompt 23).

## CI and open verification

.github/workflows/renderer.yml has six native source-build jobs and a software browser integration job.
Native caches are keyed by target, skia-safe version, features, flags and patch-directory contents.
Timings are written to target/skia-measurements and uploaded with render artifacts.
The manual hardware_gpu option requires preprovisioned self-hosted runners labeled vellum-gpu plus
macOS/Windows/Linux and X64/ARM64, with Chrome, emsdk and native build prerequisites.
Hosted jobs do not pretend to supply hardware GPUs. Remote target runs, fresh-clone download timing
and billed CI minutes remain unverified until the workflow runs on the repository.
