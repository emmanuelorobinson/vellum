---
title: "Desktop app"
description: "The desktop app: one window per folder, tabs for each design, the Home screen, and its security boundary. A development build, not yet packaged."
---

The desktop app is a window onto the same editor and the same daemon as the VS Code extension. It is a development build for now:
it runs from the repository, is not packaged, not signed and not updated automatically.

## Build and run

```sh
pnpm install --frozen-lockfile --ignore-scripts
node apps/desktop/node_modules/electron/install.js
pnpm wasm:build
node scripts/skia-build.mjs build -p vellum
node apps/desktop/build.mjs --with-daemon
pnpm --filter @vellum/desktop dev
```

The `--with-daemon` step copies the command-line binary into the app. Without it the shell starts, and it reports the daemon as
offline.

## Windows and tabs

The app opens one window for each folder you choose. Each document opens in a tab inside its folder's window, and a tab keeps the
workspace state of its document while you switch between tabs. Opening a folder that is already open focuses its window. Opening a
file that is already open selects its tab.

Each tab is a live editor on the daemon's engine. Your edits go to the daemon, and changes from agents and other editors arrive in the
tab. **Save** (`Mod+S`) asks the daemon to write the document. If the daemon cannot hold the file, the tab opens read-only.

## Home

The Home screen lists recent designs and drafts. It keeps at most two hundred of each in the app's own profile, and removing a recent
design from the list keeps the file. A draft is a new design the app created in `~/.vellum/drafts`, and it is found again at start-up
if the app stopped before the list was saved.

Some Home features are not built yet. Submitting a prompt from Home, thumbnails, and the bundled design systems are not available.

## Sign-in

**Sign in** starts device sign-in for a Pro account. The app shows a code to compare with the browser, and it does not contact the
account service until you ask it to. Local features never need an account. See [accounts and Pro](/docs/operations/accounts-and-pro).

## Security boundary

The window's content runs with the browser's sandbox and context isolation, without Node, without network access and without a
way to open a second window. The app talks to the daemon only through a narrow, typed bridge, so a web page loaded into the window
cannot read files, call the daemon directly or change the document. The [threat model](/docs/architecture/threat-model) describes the
full boundary.

## What is not yet available

- Packaged installers, code signing and automatic updates. See [updates and releases](/docs/operations/updates-and-releases).
- Presenting a live deck from the app.
- Live collaboration in the app. The app is not attached to a room yet.
