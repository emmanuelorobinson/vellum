# Design a screen

1. Read get_app_state and format.md. Identify the requested frame size, audience, content and primary action.
   Read Notes for agents and context nodes, then system guidelines, variables and components in scope.
2. State unresolved product or brand choices before drawing. For several frames or more than roughly twenty
   changes, present the frame list and approach for review first. Use get_style only when no system exists.
3. Read execute.md. Find free space with find_space. Claim an existing frame before changing it. Build a
   small layout with existing variables and component instances; keep names meaningful for later edits.
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
