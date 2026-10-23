---
title: "Backups and recovery"
description: "What to keep to protect your designs: the repository is the history, recovery copies cover unsaved work, and the daemon's folder holds sessions you may want to keep."
---

A design in {{brand}} is a file in your repository, so the repository is the backup. Commit your designs as you commit code, and push
them to a remote. Everything else in this page is about the cases a commit does not cover.

## What to keep

- **The repository.** The design files, their assets folders, and the token files you export. This is the design's history.
- **The daemon's folder, `~/.vellum`.** It holds sessions, the cache of thumbnails and libraries, the recovery copies, and drafts. Sessions
  are the record of agent conversations. They are not needed to open a design, but they are not recreated either. Keep the folder if you
  want that record.
- **Exports you care about.** A PDF or an image export is a copy of a frame at one moment. Keep the ones you hand to someone outside the
  project.

## Recovery copies

While the editor has changes that are not saved, the daemon writes a recovery copy of the document to `~/.vellum/recovery`, every few
seconds. The copy is not history, and it does not replace saving. If the editor or the machine stops before you save, the editor can
restore the copy the next time you open the file.

Save before you rely on a change. Once the file is saved and committed, Git holds it.

## Drafts

A draft is a new design that the desktop app created in `~/.vellum/drafts`. It is kept there until you move it into a repository or delete
it. A draft that exists but is not in the app's list is found again at start-up.

## Deleting

Removing a recent design from the desktop app's list keeps the file. Removing a file from disk removes it. Removing a session removes its
record and the cached images that no other session uses.

Do not delete the daemon's files while the daemon is running. Stop it first with `vellum daemon stop`.

## Restoring from Git

To go back to an earlier version of a design, check out the commit that holds it, or revert the commit that changed it. The
[history, Git and recovery](/docs/using-vellum/history-git-and-recovery) page covers the review tools that help you choose the version.
