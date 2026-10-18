# Editor tools

`initialToolState()` and `transitionTool(state, event, context)` are the headless controller. The reducer
returns a new state, local `preview` operations, an overlay model and zero or one person-attributed
transaction. It never applies operations or changes a document. Give committed transactions to
`vellum-core` through the EngineClient; never apply preview operations to DocStore. On a rejected
transaction, restore the pre-event controller state and reconcile with the current engine snapshot.

Pointer inputs use screen CSS pixels. Viewport translation is in screen pixels and zoom is a positive
ratio. Scene queries, movement, snap corrections and measurements use world CSS pixels. Path handles
and path selection boxes use viewBox coordinates, with `overlay.pathAddress` naming the transform to
apply. New pen points use the insertion parent's coordinates until committed.

`ToolContext.scene` is a read-only facade for the existing Rust Scene and PathOps, not a second scene
implementation. It supplies hit testing (including inherited visibility/locking), prepared geometry,
snapping, occurrence ancestry and inverse coordinate transforms. It must reflect one consistent engine
snapshot for each transition. `createNode` supplies schema-valid tool defaults including paint, font,
polygon/star parameters and click-created text/note dimensions. IDs come from the injected generator.
No clock, randomness, DOM, React, renderer, vendor dependency or visual tokens are used here.

Tool keys use normalized `command`, `shift`, `alt` and canvas focus, without host-specific keybinding
registration (Prompt 45). Text input and IME are delivered as `content` events containing rich text runs
or a note string. `finish` commits the content; Enter remains available to the text editor. Selection and
viewport changes have no document transaction. Switching tools or cancelling discards pending edits;
Escape during a path drag restores that drag, while Escape in an idle path session exits path editing.
Space and Z can suspend an idle path/pen/scale session and restore it after viewport interaction.

Path editing consumes normalized cubic contours prepared by the engine adapter. The controller preserves
cubic geometry when splitting a segment and emits SVG path data on commit. Primitive conversion keeps
appearance, transforms and size bindings and replaces the shape with a vector. Bound/hug/fill primitives
need `EditablePath.viewBox` from resolved geometry; absent data returns `missing-data`. Instance descendants use
`replaceInInstance` for movement, content and path edits. Scaling an instance root emits the existing core
`Scale` operation (which handles overrides); scaling a descendant occurrence returns `read-only`, because
core has no path-addressed Scale operation. No component definition is changed as a fallback.

The Op subset re-exported by `tool-types.ts` mirrors core operations as in-process TypeScript requests. It is not a
new serialized protocol. The actual WASM EngineClient/Scene adapter and rendered canvas are still the
later prompts' work, so these tests exercise the controller with synthetic Scene queries, not a live
Rust document or a visual UI.

Run `pnpm --filter @vellum/editor-core test` and `pnpm --filter @vellum/editor-core type-check`.

# Commands and extension registries (Prompt 45)

`createRegistries()` scopes tools, commands and panels to one editor. Registrations are ordered,
searchable, observable and disposable; duplicate IDs fail. `createCommandController(handlers)`
registers the built-ins through those same APIs. A missing feature handler is disabled, and a
throwing handler propagates failure. Handlers supplied by later feature/host compositions perform
the actual engine operations; the registry itself writes no document.

`Keybindings` holds the DESIGN.md Keyboard table, physical `KeyboardEvent.code` normalization,
macOS/Windows/Linux modifiers and desktop/IDE variants. Canvas, path editing, composer (empty or
nonempty), panel, text and presentation focus are distinct. Text entry keeps native Tab and undo.
Reserved host chords stay delegated; Save always goes through the host. Pointer and gesture rows
are named inputs too (Click, DragPoint, Scroll, Pinch, ZoomClick, etc.); the view supplies the target
and held-tool context. Nudge commands leave configured amounts and auto-layout reordering to their
engine handlers. `rebind` validates input and rejects overlapping scopes or reserved host keys
without changing the previous binding. `keyUp` releases rebound temporary tools by their physical
key even after modifiers or focus change; `blur` releases every held tool. Supply paired hold/release
handlers for temporary tools. Pass a `preventDefault` callback to `keyDown` for synchronous
cancellation. Overrides are scoped to host and platform; Settings will persist them in Prompt 54. Registry/keybinding commands describe features; they do not implement
slides, clipboard export, panel UI, file dialogs or the later component editors.

# Workspace, layers, assets, overlay and session (Prompt 46)

All of it is headless: no React, DOM or renderer.

