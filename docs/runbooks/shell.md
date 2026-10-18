# Palette, context menu, settings and edge states (Prompt 54)

`EditorWorkspace` takes two optional props: `shell` (`palette`, `settings`, `settingsHost`) and `edge`. Without
`shell` there is no palette, no context menu and no settings dialog; without `edge` the workspace shows no edge
state (the bottom strip, the offline composer and the earlier panels behave as before). Nothing here writes the
document: it selects, navigates, puts words in the composer and asks the host.

## Command palette (⌘/, C23)

```ts
const palette = createPaletteController({
  commands: () => paletteCommands(controller.commands), // the registry, with what can run now
  entries: () => paletteEntries(engine.snapshot().document, agent.state.sessions), // layers, pages, components, variables, sessions
});
createEditorHandlers({ …, palette, pauseAll: () => presence.action({ type: "pauseAll" }) });
```

Create the palette before the command controller (⌘/ needs it); the workspace binds what a choice does (run a
command the way a shortcut does, open an entry, or Ask the agent) once it renders. **Ask the agent: <query>** is
always the first row for a non-empty query and fills the composer; it never sends. ⇧⌘E (`edit.export`) opens the
Export dialog for the selection when `ledger.exporter` is given.

## Layer context menu (C24)

Right-click on a layer (canvas or Layers panel). A row runs its command through the controller; a command the
controller does not know goes to `onAction(command)` (the workspace reads the selection from the session). Hosts
must handle `edit.copy`, `edit.paste`, `edit.pasteHere`, `edit.group`, `edit.wrap`, `edit.autoLayout`,
`component.toggle`, `arrange.forward`, `edit.copyCss`, `edit.copyPng`, `edit.copyNodeId` and `edit.delete`; a row
whose command has no handler is greyed out, not hidden. `agent.askAbout` and `agent.redesign` are registered by the
workspace (the composer takes focus; Redesign starts the request "Redesign " with the selection as its chip).

## Editor settings (C25)

`createPreferences(host)` owns the typed local preferences (`PreferencesHost.load/save`: browser local storage, a
desktop settings file, the IDE's memento). Values are validated on set and on load; unknown or invalid saved values
are dropped. Telemetry is `false` until the person turns it on, and the dialog shows exactly the three lines in
`TELEMETRY_DISCLOSURE`. `createSettingsController({ preferences, keys, context })` adds the shortcuts list, search,
rebind (conflicts name the other command; host chords answer `host`) and reset.

What the workspace applies itself: backdrop, animations off (`data-animations`), scroll-wheel zoom and its
direction, snapping (`session.setSnap`). What the host applies: theme and colour matching (`appearance`), renderer
(Automatic or Basic), language, notifications, auto-open History, insert highlight, plan threshold, external helper
agents, permissions defaults, stock provider, icon sets. The pixel grid preference is stored; nothing draws the
grid yet. `SettingsHost` supplies the stock providers, whether a provider key is stored (never the key), Change key,
Remove skill and the host's chord list. Engine, model, effort, parallel and Skip all prompts go through the composer
and the Agent feed (`composer.setSkipAll`).

## Edge states (S01–S16)

Pass `edge: { host, … }`. The workspace reads what it can: the daemon from `connection`, the renderer from
`basicRenderer`, read-only from the engine, missing fonts and libraries from `resources`, the layer count from the
document, engine problems from the composer's engine catalog (`InstalledEngine.resetsAt` is new: when a usage limit
lifts). The host supplies what only it knows: `newerFormat`, `unreadable` (line, column, reason), `closing`
(`{ fileName, running }`), `rejection`, `largeFile` / `layers`, `loading` per panel, and `host.action(id)` for the
buttons the editor cannot do itself (`restartDaemon`, `troubleshoot`, `updateVellum`, `openAsText`, `formatSpec`,
`learnMultiFile`, `openTerminal`, `recheck`, `reviewChanges`, `updateKey` for an installed engine). `closeChoice`
answers the unsaved dialog (`cancel`, `discard`, `save`). `rendererSettings`, `reviewFonts`, `switchEngine`,
`updateKey` (built-in providers) and `dismiss` are handled in the editor.

One notice at a time takes the Agent panel's place (most important first: unreadable, closing, read-only, daemon
offline, engine problem, missing fonts, basic renderer, large file); one banner at a time sits under the document
bar. `panelView` is the global state matrix: every panel in every state shows exact copy or says it still works.
The words live in `packages/editor-core/src/shell/edge.ts`.
