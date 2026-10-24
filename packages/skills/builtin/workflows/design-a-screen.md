# Design a screen

1. Read get_app_state, format.md and design-craft.md. Identify the requested frame size, audience, content and
   primary action. Read Notes for agents and context nodes, then system guidelines, variables and components in scope.
2. Settle product or brand choices before drawing. Ask only when a missing decision would make the work wrong (an
   existing brand's colour or font you cannot see); otherwise choose, build, and list your assumptions in the
   report. When the brief already names the screens, build them; do not stop to ask for approval of the list. For
   a large rework of existing frames, present the plan first. Use get_style only when no system exists.
3. Read execute.md. Find free space with find_space. Claim an existing frame before changing it. Build a
   small layout with existing variables and component instances; keep names meaningful for later edits.
   For icons use search_icons and insert_icon, not hand-drawn paths, and colour them with a colour variable. For
   real pictures use fill_image (a repository file, or a stock search the person has allowed) rather than grey
   boxes; stock photos keep their author and licence in the layer's metadata. import brings in an SVG or a Figma copy.
4. Read snapshot_layout(problems_only) and screenshot at the requested dimensions and theme. Fix the named
   problems, inspect the picture, release the frame, and report changed ids and any remaining limitations.

Construction example, assuming an existing Screens page and an available Inter font:

```javascript
const page = doc.page("Screens");
const frame = doc.insert(page, {type:"frame", name:"Overview", width:360, height:240,
  layout:"column", gap:16, padding:24});
doc.insert(frame, {type:"text", name:"Title", width:"hug", height:"hug",
  content:[{text:"Overview", font:"Inter", size:24}]});
doc.insert(frame, {type:"rect", name:"Content area", width:312, height:96});
return {frame:frame};
```

These dimensions demonstrate the API. The person's brief and document tokens supply production values.
