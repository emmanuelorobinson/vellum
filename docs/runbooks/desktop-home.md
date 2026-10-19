# Desktop Home, drafts, recovery and releases

Prompt 58 extends the Prompt 57 shell. **This is a read-only editor preview, not a release.**
The shared document WASM ABI/EditorLink integration from Prompt 55 remains missing. Home prompt
submission, thumbnails, bundled systems (the catalog package is still a shell), provider-key controls,
weekly usage and client last activity are explicitly unavailable. Account sign-in belongs to Prompt 61.
No account is needed for the local shell, file creation, drafts, MCP configuration or recovery.

## Native lifecycle

Home stores bounded metadata in Electron's profile `home.json`: at most 200 recents and 200 drafts.
Removing a recent keeps its file. Drafts are core-created canonical documents under `~/.vellum/drafts/`;
core's headless target saves every accepted transaction. The desktop currently cannot produce edits.
A draft that exists without an index entry is rediscovered at startup, covering a crash between core's
save and the metadata rename. `--user-data` isolates drafts under that test profile's `local/drafts/`.
Never point a test at the normal profile or replace the process's HOME.

Save As copies canonical bytes to a new, exclusively created destination, flushes it, commits metadata
then removes the old draft. It refuses overwrite and symlink replacement. The daemon releases its clean
headless writer first; a live writer or failed save refuses release. A failure keeps the source until
metadata commits; interruption can leave two copies rather than destroy the only one. Deleting a draft
requires native confirmation. Native menus include Home, Manage agents, Sign in and Check for Updates;
Save As is enabled for drafts. Native drops pass disk-backed Files through preload's webUtils without
adding a renderer API for native paths. File/window capabilities and sender checks still apply.

Opening a file asks the initialized, workspace-scoped `vellum/recovery` method for metadata. Save restores
through core's File/Recovery transaction and saves; Save As creates a new core document, applies the
recovery diff as a File/Recovery transaction and saves it. Originals and backups survive failed restores;
a successful save removes the backup. Discard uses native confirmation; Cancel keeps it. Preview rendering
is unavailable. A06 uses real Rust recovery, not a second TypeScript document store. Edited-document crash
recovery cannot be accepted until the live editor runtime exists. The native kill-9 test covers an already
persisted draft and restart, not unsaved editor changes.

Agent configuration reuses Prompt 56's AgentConnections and managed JSONC/TOML writers, now exported by
host-vscode with the extension's old import re-exported. Detection never launches clients. Changes show
the destination and exact managed entry in a native confirmation; removal refuses foreign/modified entries.
No actual client configuration was changed during acceptance. Provider secrets are never sent to the renderer.

Notifications default off and persist in `preferences.json`. The native main process polls recorded turn
transitions for open files every two seconds. It suppresses historical, duplicate, opted-out and focused-window
notifications; completion, failure and permission transitions use fixed text without conversation contents.
Click focuses the existing document. External MCP sessions contain tool calls only and have no inferred turn
outcome. This is a bounded recorded-state feed, not the missing live AgentFeed; rapid intermediate transitions
and turns whose latest transition is outside the most recent 100 events are not guaranteed notifications.
OS delivery/click acceptance with a real running engine remains unverified.

## Build and test

Use Node 24 and pnpm 12 with `TURBO_TELEMETRY_DISABLED=1 NEXT_TELEMETRY_DISABLED=1`.

```sh
node scripts/skia-build.mjs build -p vellum
pnpm --filter @vellum/desktop test
node apps/desktop/build.mjs --with-daemon
cd apps/desktop
node_modules/.bin/playwright test --config test/playwright.config.ts --grep-invert @design
```

The Electron lane needs a GUI session and the native daemon. Tests isolate their profile, folders and daemon
run directory, stop only that daemon, and remove their data. `VELLUM_ELECTRON_EXECUTABLE` can select a packaged
app executable. Goldens compare all five desktop frames in both themes at 1280×800 with the existing 1% gate,
48-channel threshold. They currently fail; no reference/tolerance was changed. Comparison sheets are in
`target/screen-goldens/A0*-*-desktop-compare.png`. Fixtures cross the native state-event boundary and never
supply synthetic document scenes. Pencil read the read-only design copy; gaps are recorded append-only.

