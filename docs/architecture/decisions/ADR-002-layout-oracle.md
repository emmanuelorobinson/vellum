# ADR-002: Layout oracle and sizing semantics: Chrome writes the layout goldens

- **Status:** Accepted (closes delta D2 of ADR-000). ARCHITECTURE synchronized on the owner's instruction (2026-10-16).
- **Date:** 2026-10-12
- **Written by:** Prompt 6 (Spike C).
- **Evidence:** `fixtures/layout-oracle/*.html` (51 fixtures), `fixtures/layout-oracle/expected/` (Chrome's boxes and
  `manifest.json`), `scripts/lib/layout-oracle.mjs` (parser, semantics, CSS mapping), `scripts/gen-layout-oracle.mjs`,
  `crates/vellum-testkit/src/layout_oracle.rs` (loader and comparison), `spikes/layout/` (Taffy computing the same
  fixtures), `docs/runbooks/layout-oracle.md` (how to add a fixture and regenerate).

## Context

Invariant 7 says auto-layout matches the browser, so a design becomes code without surprises. Guide §6.1 D2 proposes
that layout expectations come from Chrome, the way Taffy writes its own test suite, rather than from hand-written
numbers. Vellum's sizing model (ARCHITECTURE §4.1: frames with `layout: none | row | column | grid`, gap, padding,
justify, align; every node with `width`/`height` of a number, `hug` or `fill`, each with an optional fallback, and
`position: auto | absolute`) is not CSS, so the oracle needs a written mapping from one to the other. This ADR is that
mapping, plus the places where CSS's defaults disagree with what a designer means and how Vellum resolves each one.

## Decision

1. **Accept D2.** Layout goldens are Chrome's boxes. A fixture is an HTML file whose `<div>`s carry only Vellum
   properties as `data-*` attributes. `scripts/gen-layout-oracle.sh` parses it, resolves the semantics below, renders
   the CSS equivalent in headless Chrome and writes each node's border box (relative to its parent's border box,
   unrounded, in CSS px) to `fixtures/layout-oracle/expected/<name>.json`. Engines compare with
   `vellum_testkit::layout_oracle::compare` at **0.5 px**.
2. **The semantic resolution lives in one place per side**, before any CSS or Taffy style is built:
   `resolveTree` in `scripts/lib/layout-oracle.mjs` and `resolve` in `spikes/layout/src/lib.rs` (Prompt 18 moves it
   into `vellum-core` so the layout adapter, the HTML export and the inspector share it). CSS and Taffy then only
   express an already-resolved `fixed | hug | fill`.
3. **The committed boxes are the contract**, not the Chrome version. `--check` regenerates and compares within
   0.5 px, so a Chrome update that moves nothing needs no commit; one that moves a box fails CI and is reviewed.
4. **Chrome runs over `--remote-debugging-pipe`.** The generator opens no TCP port (AGENTS.md: no network listener),
   needs no npm dependency, and reads Chrome from `VELLUM_CHROME` or the platform path.

### Reference: Taffy's gentest (guide §3.2 R3)

