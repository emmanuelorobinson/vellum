# Document format

Start with get_app_state, then read nodes, variables, components and Notes for agents in the affected scope.
Read context nodes beside the frame too. Existing design-system guidelines and the person's brief constrain
the work. Report missing inputs; do not invent a brand or replace existing tokens with a style recipe.

The saved document is deterministic JSON with meta, pages, variables, themes, components, styles, imports,
assets and fonts. Use tools or the doc API for changes: they validate a transaction, attribute its origin,
check leases and pauses, and preserve undo. Do not rewrite a .vellum file to bypass that path.

Node specs use the file-format spelling:

```json
{
  "type": "frame",
  "name": "Content",
  "width": 360,
  "height": "hug",
  "layout": "column",
  "gap": 16,
  "padding": 24,
  "children": [
    {"type": "text", "name": "Heading", "width": "hug", "height": "hug",
     "content": [{"text": "Overview", "font": "Inter", "size": 24}]}
  ]
}
```

Omit ids on inserts to receive generated ids. Use returned ids for subsequent edits; names can repeat.
Text content is an array of rich runs, not a string. Fills are arrays such as
`[{"type":"solid","color":"#FFFFFF"}]`. Auto-layout uses row, column or grid. Width and height accept a
number, hug or fill; absolute placement uses `position: {type:"absolute", x:0, y:0}`. Inspect the generated
NodeSpec types in vellum-doc.d.ts and the repository's generated types for the full union.

Bind values with `{"$var":"variableId"}`. Variable maps are keyed by id, with a type, name and values
array; a value entry can include `when: {Mode:"dark"}`. An alias is `{"$alias":"variableId"}`.
Inspect the document's actual axes and ids before binding. Component instances use componentId and props;
do not copy a component tree when an instance will do. Slots belong inside component frames.

Model units are floating-point coordinates. Asset and script paths use /, are relative to the document,
and cannot escape with .. . Keys belong in the system keychain, never the document or instructions.
Tools explain missing libraries, fonts, unsupported drawing and read-only documents; preserve those states
and report what could not be verified. The generated format reference in docs/format is authoritative for
serialization. This skill is a working introduction, not a replacement for that schema.
