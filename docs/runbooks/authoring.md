# Editor authoring ports (Prompt 52)

Pass an `AuthoringController` to `EditorWorkspace.authoring`. Create it with the same EngineClient,
device and node-ID generator as the session. Dispose it before disposing the engine. Absent host
capabilities disable their controls or show an explicit unavailable state.

## Scripts

`AuthoringHost.scriptSnapshot` returns a document-revision-tagged map keyed by Scene occurrence
address. Parse headers through `vellum_core::script_nodes::Header`; map its six input kinds and
min/max/step/default/options to `ScriptInput`. Do not evaluate scripts or parse headers in the UI.
Publish loading, ready, empty or error through `subscribeScripts`. Keep the revision associated
with the run; never tag old results with a new revision. Errors carry the same sanitized message,
line and bounded excerpt as `ScriptDiagnostic` in core.

Ready means the engine holds a successful current derived output. Supply that output through the
existing ContentSource/display-list renderer. The UI never saves derived children or draws a second
chart implementation. Without a content renderer it shows “Script preview unavailable”. The binding
maps `convertScript` to `Op::ConvertScript`; core snapshots its current output in one undoable
person transaction. Inherited script conversion is disabled until the instance is detached.
`openScript` opens the relative source in the host editor; `chooseScript` returns a relative `.js`
path. `askScript` routes a write/fix request through the session runner and PermissionBroker.

## Guidelines

`loadGuidelines` resolves the active design system's actual `guidelines.md`, through authorized
rooted file access. Return a file version/hash and explicit writable flag. `saveGuidelines` must
compare that version atomically before writing, fail on a conflict, and return the new version.
Refresh the design-system SkillSource after a successful save. Keep path/content out of `.vellum`:
this editor writes the external rules file; it does not edit the canvas document. Host errors must
be sanitized. Failed saves retain the edited text and the loaded version so nothing is silently lost.

## Slots

The editor reads local component definitions and occurrence addresses from the engine. `makeSlot`
maps to `Op::MakeSlot`, including updates to suggested component IDs. Inserting a suggestion appends
an instance to `slotContent` through the same person transaction boundary, preserving earlier slot
content. A nested selected instance is changed through `replaceInInstance`. Suggestions are guidance,
not an allowlist. Missing/imported definitions are not fabricated by the UI; the host must provide
resolved library editing before those definitions can be edited here.

## Pictures

The host implements choose/drop through ImageImporter/AssetStore, and search/fetch through the
configured ImageSource. Return the validated stored asset hash only after bytes are persisted;
Stock imports must return the fetched attribution. Neither API keys nor download credentials cross
this port. A document revision change or closed picker discards a late import response.

Results contain provider, author, original source and licence links. The editor renders those actual
values (ADR-009 uses Pixabay), never substitute demo attribution. The resulting Op replaces only
the requested fill/stroke paint, retains its image fit/opacity/visibility, and stores attribution in
the node metadata using the existing image-tool convention. Generate is disabled for v1.1.

The browser fixtures exercise these ports with scripted EngineClient replies. They do not prove
native file access, source watching, QuickJS execution or the compiled document-operation WASM ABI.
