# VS Code host (Prompt 55, incomplete)

The extension registers `vellum.design` for `.vellum` and `.lib.vellum`, uses the existing React editor
bundle, and exposes New Design File, Toggle Design / Text View, Open Welcome File and Pause All Agents.

**This is not an editable editor yet.** The compiled WASM module exports the renderer but no document
operations. The daemon's `EditorLink` is an in-process trait with no remote editor implementation.
Consequently the product runtime is read-only, the canvas cannot render document content, and the Agent
panel explains the missing engine with Open as text. No fake engine, JavaScript document mutations or
unregistered second writer are used to make editing appear to work. Do not release this build as Prompt 55 complete.

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
`build.mjs` alone builds the host and UI, without requiring a native daemon. Distribution, platform
selection, signed binaries and Marketplace/Open VSX packaging belong to Prompt 56.

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
Secrets and telemetry settings do not travel in the bridge.

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

Still required for acceptance:

- Implement the compiled document/scene/ContentSource ABI and its EngineClient bindings.
- Implement live editor registration, routing, follower updates and promotion over daemon IPC; prove
  that headless tools and every editor share one writer, including across extension hosts.
- Compose the runtime into the provider/webview; serve the feature ports left open by Prompts 49–54.
- Run real edit/save/revert, hot-exit restore, external git checkout as Actor::File Ops, attached-editor
  updates and native keyboard-conflict tests. Current runtime-boundary tests do not prove those flows.
- Complete pixel acceptance with the existing named design references after the document runtime is live.

## Tests

```sh
apps/extension/node_modules/.bin/vitest run --root apps/extension
apps/extension/node_modules/.bin/tsc --noEmit -p apps/extension
node scripts/skia-build.mjs test -p vellum --test mcp
```

The `vscode` lane is a **host smoke**, not full Prompt 55 acceptance. It checks registration, a library
file, the actual webview ready handshake under CSP, text/design toggle, split opening, and a successful
Pause All acknowledgement from a newly started test daemon. It needs a desktop session, the native
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
