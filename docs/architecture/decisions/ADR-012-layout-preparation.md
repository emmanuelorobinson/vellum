# ADR-012: Production layout preparation, stroke space and incremental Taffy

- **Status:** Implemented; choices awaiting human architecture review.
- **Date:** 2026-10-14
- **Evidence:** `crates/vellum-core/src/layout/`, `crates/adapters/layout-taffy/`, the 186 Chrome oracle fixtures,
  `scripts/lib/layout-oracle.mjs`, the LayoutEngine contract and incremental/fresh property tests.

## Decisions

Core builds `LayoutTree` from Prompt 17's resolved occurrences and prepares ADR-002 dimensions before a vendor
style is built. The layout port now uses `NodeIdPath` for ids, dirty entries and result boxes: a definition's same
node used in two instances must keep two boxes. The file format does not change. Hidden subtrees have no boxes.
Malformed input yields typed diagnostics and no boxes; `try_layout` also returns the typed error directly.

Grid keeps the existing `columns`/optional `rows` counts: equal `minmax(0, 1fr)` tracks, auto placement, auto rows
when rows are absent. There are no spans or explicit placement fields in the current format. Counts must fit
Taffy's repeat count (1–65535). Grid space-between/space-around is rejected, as ADR-002 already requires.

`layoutIncludesStroke` reserves **this frame's own inward stroke alongside padding**. Inside reserves the full
per-side width, centered reserves half, outside reserves zero. Children's strokes do not grow the parent's
layout measurements. Fixed/fill dimensions include this space; hug dimensions add it to content. Absolute child
x/y remain relative to the parent's outer edge. This was undefined in ARCHITECTURE and the types; no brand or UI
choice was made. Chrome snaps borders to whole pixels, so the oracle uses an integer border plus the fractional
remainder as padding. Taffy uses the exact inward border widths, with compensated absolute insets.

A main-axis fill under a hugging flex parent is a sizing dependency cycle. ADR-002 breaks it using its fallback,
or by hugging its content; the latter now emits `HugCycleBroken` with parent/child/axis. A free frame that has
nothing to hug, or fill with nothing to fill, requires a fallback. An empty auto-layout frame uses a provided hug
fallback; otherwise its padding/stroke determine its intrinsic size. Cross-axis stretch and intrinsic grid sizing
remain the browser behavior ADR-002 accepts; they are not refused as cycles.

The adapter keeps one Taffy tree and updates changed own styles, text and topology. Changed topology detaches old
child lists before attaching new ones so reversing old ancestry cannot create a temporary cycle. Dirty entries
invalidate measurement even without a document edit (e.g. a font becoming available); `dirty.all` invalidates every
node. Style/structure comparisons also detect edits omitted from a dirty set. Preparation and reconciliation scan
all inputs; Taffy's cached computations skip clean subtrees, while dependent ancestors/siblings are recomputed.
A caller changing its TextEngine/font configuration must dirty the affected text, or pass `DirtySet::all()`.

## Reference study (guide §3.2 R3)

Taffy's flexbox and grid modules separate intrinsic sizing, available space, alignment and out-of-flow placement.
Its gentest drives Chrome, captures parent-relative boxes and uses a deterministic font for intrinsic text.
Vellum keeps those box comparisons and Taffy's cached layout, using Vellum-semantic inputs and our injected text port.
Vellum retains CDP over a pipe and unrounded border boxes; no reference code was ported. The old spike entry point
now delegates to production, so it cannot silently test a different mapper. Sibling Rust layout files were searched;
no reusable implementation outside this repository was found. Context7 and installed Taffy 0.14 API were checked.

## Limits and follow-up

TextEngine measurement, wrapping and invalidation are covered by injected metrics. Browser text/font fidelity is
Prompt 19; the Chrome corpus contains geometry only. Model values remain f64; Taffy computes f32 with rounding
disabled. Individual inputs that do not fit f32 are refused. Large-document timings are not a Prompt 18 claim.
A person must reconcile the stroke meaning, grid restrictions and port additions with read-only ARCHITECTURE.
