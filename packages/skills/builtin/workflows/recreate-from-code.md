# Recreate from code

Use code or screenshots the person supplied, or request authorized access through the host's file tools.
Reading source files or running the application is outside the canvas and needs PermissionBroker access.
execute has neither filesystem access nor a browser. If access is unavailable, identify the missing input.

Read format.md and execute.md, then get_app_state, notes and system guidelines. Inventory the supplied
component structure, actual content, layout constraints, tokens, typography and interaction states. Reuse
existing canvas components and variables when their roles match. Keep layout containers editable, with
auto-layout for repeated content. A screenshot establishes appearance, not hidden token values or behavior.

Example translation of a supplied two-column code layout on an existing Screens page:

```javascript
const page = doc.page("Screens");
const frame = doc.insert(page, {type:"frame", name:"Two columns", width:640, height:240,
  layout:"row", gap:16, padding:24});
for (const label of ["Details", "Preview"]) {
  const column = doc.insert(frame, {type:"frame", name:label, width:288, height:192,
    layout:"column", gap:8, padding:16});
  doc.insert(column, {type:"text", name:"Title", width:"hug", height:"hug",
    content:[{text:label, font:"Inter", size:20}]});
}
return {frame:frame};
```

Use the supplied code's values in real work. Compare the rendered frame and reference at the same width,
height, state and theme. Run snapshot_layout(problems_only), inspect screenshot, and name any unresolved
font, asset, responsive or interaction differences. Do not infer parity from layer counts.
