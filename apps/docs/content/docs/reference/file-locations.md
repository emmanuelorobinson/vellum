---
title: "File locations"
description: "Where {{brand}} keeps its files on your computer: the daemon's private folder, the cache, recovery copies, drafts and the sessions database, and the files beside each design."
---

## Your home folder

`~/.vellum` holds everything the daemon keeps for you. Its folders are private to your user.

| Path | What it holds | Keep? |
| --- | --- | --- |
| `~/.vellum/run/` | The daemon's runtime files: `daemon.json` with the local token, the socket, and the lock files | Yes, but never print or copy `daemon.json` |
| `~/.vellum/sessions.db` | Your agent sessions: messages, steps, usage and turn states | Yes, if you want the record |
| `~/.vellum/cache/` | Thumbnails and screenshots, by hash, and cached library copies | Safe to delete while no daemon is running |
| `~/.vellum/recovery/` | Recovery copies of unsaved documents | Until you save, then they are safe to remove |
| `~/.vellum/drafts/` | Designs the desktop app created as drafts | Until you move or delete them |

Do not delete files in `~/.vellum/run/` while a daemon is running. Stop the daemon first with `vellum daemon stop`.

## Beside each design

| Path | What it holds |
| --- | --- |
| `design/app.vellum` | A document, as one file |
| `design/app.vellum` folder, for large documents | The document's index and one file per page |
| `design/app.assets/` | The images and custom fonts the document uses, each named by its content hash |
| `design/library.lib.vellum` | A library document, which other documents import by path |

Commit the design files and their assets folders. Recovery copies are not written beside your design files, so there is nothing to
ignore. If you ever copy one into a repository, keep it out of commits, because it is not part of the design.

## Keychain

Provider keys, tokens and the device sign-in are stored in the operating system's keychain, under the service `vellum`. Nothing in
this list is stored as a file. See [configuration](/docs/reference/configuration).

## Desktop and extension

| Host | Location |
| --- | --- |
| Desktop app | Its own profile folder, with `home.json`: at most two hundred recent designs and two hundred drafts |
| VS Code extension | Its own storage folder, with a copy of the daemon in `bin/`. The copy contains no keys or tokens |

The VS Code extension's storage folder is the place to look if a stale copy of the daemon is running. Stop the daemon before you
remove that copy.
