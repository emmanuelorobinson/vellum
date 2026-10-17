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

The Op subset in `tool-types.ts` mirrors core operations as in-process TypeScript requests. It is not a
new serialized protocol. The actual WASM EngineClient/Scene adapter and rendered canvas are still the
later prompts' work, so these tests exercise the controller with synthetic Scene queries, not a live
Rust document or a visual UI.

Run `pnpm --filter @vellum/editor-core test` and `pnpm --filter @vellum/editor-core type-check`.
