# ADR-016: shared Skia renderer, source builds and deterministic font rasterization

- Date: 2026-10-15.
- Status: implemented locally on macOS arm64; the six-target remote acceptance runs remain unverified.
- Scope: Prompt 22, extending ADR-001 and ADR-013/015. The guide calls this ADR-010, but that number
  already belongs to the document model. No existing architecture/specification document was rewritten.

## Build and distribution

Choose **A: source builds with CI caching**. There is no verified Vellum archive set or publishing
infrastructure. A keeps the font policy enforceable immediately, without introducing unverified downloads.
B remains a future optimization after the native matrix establishes its measurements. This is an
operational choice, not a claim that compiling is faster than downloading.

`skia-safe = 0.153.3` has default features disabled, with only gl, pdf, jpeg and embed-freetype.
In particular, binary-cache and textlayout are off. Ordinary Cargo builds cannot fetch upstream binary
archives. `scripts/skia-build.mjs` forces source builds, strips binary-download overrides, honors
`VELLUM_SKIA_FROM_SOURCE=1`, and records target/flags/timing in target/skia-measurements.
Already-synchronized sources can be reused offline through SKIA_SOURCE_DIR; source download time is
not included in the local measurements below. Source downloads are checked out at rust-skia's pins.

The cache/build identity contains skia-safe version, target, features, GN flags, emsdk version/link flags
and the complete contents of patches/skia-bindings. The CI cache also includes Cargo.lock through
rust-cache. Changes produce another identity. No distributable binary archives are downloaded or
published by option A, so there is no archive checksum allowlist or hosting endpoint to maintain.
If B is added, its consumer must verify a committed SHA-256 before invoking rust-skia; the custom URL
alone is insufficient. CI build caches are not release archives.

Local machine: Apple M5 Pro, macOS 27.0.1, macOS SDK 27.0, Rust 1.99.0, emsdk 4.0.6.
An isolated cold native renderer/text test build took 47.76 s, and its cached repeat 0.24 s.
The first product native check with synchronized source took 54.71 s. Rebuilding native release Skia
with the final precision flag took 47.22 s. These are compile measurements with dependencies already
available, not fresh-clone/network measurements. Measurements for the other targets and billed CI
minutes are not available locally. The workflow uploads measurements and render artifacts per target.

| Target | Local evidence | Fresh clone including downloads / billed PR minutes |
|---|---|---|
| macOS arm64 | Source builds, tests and real hardware WebGL2 pass | Not measured |
| macOS x64 | CI matrix configured | Not measured |
| Windows x64 | CI matrix configured | Not measured |
| Windows arm64 | CI matrix configured | Not measured |
| Linux x64 | CI matrix configured | Not measured |
| Linux arm64 | CI matrix configured | Not measured |
| emscripten | Source-built module, CPU and hardware WebGL2 execution pass | Not measured |

Option A pays one native source build on a cold target cache, and one additional emscripten build
for browser integration. B would add archive production/storage/verification and avoid those compiles
on cache misses; no honest numerical comparison is possible before the other target runs. Archive
sizes/hosting are inapplicable to the chosen source-only pipeline. Bundled emoji adds 10,730,124 raw
bytes to hosts that link ParleyFonts; renderer WASM receives font bytes as side resources.

## Font policy and the retired bindings patch

CoreText, DirectWrite and fontconfig managers are disabled through GN. Both renderers require
FontMgr::custom_empty and never fall back to FontMgr::default. Skia exposes one unnamed dummy family
in this manager; it is not a system family. Every drawable typeface is created from FontSource bytes
and collection index. Parley alone shapes/wraps; Skia only draws the supplied glyph IDs and positions.
The adapter carries normalized variation positions through inverse avar v1/fvar mapping, synthesis,
underline and strike. Unknown avar versions fail explicitly. Nonuniform text strokes and strokes on
bitmap glyphs are explicit unsupported cases; aligned outline text strokes lower to filled paths.

ParleyFonts bundles pinned, unmodified Noto Color Emoji (OFL 1.1) after Inter as coverage fallback.
The source hash and licenses are beside the fonts; THIRD_PARTY_NOTICES records redistribution.
Skia/FreeType and codec library original notices are collected by scripts/skia-notices.mjs. HarfBuzz
is not linked by Skia; Parley's shaping dependencies retain their Cargo licenses.

