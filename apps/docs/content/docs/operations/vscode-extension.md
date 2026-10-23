---
title: "VS Code extension"
description: "The {{brand}} extension for VS Code, Cursor and Windsurf: the custom editor for design files, its commands, how it edits, how to build and load it, and what is still open."
---

The extension adds a custom editor for `.vellum` and `.lib.vellum` files. Open a design file and the canvas opens in its tab, with the
same editor the desktop app uses. It is not yet on the Marketplace or Open VSX; you build it from the repository.

## Commands

| Command | What it does |
| --- | --- |
| New Design File | Creates a document in the folder you choose |
| Toggle Design / Text View | Switches a file between the canvas and its text |
| Open Welcome File | Opens a sample design |
| Pause All Agents | Makes every agent's next change fail |
| {{brand}}: Connect Agents | Writes the MCP entries for the agent clients on your computer. See [connect clients](/docs/agents/connect-clients) |
| {{brand}}: Sign In (Pro) | Starts device sign-in, with a notification that shows the code and an approval link |
| {{brand}}: Check Account | Refreshes what your account is entitled to |
| {{brand}}: Sign Out | Removes the local sign-in |

The sidebar shows the recent designs, at most twenty, and the sessions recorded for the files in the workspace.

## How it edits

The editor runs in a webview. The webview applies your edits to its own copy of the design and sends them to the daemon. It pulls
the changes from agents and other editors every quarter second. Save writes the document through the daemon.

A file opens read-only when the daemon cannot reach it, when it lies outside the folder VS Code has open, or when it cannot be read as
a design. A change the daemon refuses also turns the editor read-only, and reopening the file starts a fresh copy.

Undo and redo work within the editor. The editor's dirty state follows the daemon's record of the document.

## Colours

The setting `vellum.matchEditorColours` is off by default. When it is on, the chrome matches VS Code's theme, in all four of its kinds.
Canvas colours stay as the design defines them.

## Security

The webview has no socket, no raw connection to the daemon, no command execution and no access to files outside what the host allows.
It can load only the scripts, styles and fonts that the extension ships, and it cannot open frames or make network requests. Its
messages come from an exact list. The [threat model](/docs/architecture/threat-model) covers the trust boundary.

## Build and load

From the root of the repository, build the command line first, then the extension with the daemon included:

```sh
node scripts/skia-build.mjs build -p vellum
node apps/extension/build.mjs --with-daemon
```

Open `apps/extension/dist/package` as a development extension in VS Code. The build replaces the copied daemon by renaming a new file into
place. Overwriting an executable in place made macOS kill it on launch, so the build does not do that.

## Packaging

The extension packages for six targets: macOS on Apple silicon and Intel, Linux on x64 and arm64, and Windows on x64 and arm64. Each
package holds one native daemon for its target and checks that the binary matches it. The extension is licensed under Apache-2.0.

Publishing to the Marketplace and to Open VSX needs a publisher account and a signed, reviewed package. Neither has been done. The
[updates and releases](/docs/operations/updates-and-releases) page lists what is still needed.

## Tests

The extension's checks run in a real VS Code, with a private profile, a private daemon and private configuration folders. The
[verification](/docs/development/verification) page lists the commands. The VS Code tests need a VS Code installation and a desktop
session.
