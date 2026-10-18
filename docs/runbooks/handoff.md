# Ledger and hand-off ports (Prompt 53)

Pass the controllers to `EditorWorkspace.ledger` (all optional): `history`, `conflicts`, `inspect`, `tokens`,
`exporter`, plus `timeZone`. Create each with the session's EngineClient and device. `createEditorHandlers` takes
`exporter` so Copy as PNG / HTML (⇧⌘C, ⌥⌘C) work. Absent ports disable their controls; nothing is faked.

## Ledger (`HistoryHost`)

`entries()` is the daemon's session-log view of every accepted batch (actor, time, page, session, op summaries,
the raw op JSON, and `inverse` ops). The editor never derives entries. Revert applies `inverse` as ONE person
transaction through `EngineClient.apply`, so the host records it as its own entry attributed to the person
(`revertOf` when it can say so). A refused apply (the engine says so when later changes depend on the batch) shows
"Revert refused" and records nothing. `undo(entry)` is `vellum_core::ops::undo_turn`; offer it only for entries
marked `undoable`. `ToolOp` gained `delete` (Op::Delete) for conflict resolution; bindings must accept it.

## Files on disk (`FileChangeFeed`)

`{ kind: "applied", source, count, entry }` after an external change was applied as ops; `{ kind: "conflict",
versions, nodes }` after a three-way merge left `conflict` nodes. The conflict controller reads conflict nodes from
the document itself, so the banner and canvas marks disappear when the last one is resolved. Keep ours/theirs
replaces the node with that version (merged children kept); a removed side deletes it; Keep both inserts theirs
beside ours. Ask agent to merge calls `askAgent(node)`: route it through the session runner and PermissionBroker.

## Inspect (`InspectHost`)

`spec` maps `get_spec`; `prompt` returns the exact line copied ("Implement <file> · <frame> using get_spec.");
`engines` lists installed engines and their sign-in state; `implement` starts a turn on one; `html`/`exportHtml`
render deterministic HTML with `HtmlSettings` (flavour, scaffold, assets). `exportHtml` writes through the
PermissionBroker and returns the written path.

## Token sync (`TokenSyncHost`)

`preview` returns `vellum_core::tokens::plan`'s diff and the ops that apply it; the dialog shows the exact diff and
Apply is one person transaction, refused when the document revision moved since the preview. `exportPreview` and
`write` render and write the file (broker-gated; never inside the `.vellum` file).

## Export (`ExportHost`)

`frames` gives names and saved settings, `destination` the default folder, `thumbnail` an optional preview URL,
`writeFiles` receives the destination and one row per frame with its computed file name, `copyAs` puts PNG or
HTML on the clipboard.
