---
title: "Collaboration"
description: "How people and agents share a design today through Git, and what live collaboration needs before it is available in the editors."
---

Collaboration works today through the repository. A design is a file, so a team shares it the way it shares code: commits,
branches, pull requests and review. The merge driver resolves most concurrent edits to different layers without a conflict,
and the [history, Git and recovery](/docs/using-vellum/history-git-and-recovery) page covers the review tools.

Live collaboration, where people and agents edit the same document at the same moment, is built in part, and is not yet
available in the editors.

## What exists

- **The room service** keeps one room per document. It stores the changes, merges them, and sends them to every connected
  editor. The sync server is a separate program from the daemon, and it can run on your own server.
- **Permissions in the room.** A viewer reads, a commenter adds comments, and an editor changes the document. The server checks
  these on every change, not only in the interface.
- **Capabilities.** A person or a program joins a room with a credential that names one room, one actor, one role and an
  expiry.

## What is not available yet

The VS Code extension and the desktop app open a design through the daemon, and they edit it there. They are not yet attached
to a room. Until they are, a live session with other people is not available in the editors, and the browser checks use
scripted replies, and the server is tested with real connections.

Comments are planned with live collaboration and are not available in this version.

## Self-hosting the room service

The [sync and self-hosting](/docs/operations/sync-and-self-hosting) page explains how to build and run the sync server, which
needs no account. A hosted service does not exist yet.
