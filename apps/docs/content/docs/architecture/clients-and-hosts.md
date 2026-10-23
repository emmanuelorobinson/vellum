---
title: "Clients and hosts"
description: "The editor's stack, its renderer, its overlays and its translations, and the hosts that run it in VS Code and in the desktop app."
---

The editor is one bundle that runs in every host. The host decides where the bundle runs, which files it may open, and how it talks to the
daemon. The editor decides what the canvas shows and what a person can do with it.

## The editor's stack

| Layer | Choice | Why |
| --- | --- | --- |
| Build | Vite, producing a static bundle | A VS Code webview cannot run a server |
| Controller | `editor-core`, in TypeScript, with no framework | The controller can be tested without a browser or a renderer |
| Interface | `editor-react`, in React, with Base UI primitives | The interface can change without touching the controller |
| Canvas | The renderer, which is Skia inside the WebAssembly module, drawing on WebGL 2, with a Canvas 2D fallback | One renderer for every host |
| Overlays | A second, light canvas for handles, guides, agent cursors and claimed frames | Overlays change every frame and should not redraw the design |
| Translation | i18next, with bundled resources, and right-to-left layout for Arabic | No remote service is involved in translation |

The engine and the renderer are one WebAssembly module. The engine owns the document in that module. The page sends operations and input
events in, and receives change notices back. Drawing data stays in the module's memory, so no drawing command crosses the boundary one at a time.

## The hosts

A host is the program that runs the editor, and it provides the things the browser cannot: the files, the clipboard, the theme, and the
connection to the daemon.

**VS Code and compatible editors.** The extension registers the custom editor for design files. The webview holds the editor, and the
extension host holds the connection. The webview cannot reach the daemon, the file system or the network directly, and its messages come from
an exact list. The [VS Code extension](/docs/operations/vscode-extension) page describes its commands and its editing model.

**The desktop app.** An Electron shell opens one window per folder, with a tab for each document. The renderer runs sandboxed, with no Node
access and no network, and talks to the main process through a narrow, typed bridge. The [desktop app](/docs/operations/desktop-app) page
describes what it does today.

**A browser.** Planned for later, and outside the first release's scope. The same editor would run in a browser page.

## What every host must provide

Every host supplies the same few things: the bytes of the document, the fonts and the translations the editor loads, a way to save, a way to
present the daemon's state, and the connection to the daemon. A host that cannot give the editor a writable document opens it read-only, and says
so. A host never gives the editor its own copy of the document logic, so a change behaves the same in every host.
