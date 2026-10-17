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
