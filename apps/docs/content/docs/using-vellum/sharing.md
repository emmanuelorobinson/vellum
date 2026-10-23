---
title: "Sharing"
description: "Share snapshots: a link to a read-only viewer of a design, updated on your command and revoked on your command. Requires a Pro account."
---

Sharing is a Pro feature. It publishes a snapshot of a design to a viewer, at a link you can send to someone who does not
have {{brand}}. Sharing is explicit: nothing is published until you create a link, and nothing changes on the link until
you update it.

> **Availability.** Sharing is built, and its server parts are tested against a local stand-in. No public account service
> exists yet, so you cannot create a link from a released product today. See the [availability](/docs/getting-started/availability)
> page.

## What a share contains

A snapshot is the design and everything it needs to draw:

- the document, with its pages and layers
- the images and fonts it uses
- the libraries it imports
- the script files of its script nodes, which run only in the native sandbox, never in the browser

A portable ZIP contains the same material, with the document at its path in the repo. Someone who downloads it extracts the
ZIP to a new folder and opens the `.vellum` file in {{brand}}.

## The actions

- **Create share link** publishes a snapshot and returns a link to the viewer.
- **Update snapshot** publishes your later edits to the same link. It is never automatic. Editing a design does not change
  what the link shows until you update it.
- **Delete share link** revokes the link. Anyone who opens it afterwards gets nothing. A copy someone already downloaded
  stays with them.

Before a snapshot is uploaded, the share dialog checks the document and its files. It names any missing file and asks
whether to share anyway. The current viewer leaves out the frames that depend on a missing file, so it never shows one
incomplete.

## Who can see a link

A link is public to anyone who has it. Before you create one, check the audience, the frames you are about to publish,
and any text in notes for agents, because a note is part of the document. The
[security and privacy](/docs/operations/security-and-privacy) page explains what the server keeps.

## Limits

A snapshot is bounded: 64 MiB for the whole upload, 2,048 files, 256 libraries and 100 top-level frames shown in the viewer.
A script can run at most 32 times while the snapshot is prepared. A user has at most 1,000 share records. Uploads expire
after 24 hours if they are not completed.

## Requirements for the person who runs the server

Sharing needs a private storage bucket and the account service's settings. Those are in the
[accounts and sharing](/docs/operations/runbooks/accounts) and [sharing](/docs/operations/runbooks/sharing) runbooks.