- `createWorkspace()` / `computeLayout(size, state)`: DESIGN.md's layout rules as rectangles. The agent panel is 360 (resizable
  320–520 from 1040 up), 320 below 1040, a 44 px strip below 960 that opens as an overlay; the right panel appears with a
  selection, docked from 1200 and over the canvas below; Tab hides all chrome, ⌘\ only the panels; the last tab is kept per
  document. `canvasInsets` says what docked chrome covers, for fit commands.
- `layerRows`, `createLayerIndex`, `dropTarget`, `planDrop`, `renameTransaction`, `setVisibleTransaction`,
  `setLockedTransaction`: the Layers tab. Rows list children in **document order** (`order: "front-first"` reverses it); a marker
  shows for agent-made layers younger than 10 s by the injected clock. Dropping gives `reorder` among siblings and `move` to
  another parent, placed after an anchor so several dragged layers stay together and layers already in place produce no op.
- `assetSections`, `instanceTransaction`: the Assets tab's sections, search and the instance a dropped component makes.
- `overlayShapes`, `distanceLabels`: screen-space shapes (outlines, handles, guides, size and distance labels, frame names) from
  the tool controller's overlay. Drawing them is the host's job.
- `createEditorSession(deps)`: one person's session over an `EngineClient`. It feeds tool events, hands Person transactions to the
  engine, puts the tool back to idle (and keeps a notice) when the engine refuses, drops selected ids that no longer exist,
  and does only the person's own undo and redo.
- `createEditorHandlers({ workspace, session })`: handlers for `createCommandController` (panels, tools, undo/redo, selection keys,
  zoom and fit). Commands without a handler stay disabled.
- `ToolOp` gained `move` and `reorder`, matching core's `Move` and `Reorder`.

# Properties: values, paints, strokes, effects, text and components (Prompt 47)

Still headless. Everything the right panel's Design tab does is here; `editor-react` only draws it.

- `parseField`, `formatField`, `stepField`, `scrubField`, `mergeValues`: the value-field rules. Numbers keep their unit; a sum
  such as `16*2` is read by a small parser (no names, calls or properties); arrows step by 1, 10 with Shift, 0.1 with Alt; a
  selection that disagrees is `mixed`.
- `readDesign({ targets, document, mode, readOnly, text, editing })` returns the `DesignModel`: the sections a selection has
  (`sectionsFor`: Text, Component or Notes first, as design screens C13, C14, C16 show), each field as a `FieldValue`
  (`value`, `bound` to a variable with its name and what it resolves to under the theme, `mixed`, or `none`), size per axis as
  hug, fill or fixed, paints, strokes and effects as rows, the text model and the component or instance model.
- `design/patches.ts`: one `Edit` per field (`setOpacity`, `setGap`, `setPaddingSide`, `addFill`, `updateStroke`, `setBlend` …).
  An edit gives a `NodePatch` for a target or `null`; `combine` merges several so one click is one transaction. A default
  value is written as `null`, which the core reads as "back to the default", so files print one spelling. A part of an instance
  is changed with `replaceInInstance`, never by writing the component. Variables are `{ $var }` values anywhere a number or colour is.
- `paint.ts`: paint kinds and conversions (a solid becomes a gradient from its colour to clear; a gradient becomes a 2 × 2 mesh),
  stops, gradient angle, mesh resize (colours resampled), point moves, `renderMesh` (RGBA pixels laid onto the quads the points
  make), image fit and crop (`cropCorners`, `dragCropCorner`), opacity, visibility, the 18 blend modes. `stroke.ts`, `effects.ts`,
  `text.ts` and `component.ts` do the same for their parts; `component.ts` also builds the core's `Detach` and `UpdateComponent`.
- `createDesignController({ session, engine, defaults, text })`: the selected layers as targets (including whether a parent's
  layout places them), the model, `edit(edit)` (one Person transaction through `session.commit`), `ops(ops)`, and the request
  the Replace inside instance command makes of the panel. A text layer open for typing is read as its draft.
- `createPaintEditor({ session, design })`: the fill editor's part of the canvas. While a mesh gradient is open its points can be
  dragged where they are on the shape, and an image's crop corners while cropping; a drag is shown at once and written as one
  transaction on release; `cancel()` puts it back. `overlayShapes` draws them (`paintHandles`).
- Tools: with the move tool, a double-click on a text layer opens it for typing (DESIGN.md C14). `EditorSession` gained `commit`
  and `device`; `ToolOp` gained `detach` and `updateComponent`; `createEditorHandlers` takes the design controller and then
  handles `component.detach` and `component.replace`.
- Not here: a TypeScript document engine, variable creation and binding a plain number to a variable (Prompt 48), range formatting
  of text (changes apply to every run of a layer), fallback sizes next to hug and fill, and anything that reads or writes files.