With the production data-only GN configuration, unpatched bindings compile on SDK 27.0. The spike's
unbound libc++ alias is absent. Consequently the spike patch is **retired**, documented in
patches/skia-bindings/README.md, rather than applied to working output. A guard rejects its return.
No upstream issue was filed for a break no longer reproducible in this configuration. This deliberately
differs from the guide's requirement to ship a patch and file an issue; there is no production bug to
report. A future reproduction needs a new issue before another workaround is added.

A real pixel test exposed native NEON's low-precision raster pipeline versus WASM's high-precision
pipeline: one translucent emoji edge differed by 3 unpremultiplied levels. Both builds now define
SK_DISABLE_LOWP_RASTER_PIPELINE through extra_cflags_cc. The unchanged CPU comparison passes with
maximum delta 1. This retains the optimized high-precision implementation; it does not force scalar
Skia. General renderer throughput budgets are measured in Prompt 23.

## Renderer behavior

- One preparation pass validates the versioned display buffer and resources before drawing. Prepared
  scenes own Skia paths/typefaces/images. Failure preserves the browser's prior prepared scene/pixels.
- One command interpreter draws onto CPU, PDF and WebGL2 canvases. Transforms are absolute device
  matrices per ADR-015; clips retain the matrix at establishment. Opacity isolates a complete group.
- Rectangles/radii, elliptical arcs/rings, polygons, stars, lines, SVG paths with viewBox/fill rules and
  transformed boolean operands lower to Skia paths. Closed strokes lower to filled geometry for all
  exports, including align/cap/join/dashes. Zero width draws nothing. Per-side rectangular strokes use
  corner-to-center side regions; other shapes refuse nonuniform widths explicitly. Stroke dash phase
  starts at the same path origin for every side; corner bisectors separate widths.
- Images use centered cover (fill), contain (fit), or stretch. Crop is an affine transform in normalized
  destination coordinates applied after fitting. The destination remains clipped. Images and fonts are
  byte resources only; no URLs, file reads or platform lookup. Output/image dimensions have bounds.
- PNG retains transparency. PDF pages share the exact prepared drawing calls. The pure core SVG writer
  serializes lowered paths and embedded PNGs with XML escaping, absolute transforms, clips and groups.
  Outline text becomes paths; bitmap/colour glyph runs become PNGs. No SVG text requires a viewer font.
- Gradients, blend modes other than normal, effects and shaders return explicit errors until Prompt 23.
  They are never silently simplified. Raster/picture caching is also Prompt 23.

## Browser boundary and validation

The editor module is an emscripten **executable**, linking vellum-core and the renderer in one module,
with a thin TypeScript Renderer port. vellum-wasm's Rust library is rlib-only; the unknown-unknown smoke
uses cargo rustc --crate-type cdylib to retain version-export coverage. Export ownership is checked by
a buffer table, with resource/aggregate transfer limits and safe handling of unknown pointers/double
free. JS never passes draw commands individually. Font u64 handles are split into exact 32-bit halves.
Init refuses an ABI mismatch. Resize/DPR, disposal and WebGL context loss/restoration are exercised.

The local hardware comparison uses production binaries, a shared binary fixture, supplied Inter/emoji
bytes and an image. Native CPU vs WASM CPU: maximum channel delta 1. Native CPU vs Apple M5 Pro WebGL2:
p95 delta 0; 961/65,536 pixels (1.4664%) exceed 2, within ADR-001's 2% cross-backend limit. High deltas
are allowed only within that differing share (edges and very low-alpha unpremultiplied colours).
The actual Renderer contract and real lose/restore cycle pass. The contract also catches a backend
that drops ink. No six-target pixel-parity claim is made from this one host.

Hosted CI runs ordinary native behavior tests on six native architectures and a separate software
WebGL2 integration lane. Hardware GPU testing is opt-in workflow_dispatch on six configured
self-hosted vellum-gpu runners; provisioning these and obtaining green remote runs requires a person.
A missing hardware GPU is a failure in that lane, never a software pass mislabeled as hardware.

## Reference study

Penpot's rendering-architecture document and surfaces/shape_renderer/strokes/text/images paths separate
live GPU composition from vector export and stress shared primitive/order logic. Vellum keeps a single
prepared command interpreter, with typed preflight errors, and uses Parley glyph runs rather than a
second paragraph engine. Graphite's wasm_value documents lossless binary/BigInt transport; Vellum uses
checked buffers and exact handle halves on its C ABI. rust-skia's installed README/build sources verify
source/offline feature behavior and custom-empty availability. Sibling projects were searched; dubid's
React Native Skia wrapper supplies no reusable Rust/browser renderer. No reference code was ported.