Taffy's `scripts/gentest` drives Chrome through ChromeDriver, loads each `test_fixtures/**/*.html` with
`test_helper.js` and `test_base_style.css` (Ahem font, border-box, `display: flex` on every div), reads the inline CSS
back into a style description plus `getBoundingClientRect` boxes, and writes Rust tests and XML, for border-box and
content-box, LTR and RTL. **Vellum keeps** the core idea (Chrome's boxes are the expectation), parent-relative
unrounded boxes, and one fixture per behaviour. **Vellum changes** the input: fixtures are written in Vellum's
semantics, not CSS, because the thing under test is our mapping as much as the engine; output is JSON loaded by
`vellum-testkit`, not generated Rust; only border-box LTR (Vellum has no content-box and RTL is out of scope here); and
CDP over a pipe instead of ChromeDriver, so there is no WebDriver download or port. No code was ported.

## The CSS mapping

Every element gets `box-sizing: border-box; margin: 0`. The page has `html, body { margin: 0; padding: 0 }`.

### Frames (containers)

| Vellum | CSS |
|---|---|
| `layout: none` (default) | `display: block`; every child is placed at its `x`, `y` (below) |
| `layout: row` / `column` | `display: flex; flex-direction: row \| column; flex-wrap: nowrap` |
| `layout: grid`, `columns: N` | `display: grid; grid-template-columns: repeat(N, minmax(0px, 1fr))` |
| grid `rows: R` | `grid-template-rows: repeat(R, minmax(0px, 1fr))`; without `rows`, `grid-auto-rows: auto` |
| `gap: g` | `gap: g px` (both axes in a grid) |
| `padding` 1, 2 or 4 values | `padding` with the same values in CSS order (2 values: vertical, horizontal; 4: top, right, bottom, left) |
| row/column `justify: start \| center \| end \| space-between \| space-around` | `justify-content: flex-start \| center \| flex-end \| space-between \| space-around` |
| row/column `align: start \| center \| end` (default `start`) | `align-items: flex-start \| center \| flex-end` (always written) |
| grid `justify` / `align: start \| center \| end` | `justify-items` / `align-items` with the same keyword (places items inside their cells) |
| `clip: true` | `overflow: hidden` |
| `visible: false` | `display: none` |

### Children: sizing per axis

| Resolved sizing | In a row/column, main axis | In a row/column, cross axis | In a grid cell | Root, or positioned child |
|---|---|---|---|---|
| fixed `n` | `width: n px; flex-shrink: 0` | `width: n px` | `width: n px` | `width: n px` |
| hug | `width: fit-content; flex-shrink: 0` | `width: fit-content` | `width: fit-content` | `width: max-content` |
| fill | `flex: 1 1 0px; min-width: 0` | `align-self: stretch` (size auto) | `justify-self: stretch; min-width: 0` (size auto) | not possible: resolves to the fallback |

(`height`, `min-height` and `align-self` for the vertical axis.)

### Children: position

| Vellum | CSS |
|---|---|
| root (a top-level frame on the canvas) | `position: absolute; left: 0; top: 0`, laid out in max-content space |
| child of a `layout: none` frame, or `position: absolute` inside a layout | `position: absolute; left: x px; top: y px`; out of the flow, ignores padding, gap and alignment |
| any other child | `position: relative` (in the flow) |

### Taffy (the spike, and Prompt 18's starting point)

The same table, property for property, with two differences in spelling: Taffy has no `fit-content` size keyword, so
hug is `size: auto` and the explicit `align-items`/`justify-items: start` keeps it from stretching; and the root is
computed with `AvailableSpace::MaxContent` on both axes with rounding disabled. With taffy 0.14.0 that matches Chrome on
every fixture, including absolute hugging children.

## Semantic mismatches found, and how Vellum resolves them

These are the places where CSS, left to its defaults, does something a designer would not expect from hug, fill and
fixed. Each is resolved in the semantics or the mapping, and each has a fixture.

| # | CSS default | What Vellum means | Resolution | Fixture |
|---|---|---|---|---|
| 1 | `align-items: stretch` stretches every child on the cross axis | children sit at the start unless they fill | `align` defaults to `start` and is always written | `row_align_start`, `row_fixed_children` |
| 2 | `flex-shrink: 1` shrinks fixed-width children when a row overflows | a fixed size is fixed | `flex-shrink: 0` on fixed and hug children | `row_overflow_fixed_no_shrink`, `row_justify_center_overflow` |
| 3 | `flex-grow: 1` with `flex-basis: auto` shares only the space left after content | fill children share the space equally | `flex-basis: 0px` | `row_fill_equal_split`, `row_fill_thirds` |
| 4 | `min-width: auto` keeps a flex item at least as wide as its content | fill ignores what it contains (content overflows, or is clipped) | `min-width: 0` (`min-height` in a column) on fill | `row_fill_ignores_content`, `clip_does_not_change_layout` |
| 5 | a flex item with basis 0 inside a `fit-content` container is a cycle CSS resolves in its own way | fill on the main axis of a parent that hugs that axis has nothing to fill | the child uses its fallback, else hugs | `fill_main_in_hug_parent_uses_fallback`, `fill_main_in_hug_parent_hugs` |
| 6 | `stretch` on the cross axis of a hugging parent stretches to the widest sibling | the same: a hug-width menu with fill-width items | kept as CSS does it | `column_fill_cross_in_hug_parent` |
| 7 | fill outside a flex or grid container has no meaning | the root, children of a `layout: none` frame and absolute children have nothing to fill | fill uses its fallback; without one the fixture is rejected (Prompt 7/11 validation should reject it too) | `fill_root_uses_fallback`, `absolute_fill_uses_fallback`, `layout_none_fill_uses_fallback` |
| 8 | a block with only absolute children has content size 0, so it "hugs" to nothing | hug needs auto-layout | hug on a `layout: none` frame uses its fallback; without one it is rejected | `hug_layout_none_uses_fallback` |
| 9 | `fit-content` clamps to the containing block, so a top-level or absolute frame that hugs would wrap at the viewport or parent edge | the canvas has no edge; an absolute child is outside its parent's layout | `max-content` for the root and for positioned children | `row_hug_both_axes`, `absolute_hug_child` |
| 10 | `justify-content: space-*` on a grid moves tracks, which `1fr` tracks leave no room for | justify/align in a grid place items in cells | grid `justify`/`align` map to `justify-items`/`align-items`; `space-between`/`space-around` on a grid are rejected | `grid_items_align_center`, `grid_items_end` |
| 11 | absolute `left`/`top` are measured from the padding box (inside the border) | `x`, `y` are measured from the frame's outer edge | the same as long as nothing maps to a CSS border; strokes do not take layout space today (see "Not covered", `layoutIncludesStroke`) | `absolute_child_in_padded_column`, `layout_none_children_positioned` |
| 12 | `overflow: hidden` changes a flex item's automatic minimum size | clipping is drawing, not layout | neutralised by 2 and 4 (`flex-shrink: 0`, `min-* : 0`) | `clip_does_not_change_layout` |
| 13 | `space-between` with one item puts it at the start; `space-around` centres it | the same | kept | `row_space_between_single_child`, `row_space_around_single_child` |
| 14 | centred content that overflows spills out on both sides (unsafe alignment) | the same; Taffy does the same | kept | `row_justify_center_overflow` |
| 15 | hidden (`display: none`) children take no space and no gap | the same | `visible: false` maps to `display: none`; the oracle records no box for them | `hidden_child_excluded` |

## Taffy against the oracle

taffy 0.14.0 (crates.io), Chrome 155.0.8059.39 (macOS 27, headless), 51 fixtures, 0.5 px tolerance:

| Fixtures | Visible nodes compared | Within 0.5 px | Largest difference |
|---:|---:|---:|---|
| 51 | 186 | 186 | 0.0104 px (`grid_three_columns_gap_padding` node 0.2: Chrome rounds to 1/64 px layout units, Taffy keeps f32) |

`spikes/layout/known-mismatches.txt` is empty. Mutating the spike's mapping (default `flex-shrink`, `flex-basis: auto`)
makes 11 nodes in 4 fixtures fail, so the comparison is not vacuous.

Notes for Prompt 18:

- taffy 0.14.0 on crates.io spells alignment constants `AlignItems::FLEX_START`; the pinned `refs/taffy` commit
  (newer than 0.14.0) and its `docs/style-properties.md` use `AlignItems::FlexStart`. Read the installed crate.
- Taffy reports `location` relative to the parent's border box, like the oracle; rounding must stay off for the
  comparison (`TaffyTree::disable_rounding`). Whether the editor rounds for display is Prompt 21's call.

## Not covered yet (the guide grows the suite to at least 150 fixtures in Prompt 18)

- **Text.** No fixture has text. Hugging text needs a measure function from `TextEngine` (Prompt 19) and a font both
  sides agree on; Taffy's suite uses the Ahem font for this. The oracle page loads no fonts today.
- **`layoutIncludesStroke`.** ARCHITECTURE lists it but does not say what it includes (the frame's own stroke as extra
  padding, or children's outside strokes as extra size), nor how inside, center and outside strokes count. It needs a
  definition before it can be mapped; Prompt 18 owns it.
- **Grid tracks.** ARCHITECTURE gives a grid no track definition. The spike assumes `columns`/`rows` counts with equal
  tracks and auto-placed items; Prompts 7–8 should define the real fields (spans, explicit placement, track sizes).
- **Padding with 2 values** is assumed to be vertical then horizontal (CSS order). Prompt 7 fixes it in the types.
- Not in ARCHITECTURE's model and so not tested: wrap, min/max sizes, percentages, aspect ratio, baseline alignment,
  RTL, absolute children constrained by right/bottom, rotation's effect on layout (none expected).

## Revisit when

- A fixture shows a Vellum behaviour that its CSS equivalent cannot express (the resolution then has to be written
  as Vellum-only semantics, and the oracle cannot check it).
- The oracle stops being reproducible in CI, or a Chrome update moves committed boxes for reasons that are not a
  layout change.
- Taffy gains mismatches that cannot be fixed in the mapping (list them in `spikes/layout/known-mismatches.txt`
  first; more than a handful means this decision, or the engine, is revisited).