## Packaging

Pinned tools: electron-builder 26.15.3, electron-updater 6.8.9. `packaging.mjs` validates the daemon's Mach-O,
PE or ELF architecture before building. Every artifact contains the native daemon in resources/bin and the
bundled JS/assets, Apache-2.0 license, third-party asset notices and dependency license texts. Explicit file
patterns exclude redundant workspace/node_modules sources. No network-capable UI or signing credential is bundled.

```sh
node apps/desktop/packaging.mjs --platform darwin --arch arm64 \
  --daemon target/debug/vellum --development-preview --dir
```

This creates an **unsigned development app** under `target/desktop-packages/mac-arm64/`; it has no update feed. Omit `--dir` to also build the local ARM64 DMG and ZIP previews. Each invocation narrows the builder target list to the validated daemon architecture; the CLI architecture flag alone does not override configured target architectures.
The preview currently uses Electron's default icon. There is no chosen standalone app-icon asset to package.
Matrix: macOS arm64/x64 (dmg and updater zip); Windows x64/arm64 (NSIS); Linux x64/arm64 (AppImage, deb, tar.gz).
Only macOS arm64's app, DMG and ZIP previews were built; the app was smoke-tested here. Other native binaries, installers,
signing/notarization, Windows publisher verification and installed platform upgrades need their own lanes.

Release packaging refuses version 0.0.0, missing signing/notarization credentials, missing publisher/key/feed
and incomplete runtime/visual acceptance (`VELLUM_DESKTOP_ACCEPTANCE=complete` is for the release operator after
those checks really pass). Supply the release version, CSC signing inputs, Apple notarization inputs,
`VELLUM_WINDOWS_PUBLISHER`, an HTTPS directory `VELLUM_UPDATE_URL`, and `VELLUM_UPDATE_PUBLIC_KEY_FILE` containing
only the public Ed25519 key. Feed paths must end with `/`; credentials/query strings are refused. Preview explicitly
sets mac.identity to null. No package command publishes: electron-builder always receives `--publish never`.

## Signed updates

The installed public key is the trust root; it is never fetched from the server. The updater verifies an
Ed25519-signed `release-<platform>-<arch>.json` before consulting electron-updater. Its signed payload binds
platform, architecture, stable version, relative artifact basenames, sizes and SHA-512 hashes. It rejects
unsigned/tampered metadata, metadata mismatch, unexpected artifacts, downgrades and altered downloads.
macOS's Squirrel and Windows Authenticode checks still apply; Windows build configuration requires the expected
publisher. The additional Ed25519/digest gate covers Linux packages too. Downloads and installation require
separate explicit actions; automatic download/install-on-quit, prereleases, downgrades and web installers are off.
A verification failure leaves the current installation running and cannot enter the installable state.

Alongside electron-builder's generated latest*.yml, publish a signed envelope for each platform/architecture.
`apps/desktop/sign-release.mjs --version <stable-version> --platform <platform> --arch <arch>
--artifact <artifact-path> [--artifact ...] --out <envelope-path>` computes the actual artifact hashes and signs
the payload. Its private key comes from stdin, supplied by the operator's OS keychain or CI secret injection;
never store that key in the repo, command arguments or logs. The script prints no key or signing input and
refuses to overwrite output. Include every file that latest*.yml lists. Serve architecture feeds separately
where the library's platform metadata would otherwise select another architecture. Keep the private signing
key outside this project and manage public-key rotation through a signed application release.

Unit tests use real in-memory Ed25519 signatures and SHA-512 with mocked network/filesystem/installer boundaries,
including bad manifests and changed artifact bytes. No live feed, OS upgrade, signed installer rollback or
post-install health rollback has been exercised. Release is blocked on those checks and the missing runtime.

Reference study: no §3.2 row names Prompt 58. Read Electron's Notification/webUtils documentation and
builder/updater signature/packaging documentation through Context7; inspected sibling outpost's updater
policy and builder configuration. Kept explicit download/install, fixed notifications and native ownership;
Vellum adds a signed release envelope and verifies exact downloaded bytes on every platform. Reused Vellum's
own config writer and Rust recovery service; no reference-repository source was ported.
