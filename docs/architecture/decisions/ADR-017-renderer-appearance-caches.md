# ADR-017: renderer appearance, caches and basic fallback

Status: implemented for Prompt 23; other platforms and target-class performance need remote verification. ARCHITECTURE synchronized on the owner's instruction (2026-10-16).
Date: 2026-10-15. Supplements ADR-015 and ADR-016. Document format and display-list version remain unchanged.

## Appearance and exports

Normalized gradients use shape ink bounds (stroke bounds for zero-area geometry). Linear runs from
(0, .5) to (1, .5); radial uses centre (.5, .5), radius .5; angular starts on the positive x axis,
clockwise in downward-y coordinates. The supplied affine maps this unit gradient before the shape
bounds map. sRGB stops interpolate unpremultiplied color/alpha, clamp outside their range and retain
coincident stops. Empty/unsorted/out-of-range stops and singular transforms fail preflight.
Consecutive glyph runs with the same paint/stroke share union ink/decorations bounds, preventing a
rich-text or font fallback boundary from restarting the gradient.

Meshes lower to shared-boundary Coons patches. Points/handle offsets are normalized to shape bounds;
handles are left/right/up/down, absent handles use straight cubic thirds. Corners interpolate RGBA.
At least 2×2 points and exact row-major cardinality are required; grids are capped at 65,536 points.
All 18 document blend modes lower to Skia. Linear dodge uses Plus; linear burn uses an internal fixed
premultiplied-alpha blender. This fixed implementation accepts no document shader source.

Effects configure the immediately preceding layer, before its content. Ordered foreground filters
implement drop/inner shadows, signed spread (morphology), offset, tint, blend, and layer blur. Blur
radius maps to Gaussian sigma = radius / 2. Negative blur fails. Background blur samples the previous
destination and masks output to the union of transformed layer geometry and clips. Bitmap glyph masks
use font bounds; ordinary glyph masks use outlines. Outer effects can extend beyond this silhouette.
Shader paints/effects explicitly refuse in the v1 renderer; document-supplied runtime shaders remain
Prompt 67/v1.1. No shader source is evaluated or loaded here.

GPU filter kernels differed from CPU rasterization beyond the existing per-fixture thresholds. The
shared CPU compositor therefore rasterizes filtered **whole viewport scenes**, then the WebGL path
uploads/composites those same tiles. This retains pixel parity and caching but trades first-render
CPU time for consistency; GPU-native effects are deferred until they pass these fixtures. The ordinary
path/stroke/gradient/mesh/blend interpreter still executes through WebGL. CPU/PDF share the interpreter;
PDF composites to its page canvas, using the same raster tiles for filtered scenes. SVG retains vectors for the previous simple feature set and
embeds a complete viewport PNG for gradients/mesh/effects/non-normal blends. It preserves appearance,
but those SVGs are raster content rather than editable gradient/filter primitives (export work: P41).

## Cache ownership and limits

`Renderer::prepare_frame(id, buffer, resources)` reuses an immutable prepared scene only on exact
buffer **and font/image byte/index equality**. Eight frame entries and 32 MiB of retained input bytes
are the limits, evicted in deterministic insertion order. An invalid replacement preserves old entries.
`prepare_buffer` supplies the viewport id; hosts may partition streams by frame id. This is not automatic
reuse of individual document subtrees from a changed full-page buffer.

A scene records a Skia picture for its device viewport. Repeated draws reuse it; size changes recreate
it. Below 50% zoom, an isolated viewport can reuse 512-pixel raster tiles (16 MiB retained pixels per
scene). Filtered scenes also use tiles at other zooms; larger filtered viewports release them after
compositing. Tiles are cut from one complete filtered surface, so there are no independently filtered
seams. The transient raster surface retains the existing 64-megapixel/16,384-axis limit. Host-held scene
clones may outlive the input cache. These are input/pixel caps, not a total process-memory guarantee.

`Frame.zoom` is only a cache hint; model→device coordinates already include zoom and DPR. Changed pan,
zoom coordinates, resources or scene bytes invalidate conservatively; this does not claim world-tile
reuse across changed display buffers. Context replacement drops GPU ownership; retained CPU scenes can
rebuild resources. Cache stats and equivalence/invalidation tests make reuse observable without clocks.

## Canvas2D (S02)

The fallback needs no WebGL context. A dedicated instance of the same WASM module validates the VDL,
lowers paths/strokes/booleans, decodes supplied images and rasterizes supplied-font glyphs. Canvas2D
independently executes paths, affine sampled gradients, clips, images, blend modes and group opacity.
The compiler is injectable, providing the reference-fake seam for tests. There is no system-font,
network or file lookup, or adapter-to-adapter dependency. The fallback still needs the geometry/font
WASM module; it is not a fallback for a missing or incompatible WASM toolchain/runtime.

Basic rendering omits effects, averages mesh point colors, shows shader paints as transparent, and
uses normal in place of linear burn. The returned `simplifications` enumerate these choices. Gradient
sampling is capped at 2,048 pixels per axis; text raster scale is capped at 8×. Group compositing uses
transparent offscreen surfaces; drawing stages the whole result before replacing the last good frame.
Hosts expose S02 using config's `basicRenderer` copy; exports always use the full native renderer.
No chrome/banner UI is introduced before Phase 6.

## Golden policy and performance

31 original fixtures cover shape/text/image/stroke compatibility, gradients, handled meshes, every
blend, drop/inner shadows, signed spread, both blurs, transformed clips and grouped opacity. Each runs
against committed native RGBA, browser CPU, browser WebGL and the documented basic native projection
versus Canvas2D. The tolerance file is checked for exact fixture coverage. Cross-platform comparison
uses premultiplied color plus alpha (the maximum channel delta per pixel): invisible RGB cannot create
a visual difference. Corruption/alpha tests prove the gate fails. Numeric thresholds were retained
while fixing implementation failures. The older P22 raw-RGBA gate and context recovery stay intact.
Native expected inputs, browser actual RGBA and magenta PNG diffs remain in target/renderer-goldens.
Recording is explicit, never performed by tests or CI.

See docs/runbooks/renderer.md for the commands and measured §12 budget table. This session verifies
macOS arm64 and browser WASM; it does not certify six-platform CI, M1/mid-range Windows frame budgets,
or future host input/startup latency. Hardware timings explicitly finish the GPU before stopping the
sample clock. Software WebGL results are integration evidence, not hardware performance claims.

## Reference study

Penpot's fills, shadows and filters paths use normalized paints, ordered filters and shared offscreen
surfaces, while its visual-regression guide emphasizes waiting for completed rendering before capture.
Vellum keeps those concerns, but uses its own VDL, explicit preflight failures and directly compared
headless/editor artifacts; no reference implementation or text was ported. Vello's current architecture
includes CPU/GPU Sparse Strips and WebGL2, but ADR-001's host support, parity and same-machine timing
conditions have not all been established. Sibling cutesycal sprite logic has no reusable general renderer.
