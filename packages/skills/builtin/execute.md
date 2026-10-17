# Execute API

Read format.md and the relevant workflow first. Call get_app_state to learn the active document, pages,
selection, Notes for agents, context, leases and paused agents. Use list_components and get_variables to
reuse the design system. Read only the scope needed for the task.

The injected `doc.apiVersion` is "1.0". Read vellum-doc.d.ts for the generated signatures of every method:
page, get, query, insert, update, copy, replace, move, delete, setVariables, createComponent, makeSlot,
instance, findSpace, layoutProblems, screenshot, export, fillImage, icon, claim and release.
`doc.page` finds an existing page by id or unambiguous name; it does not create one.
`doc.query` matches type and name exactly; the typed query_nodes tool has broader filters.

An execute call runs synchronously on a scratch document and commits its resulting ops as one transaction
and undo step. A failing call applies nothing. `validate_only: true` reports the proposed changes without
committing them or their leases. Return a JSON-compatible value explicitly; to see a picture, return
doc.screenshot(id, {scale:1}). The tool puts a returned picture in its image field and other data in value.

Limits: 5 seconds, 64 MB, 2,000 doc calls, 2,000 new layers and 100,000 source bytes. The final transaction
also obeys the Guard's limits (currently 1,000 ops and 1,000 inserted layers), so prefer small batches.
Scripts have no network, filesystem, imports, timers, async work or host objects. Math.random is seeded;
use Math.pow rather than the unsupported ** operator. Keep output bounded; do not return entire documents.
Use JSON-compatible data, not functions or cycles.

This example assumes an existing page named Screens. Replace it with a page id from get_app_state:

```javascript
const page = doc.page("Screens");
const frame = doc.insert(page, {
  type: "frame", name: "Summary", width: 360, height: 180,
  layout: "column", gap: 12, padding: 24
});
doc.claim(frame);
doc.insert(frame, {
  type: "text", name: "Heading", width: "hug", height: "hug",
  content: [{text: "Overview", font: "Inter", size: 24}]
});
doc.release(frame);
return doc.get(frame);
```

Use execute for a bounded sequence or repeated construction; use typed tools for quick reads and simple
edits. claim/release refer to frames. Another agent's claim or a pause can refuse the transaction; report
that state and wait for direction. Never route around it. On a batch error inspect op (zero-based), path,
line and column; correct the failing input, refresh the scope if needed, and retry.

After editing, call snapshot_layout with problems_only and screenshot on the frame. Inspect overflow,
clipping, overlaps, missing fonts and visual hierarchy. Layout problems and screenshots can also be read
inside a script; they see earlier changes in that script. Report unsupported or incomplete render results.
doc.export supports png, jpeg, webp, svg and pdf with an optional scale. Use the export tool for HTML, multi-selection ZIPs and multi-page PDFs. doc.fillImage binds an
existing asset id, and doc.icon needs explicit icon options; neither fetches stock images or installs assets.
