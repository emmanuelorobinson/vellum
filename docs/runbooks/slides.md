# Slides and presenting (Prompt 68)

A slides page stores its order in the ordinary top-level child array. Only frames are slides;
notes, context and prompts stay editing annotations. There is no alternate deck format or slide store.

From Layers, choose **New page → Slides**, then draw frames. The strip numbers frames in document
order. Click to select and fit one; Command/Ctrl/Shift-click toggles selection. Drag a card above or
below another, or focus it and use Alt+Up/Down, to reorder. One person-origin Reorder Op is one undo
step. Creation uses InsertPage. Locked frame content can still be presented; read-only disables
creation/reorder but permits presenting/export. The page selector returns to other pages.

**Present** or Command+Enter (Ctrl+Enter on Windows/Linux) starts at the selected frame (or the slide containing a selected child), or the first
frame. Left/Right, Space and Enter navigate; endpoints clamp. Escape or Stop presenting exits.
The native modal dialog isolates the editor while its selection, viewport, history and document stay
untouched. These keys use the existing rebindable presentation scope. Presentation shows only an
isolated PNG from the shared export renderer. Loading, unavailable, stale and failed pixels are
explicit; previous slide pixels are never relabelled as a newer slide. The view fills the host window
with aspect-fit pixels; it does not change the OS display/fullscreen mode.

**Export PDF · slide order** writes one PDF of every frame. **Export selected slides** writes only
selected slide frames, in deck order, irrespective of selection order. Annotations are excluded.
Each frame's laid-out width/height is its PDF page size, including fractional values within Skia's
f32 precision. Frames on ordinary canvas pages retain selection order. Multiple slide decks retain
the order in which each deck first appears in the request, and each deck keeps its own slide order.

The existing CLI/MCP export path handles this too:

```sh
vellum export deck.vellum --format pdf --output slides.pdf
```

See `vellum export --help` for this version's selection/destination arguments. MCP uses `export`
with `nodes: [pageId]` for all slides, or frame ids for a subset, and `format: "pdf"`. It returns
bytes without writing files. Agent file writes still require PermissionBroker authorization. Existing
100-frame, dimension, total-pixel, bundle and synchronous tool-result budgets apply. Export fails
explicitly for incomplete rendering, missing sources or excessive output; it does not silently omit pages.

Skia rounds physical PDF device bounds at 72 dpi. The renderer keeps that raster budget and clips and
offsets each page to its logical top edge; `lopdf` corrects fractional MediaBox metadata in bounded,
internally generated PDF bytes. It never opens foreign PDF files. This preserves the prepared scene,
fonts/images/shaders and PDF vector content without increasing filter-raster memory through higher DPI.

## Host integration

`EditorWorkspace` creates/disposes a framework-independent SlidesController and takes optional
`nextPageId` and `slidesHost` props. A supplied page id generator must use the host's ordinary injected
id source. Without it, the workspace uses its existing node id source. All writes go through
`session.commit`, including multiplayer role restrictions.

Use `slidesHostFromExport` to bridge the live document's existing export service: its `render` callback
calls the shared exporter with `{nodes, format, scale: 1}`; `revision` reads the authoritative live
revision; `savePdf` opens the host destination UI and saves the one PDF. Revision checks run before
and after rendering, before handing bytes to the writer. The host must bind that exporter to the
same live document, including unsaved changes, and keep renderer resources available. No disk-only
fallback is safe for a live edited document.

**The installed desktop/extension runtime still awaits Prompt 55's shared document ABI/EditorLink.**
This prompt does not claim those installed hosts can author, present or save a live deck. Their slide
render/export callbacks remain unavailable; the reusable bridge, UI behavior and native CLI/MCP PDF
path are built. Do not advertise native presentation until that runtime and destination integration
pass installed-host acceptance.

## Validation

- `pnpm --filter @vellum/editor-core --filter @vellum/editor-react --filter @vellum/engine-client test`
- `CARGO_TARGET_DIR=target/prompt63-verification cargo test --offline -p vellum-core slide_order_uses_attributed_history`
- `CARGO_TARGET_DIR=target/prompt63-verification node scripts/skia-build.mjs test --offline -p vellum --test exports --test mcp -p vellum-render-skia --test renderer`
- Chrome lane: `pnpm --filter @vellum/editor-react exec playwright test --config e2e/playwright.config.ts slides.spec.ts --workers=1`

The browser fixture is marked TEST DATA and uses scripted core replies and a recorded PNG boundary.
It verifies actual workspace navigation, PDF requests, reorder attribution, read-only restrictions,
focus restoration and stale asynchronous responses, not installed-host rendering. Native tests verify
real PDF bytes/page sizes and ordinary core history verifies creation/reorder undo/redo. C32 strip
and action crops compare to exported design PNGs at the unchanged 1% gate; missing PNGs are explicitly
skipped in CI. Full C32 artwork and new states are not claimed as design-matched. See reports.md for
measured results, environment limitations and remaining release acceptance.
