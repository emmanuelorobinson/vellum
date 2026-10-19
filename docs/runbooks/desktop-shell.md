# Desktop shell (Prompt 57)

The shell opens one window per canonical folder, with document tabs inside the shared editor workspace.
It currently opens documents read-only: the compiled document ABI and daemon EditorLink wire protocol are
still missing (Prompt 55). No TypeScript document store, synthetic canvas, or alternate save path is used.

```sh
pnpm install --frozen-lockfile --ignore-scripts
node apps/desktop/node_modules/electron/install.js
node scripts/skia-build.mjs build -p vellum
pnpm --filter @vellum/desktop dev
```

`build.mjs --with-daemon` builds static main/preload/renderer bundles and copies this machine's debug Rust
binary to `apps/desktop/dist/bin/`. This is a development build, not a distributable or signed application.
Packaging, Home, drafts, recovery and updater belong to Prompt 58. With no bundled binary, the shell reports
the daemon offline. `--user-data <directory>` and `--run-dir <directory>` on the native entrypoint isolate
profiles and daemon state for tests; they are never renderer capabilities.

File opens use native dialogs, accept `.vellum` files (including `.lib.vellum`) inside the chosen folder,
and resolve symlinks before granting window-scoped document IDs. Opening a folder again focuses its window;
opening a file again selects its tab. Each document remembers its workspace/tool state while switching tabs.
Arrow keys, Home and End move between document tabs. Close is available on hover, keyboard focus, and the
native File menu. The lifecycle model retains dirty tabs after cancelled confirmation or failed saves; the
current runtime cannot create unsaved edits, so those scenarios are boundary tests rather than live acceptance.

File / Edit / View / Object / Arrange / Agents / Help are native menus on macOS. Windows/Linux also expose
the categories through the document-bar menu. Folder/file picking, tab/window closing, core-backed New Design
File, panel switching, panel visibility, zoom, About, reconnect and Pause All are implemented. Save, Save As,
document edits, composer focus/Stop, keyboard-shortcut settings and other unserved features are disabled.
New Design File calls Rust's `open_document(create: true)`; choosing an existing file opens it without replacing
it. Reconnect restarts only this host's stdio bridge. Pause All requires the daemon's acknowledgement; it does
not claim to interrupt an engine that the daemon does not host.

The sidecar uses the shared native relay exported by `@vellum/host-api/native-relay`, also used by VS Code.
The native Rust executable is spawned without a shell; it starts/reuses and authenticates to the existing
user-only daemon. [`utilityProcess.fork`](https://www.electronjs.org/docs/latest/api/utility-process) runs a
JavaScript module, so it is not used to launch the Rust executable. Closing the host disposes its bridges,
watchers and subscriptions, leaving the shared daemon available to other clients. Failed requests are not replayed.

The renderer has `contextIsolation` and `sandbox` enabled, Node integration disabled, an exact bundled-asset
protocol, a restrictive response-header CSP, no network access, denied navigation/popups/downloads/web permissions,
and no webview element. A contextBridge API exposes only typed requests and subscriptions. Main-process IPC checks
the registered window, main-frame identity, exact application URL, action keys and document capability. No raw IPC,
RPC method, filesystem path, socket, daemon metadata, credential or document-write API reaches the renderer.
The Host bridge supports scoped document reads, invalidation, text clipboard, theme changes and an opaque daemon
endpoint; writes explicitly reject as read-only. Dispose both the Host and `createBridgeServices` when unmounting.
See the [Electron security guide](https://www.electronjs.org/docs/latest/tutorial/security).

```sh
pnpm --filter @vellum/desktop test
pnpm --filter @vellum/host-electron test
pnpm --filter @vellum/desktop test:electron
```

The tagged `electron` lane needs the pinned Electron runtime, a GUI session, a built native daemon and permission
to launch native processes/use local sockets. Playwright uses native-dialog boundary stubs, real windows, the real
daemon, temporary documents/profile/run directory, and stops only that isolated daemon on teardown. Functional tests
cover canonical folder reuse, tabs, native menus, Rust file creation, Pause All, Node/network/file denial, an
unregistered renderer, cross-window capability denial, symlink replacement, revocation, keyboard focus and shutdown.
The dirty/working A03 visual states are explicitly TEST DATA injected through the native event boundary; production
has no live running-session feed. `.github/workflows/desktop.yml` runs the macOS shell lane; its remote run and native
Windows/Linux acceptance remain unverified.

The `design` cases compare the 808×40 tab bar in light/dark against the unchanged A03 PNGs, at CSS-pixel scale
on Retina displays, with the existing 1%/48-channel gate. They skip with a reason when those gitignored PNGs are
absent. Current comparisons fail at 3.81%/3.60%; text rendering and the neutral running marker differ. References and
tolerance were not changed. `target/screen-goldens/A03-*-compare.png` shows design, implementation and difference;
`A03-*-desktop-window.png` shows the actual unavailable-runtime composition. The complete A03 screen is not accepted.

Before implementing the shell, the session read A03 and its tab component through Pencil on the read-only design
copy, studied Electron's security/utility-process docs through Context7, and checked sibling `outpost`'s secure
preload pattern. It reused Vellum's own VS Code stdio relay. No §3.2 reference row names Prompt 57; no external code
was ported and no new third-party package/version was introduced.

## Integrated macOS title bar

The Prompt 57/58 follow-up uses `titleBarStyle: hidden` with real native traffic lights at (20,20).
There is no additional macOS title strip. The existing editor bar reserves 88px at its physical left;
Home uses the same safe area. The top 56px and document-bar background are draggable; tabs, close controls,
Home navigation and the right-side actions are no-drag. Windows/Linux keep their native frame.
The native Electron regression checks full-window content height, native button coordinates, CSS drag regions
and real tab/header actions. Chromium screenshot captures exclude native window buttons. A03's unchanged
reference has no native-control reservation, so its pixel comparison still fails; see reports.md.
