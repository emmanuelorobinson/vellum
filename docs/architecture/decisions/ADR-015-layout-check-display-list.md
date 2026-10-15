# ADR-015: structured layout checks and the display-list boundary

Status: implemented in Prompt 21; architecture rendering sketch update needs a person.

## Decision

Layout inspection consumes resolved occurrence trees and parent-relative layout output. It never
changes a document. Reports contain world-space boxes and typed problems, all addressed by
`NodeIdPath`. `problems_only()` retains each diagnostic's participants in tree order. The later
`snapshot_layout` tool can consume the report without running another layout implementation.

Text overflow comes from the same TextEngine box layout used by the display-list producer.
Clipping checks full child boxes against every clipping ancestor, including rounded corners and
transforms, so a completely hidden child is still reported. Overlap checks compare positive-area
intersections of visible flow siblings in row, column and grid frames; absolute children and touching
edges are excluded. Intersections below 1e-9 square pixels are treated as numerical noise.
A non-frame occurrence is outside any frame when no visible frame contains its complete transformed
box, regardless of structural membership. Frames themselves are exempt. Hug-cycle diagnostics retain
the layout engine's occurrence pair and axis. Missing or unresolved geometry returns an explicit error.

The display-list producer resynchronizes the supplied Scene before querying its index. It retains
ancestor state for visible descendants, including children outside unclipped parents. Strokes, shadows,
blurs and text overflow conservatively expand the query. When an outset exists it queries temporarily
unclipped scene bounds, then restores the ordinary scene before emitting commands: a clipped-away box
may cast a shadow back into the clip. SVG paths and runtime shaders disable culling because their
paint bounds are not established by the existing ports. This favors complete output over underdrawing.

The shared `vellum-types::display` module owns `DrawCmd`, renderer resources and
`DISPLAY_LIST_VERSION`. Geometry and appearance scalars crossing the boundary are f32, while integer
identities remain integers. Transforms **set the absolute local-to-device matrix**. The producer
accumulates world transforms and subtracts the viewport origin in f64 before casting, preserving small
offsets far from the canvas origin. Clips already established remain in device space. `Layer` pushes
one isolated state, configured by its immediately following effects; `Restore` composites it.
Opacity and blend apply to the whole subtree, rather than separately to each primitive.

Path references are typed shape resources (including SVG data and boolean operands), rather than
vendor paths. This differs from ARCHITECTURE's abstract `PathRef`: the current PathOps port neither
parses SVG nor converts primitive shapes. Renderers lower these descriptors through their geometry
implementation in Prompts 22–23. No backend drawing, icon lookup or collaboration-node chrome is
implemented here. Unsupported nodes and missing font bytes are explicit output problems.

Glyph resources retain exact font/glyph/cluster identities, variable axes, synthesis, baseline-adjusted
origins, advances, decorations and links. Used font bytes are shared as immutable FontData beside the
buffer; the host registers them with the renderer. Images retain asset identity, fit and crop. Ordered
fills, strokes, effects and blend modes survive the binary boundary. No default text paint is invented.

## Binary ABI

The owned byte vector can be exposed as one WASM memory view without JSON or per-node host calls.
Fixed command opcodes refer to first-use-ordered resources. Resources have a deterministic typed
binary value encoding; serde_json::Value is used internally as an owned schema bridge, never emitted
as JSON text. See `docs/runbooks/display-list.md` for the layout. Version 1 is separate from the
`.vellum` file version. Any change to opcodes, resource schemas or semantics requires a version bump.
The decoder rejects unknown versions, malformed lengths/UTF-8, invalid resource references, unbalanced
state, nonfinite wire scalars and trailing bytes. Input is limited to 64 MiB and resource nesting to 128.

## Reference study

Excalidraw's `packages/excalidraw/renderer/staticScene.ts` brackets drawing with saved state and frame clips.
Vello's `ARCHITECTURE.md` separates draw/transform/resource encoding from graphics execution.
Vellum keeps these separations, uses occurrence addresses and its SpatialIndex/TextEngine/FontSource ports,
and defines its own ABI and resources. No reference code was ported. Sibling searches in `2d` and `clarum`
found no reusable layout-check or Rust display-list implementation; the existing renderer spike informed
buffer transport, but its Float32Array integer indices and string-text commands are not used here.

## Limits and revisit conditions

No renderer pixel comparison or WASM runtime execution is claimed. Text culling uses the text port's
line/advance extents, not font-outline ink bounds. Stroke outsets reserve four times the greatest side
width (the renderer must use a miter limit at most four); blur outsets reserve three blur radii.
Large documents with text/effects may overdraw. A dedicated paint-bounds index, cached text output,
resource interning and a compact resource codec should follow measurements rather than a new vendor
format in core. WASM memory allocation/transport and actual renderer consumers belong to later prompts.
