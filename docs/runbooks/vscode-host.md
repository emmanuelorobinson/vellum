# VS Code host (Prompt 55; document runtime per ADR-033)

The extension registers `vellum.design` for `.vellum` and `.lib.vellum`, uses the existing React editor
bundle, and exposes New Design File, Toggle Design / Text View, Open Welcome File and Pause All Agents.

**Editing.** Each webview runs the editor WASM module (`vellum-editor`, engine plus renderer) as a replica of the
daemon's engine (ADR-033). The host reads `dist/webview/engine/vellum_editor.wasm` and Inter and posts them to
the webview, which opens its own editor at the daemon through the host relay (`vellum/editor`), applies the
person's edits locally and pushes them, and pulls agents' and other editors' changes every 250 ms. The
document's `DaemonRuntime` (`@vellum/host-api`) answers VS Code's save, save as, revert, backup and hot-exit
callbacks from the daemon's engine; dirty state follows the engine's revision.

A file opens **read-only** (the webview draws it from its bytes, with Open as text) when the daemon is
unreachable, when the file is outside the window's workspace folder (the daemon scopes editors to the host's
workspace), or when the file cannot be read as a document. A push the daemon refuses makes the editor read-only
and says so; reopening the file starts a fresh replica.

## Build and run

From the repository root (Node 24 and the pinned pnpm):

```sh
node scripts/skia-build.mjs build -p vellum
node apps/extension/build.mjs --with-daemon
```

Load `apps/extension/dist/package` as the VS Code extension development directory. It contains an
unscoped VS Code manifest, the CommonJS host, static JS/CSS, a copy of the TEST DATA welcome fixture,
and, with `--with-daemon`, the native daemon for the current platform. The build replaces copied
executables by rename; overwriting an existing executable inode caused macOS to kill it on launch.
`build.mjs` alone builds the host and UI, without requiring a native daemon. Prompt 56 adds platform validation, private preview VSIX packaging and explicit publishing commands;
see `docs/runbooks/connect-agents.md`. Signed release binaries and public publishing remain blocked.

The extension uses `host-vscode` for native file, clipboard, theme and daemon-discovery services. It
launches the bundled `vellum mcp` bridge; that bridge starts/reuses the authenticated private daemon.
The webview has no socket, discovery token, raw RPC, unrestricted command execution or file access.
Messages are an exact allowlist. CSP allows local scripts with a nonce, local styles/fonts, and
style attributes needed by the existing React canvas layout. Network connections, frames, forms,
command URIs and arbitrary local resources are denied. Standard error is consumed without forwarding
vendor diagnostics. Requests are bounded and never replayed after disconnect.

`vellum.matchEditorColours` is off by default. All four VS Code theme kinds map to the existing token
appearances. Matching changes neutral chrome surfaces/text only, leaving canvas semantic tokens alone.
The webview uses editor-core's existing VS Code shortcut policy; the manifest adds no global keybindings.
Secrets and collector configuration stay in the native host. Prompt 71 adds only an explicit telemetry
consent boolean to the bridge; the webview has no collector URL, key or network access.

## Document lifecycle boundary

`DesignDocument` serializes lifecycle requests. It accepts bytes and revisions only from a
`DocumentRuntime`: core must serialize canonical bytes, emit accepted-commit notifications, and
apply revert, external merge and recovery through attributed Ops. A replacement returns its committed
revision so a later person edit cannot accidentally become saved. Saves acknowledge only the snapshot
actually written. Backup never clears dirty state; Save As never marks the old resource saved.
Cancellation, failure and disposal retain the unsaved state. Native writes use a same-directory exclusive
temporary file, fsync and rename; symlink targets are refused.

`DesignEditorProvider` shares one custom document among VS Code split panels, owns the file watcher,
registers save/save-as/revert/backup callbacks, and waits for each webview's ready handshake. Dirty
notifications currently use content-change events. They are not a substitute for core's per-person undo;
full native undo integration must be validated with the real runtime.

Still open: library documents and script outputs in the browser scene, image fills and document fonts in
frames, and the feature ports of Prompts 49–54 (agent feed, presence, ledger). Pixel acceptance of the named
design references, Windows/Linux and remote CI remain.

## Tests

```sh
apps/extension/node_modules/.bin/vitest run --root apps/extension
apps/extension/node_modules/.bin/tsc --noEmit -p apps/extension
node scripts/skia-build.mjs test -p vellum --test mcp
```

The `vscode` lane checks registration, a library file, the webview ready handshake under CSP, live editing
(the webview's engine opens its replica, an agent's change through the CLI makes the document dirty, Save writes
it), text/design toggle, split opening, and a Pause All acknowledgement from a newly started test daemon. It opens
a workspace folder, since the daemon holds only files inside the host's workspace. It needs a desktop session, the native
binary, and VS Code >=1.140. Without a supplied executable, test-electron downloads 1.140.0 (network).

```sh
VELLUM_VSCODE_EXECUTABLE='/Applications/Visual Studio Code - Insiders.app/Contents/MacOS/Code - Insiders' \
  pnpm --filter @vellum/extension test:vscode
```

Tests use a fresh profile and private daemon run directory, short enough for macOS Unix sockets, then
stop that daemon and remove the directory. `VELLUM_RUN_DIR` is an optional native-process environment
override for the runtime directory; it is not a workspace setting or a webview parameter. No request
or shutdown is directed to the person's default daemon by the final smoke harness.

## Reference study (guide R19)

The pinned vscode-samples custom editor keeps one native document per URI, broadcasts updates to its
webviews, reports edits for dirty/undo, and implements save/revert/backup through provider callbacks.
Vellum keeps that shared-document and ready-handshake pattern. Its data must come from the Rust core,
not the sample's JavaScript edit replay or webview serialization. The sample's broad resource policy
is replaced with local resource roots and a restricted typed bridge. No source was ported.

Prompt 56 adds Connect Agents, the Activity Bar views and native host metadata; it does not resolve the
document-runtime limitations above. Its test home/config overrides keep real client configuration untouched.
