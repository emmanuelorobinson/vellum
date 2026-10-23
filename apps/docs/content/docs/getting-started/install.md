---
title: "Install from source"
description: "Build the command-line tool, check the daemon, and optionally build the VS Code extension or the desktop app."
---

No signed binary is published, so you build {{brand}} from a clone of the repository. The steps below build the
command line and its daemon, which every other surface uses. The later steps add the editor surfaces.

## 1. Clone and install

```sh
git clone <your-clone-of-the-repository> vellum
cd vellum
pnpm install --frozen-lockfile
```

If you only need the command line, `pnpm install --ignore-scripts` skips the install scripts. The desktop app needs its
Electron binary, which the [desktop app](/docs/operations/desktop-app) page covers.

## 2. Build the command line

The native build compiles Skia from its pinned source the first time, which takes a while. Later builds reuse the
cache.

```sh
node scripts/skia-build.mjs build -p vellum
target/debug/vellum version --json
```

Put `target/debug/vellum` on your `PATH`, or call it by its full path in the examples that follow. The same binary is
the command line, the daemon and the MCP server.

## 3. Check the daemon

The daemon is the one process that owns your documents while agents and editors work on them. You rarely start it by
hand: the commands that work with documents, the MCP server and the hosts start it when it is not running.

```sh
target/debug/vellum status --json
```

The daemon keeps its private files in `~/.vellum/run` by default. The directory is readable only by your user. If you
want an isolated daemon for a test, give every command the same `--run-dir`, as the
[running the daemon](/docs/operations/running-the-daemon) page explains.

## 4. Optional: the VS Code extension

The extension is the editor for `.vellum` files inside VS Code, Cursor and Windsurf. Build it with the daemon included:

```sh
node scripts/skia-build.mjs build -p vellum
node apps/extension/build.mjs --with-daemon
```

Open `apps/extension/dist/package` as an extension development folder, or install a packaged file for your own use.
The [VS Code extension](/docs/operations/vscode-extension) page covers the commands and the editing model. The
extension is not on the Marketplace or Open VSX yet.

## 5. Optional: the desktop app

The desktop app is a development build for now. It starts the same daemon and shows the same editor in its own window:

```sh
pnpm wasm:build
node scripts/skia-build.mjs build -p vellum
node apps/desktop/build.mjs --with-daemon
pnpm --filter @vellum/desktop dev
```

The `--with-daemon` step copies the command-line binary into the app. Without it the shell starts and reports the
daemon as offline. The [desktop app](/docs/operations/desktop-app) page explains what the shell does and what it does
not do yet.

## Verify the install

```sh
target/debug/vellum new design/app.vellum
target/debug/vellum export design/app.vellum --frame Home --format png -o home.png
```

The first command creates an empty document. The second needs a frame named `Home`, so it fails on an empty file. Once
you have drawn a frame, it writes `home.png`. Continue with [your first document](/docs/getting-started/first-document).

## When it fails

Build failures are almost always a missing native tool. The [troubleshooting](/docs/getting-started/troubleshooting)
page lists the common ones and their fixes.
