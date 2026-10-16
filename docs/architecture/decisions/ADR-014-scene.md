# ADR-014: occurrence-addressed scene queries

Status: implemented in Prompt 20; architecture signature update needs a person. ARCHITECTURE synchronized on the owner's instruction (2026-10-16).

## Decision

The pure core owns one scene per page. Hosts supply resolved roots and parent-relative layout boxes
through `scene::from_resolved`, then reconcile that prepared snapshot with `Scene::sync`. Scene
snapshots and session reservations never mutate the document. All document edits remain Ops.

`SpatialIndex` keys are `NodeIdPath`, matching layout and resolved component occurrences. A bare
`NodeId` conflates descendants shared by two instances. The rstar 0.13.0 adapter bulk-loads on
`rebuild`, removes old envelopes before incremental inserts, and keeps an address-to-bounds map.
Duplicate input addresses use the last entry; removal wins over change; malformed bounds are omitted
(and remove an old entry on update). Invalid queries return no candidates. The fake shares this contract.
Scene input instead returns typed errors atomically for duplicate addresses, missing parents, cycles,
missing layout, unresolved rotation/radii and invalid geometry.

## Query semantics

- Scene reconciliation scans the supplied page and recomputes transforms and clips; only changed
  envelopes and removals reach the index. Changes to hierarchy, drawing order, lock or clip metadata
  still take effect when an envelope is unchanged. Roots and siblings follow their supplied order.
- Point and marquee selection use transformed layout boxes. Deep selection returns the frontmost
  eligible occurrence; top-level selection maps a hit to its page root, even when overflowing that
  root's unclipped box. Marquee supports intersection and full containment of the visible clipped box.
- Visibility, locking and selection eligibility inherit through the tree. Locked content remains in
  viewport culling and occupies canvas space. Nonselectable session placeholders reserve space without
  becoming selectable or snap targets. Culling returns paint order, selection returns front to back.
- Child clips intersect through all ancestors, including rotation and reflection. Rounded clips use
  CSS's shared corner-radius reduction and convex arc segments with a 0.01 px sagitta target, capped
  at 128 segments per corner. Zero-area clipping frames show no descendant content. Borders count.
- This is bounds-based selection, not fill/stroke/glyph coverage: ellipse corners and transparent
  vector interiors can select the layer. No SVG parser, PathOps implementation or renderer is added.
  Precise shape coverage should extend the narrow phase behind the geometry port when that is needed.
- Snapping gets candidates only from the index, within a caller-supplied search region (normally the
  viewport). Tolerance is in screen pixels divided by zoom. It supports edge pairs, centre pairs,
  equal gaps between or beyond adjacent nonoverlapping neighbors, and an optional model-pixel grid.
  Selected subtrees are excluded. Per-axis distance wins, then kind and occurrence address; grid
  snaps the moving box's origin. Guides carry axis, kind, target addresses, coordinate and gap data.
- `find_space(width, height, near?)` starts at the requested top-left or the origin. Indexed visible
  bounds, including locked layers and reservations, block placement. Best-first obstacle-edge probes
  use a 16 model-pixel gap with distance/y/x ties. This is deterministic, not a globally nearest-space
  guarantee. At 10,000 probes it returns `SearchLimit`, never an occupied fallback.

## Reference study

Excalidraw separates broad bounds, rotated narrow-phase tests, visible scene membership and overlays.
Graphite separates snap candidates, alignment/distribution/grid targets and screen-to-model tolerance.
Kurbo exposes curve bounds, winding, nearest-point and bounded approximation rather than canvas state.
Vellum keeps that separation, uses occurrence paths and its SpatialIndex port, and implements convex
box clipping independently; no reference code was ported. Guide R7's `scene/Renderer.ts` moved to
`packages/excalidraw/scene/Renderer.ts` at the pinned reference; that file was read. Sibling-project
searches found no reusable Rust scene/index implementation.

Context7 returned no matching rstar library. [Official docs.rs](https://docs.rs/rstar/0.13.0/rstar/struct.RTree.html) and the downloaded 0.13.0 source were
used to verify the adapter API; envelope query arguments are by value in that version.

## Alternatives and revisit conditions

A per-document-ID index was rejected because repeated instances alias. A flattened world AABB-only
scene was rejected because rotated clipping would accept empty corners. Rebuilding on every edit
would lose incremental index work. A global snap scan would bypass the port and grow with document
size instead of the visible candidate set.

Revisit when scenes need exact curve/stroke coverage, large deeply clipped trees exceed the budget,
free-space exhaustion is common, or measurements justify caching world transforms by dirty subtree.
The release baseline test is opt-in, requires no external services, and reports timings rather than
using machine-dependent pass thresholds.

Run the local 5,000-node baseline with:

```sh
cargo test --offline -p vellum-index-rstar --release --test scene scene::baseline_5000 -- --ignored --nocapture
```
