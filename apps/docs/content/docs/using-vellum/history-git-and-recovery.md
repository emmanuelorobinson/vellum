---
title: "History, Git and recovery"
description: "How undo works per person and per agent, how the history panel reverts a change, how Git holds the design's history, and where recovery copies go."
---

{{brand}} keeps three kinds of history, and each one answers a different question.

| Question | Where the answer lives |
| --- | --- |
| What did I just change, and what can I take back? | Undo, and the history panel, in the editor |
| What did agents change, and when? | The history panel, and the session's steps |
| What did the design look like last week? | Git, in your repository |

## Undo

Undo reverses your own last change with `Mod+Z`, and redo puts it back with `Mod+Shift+Z`. Undo is per person, not per
document, so it never reverses an agent's change, and an agent's undo never reverses yours.

Undo follows the change through the document as it is now. If someone changed the same property later, undo keeps that later
change. If a layer was built on top of the one you are undoing, the layer stays. Where a change cannot be taken back cleanly,
the editor says so instead of guessing.

**Undo this turn** reverses everything one agent turn did, and nothing else. It is available from the agent panel.

## The history panel

The history panel lists every accepted change to the open document, with who made it, when, and on which page. An entry an agent
made can be reverted. Reverting applies the inverse of that change as one new change, attributed to you, so the record shows the
revert as well. A revert that later changes depend on is refused, with a reason, and nothing is recorded.

## Git

Your repository is the long-term history. Commit the `.vellum` file and its assets folder the way you commit code, and every
design change is in the history, with its author and message.

{{brand}} can make the review better in two ways:

- **`vellum git setup`** writes the repository's `.gitattributes` and sets the Git configuration that the design files need. It
  changes the repository, so run it deliberately, and review the change before you commit it.
- **`vellum diff BASE..HEAD`** compares two commits and draws the frames that changed. With `--png`, it writes the images to an
  output folder, which you can attach to a pull request. The comparison uses exactly the two commits you name, not their merge
  base or your working tree.

A merge of two branches that changed the same design uses the merge driver. The driver merges by layer and by property, so two
changes to different properties of one layer merge cleanly, and a change beats a removal. When both branches change the same
property, the driver keeps both versions in a conflict layer, so no change is lost silently.

Git review works without signing in to anything.

## Recovery copies

While the editor has changes that are not saved, the daemon writes a recovery copy of the file to `~/.vellum/recovery`, every few
seconds. After a crash, the editor offers to restore the copy. A recovery copy is not history. It is kept outside your
repository, in your home folder, so it never appears in a commit.

Save the file to make the change permanent. Git is the history; the recovery copy only covers the time between a crash and the
next save.

## When the file changes outside the editor

If you check out another branch, merge, or let an agent edit the file directly, the daemon notices, re-reads the file, and merges the
change with the editor's unsaved changes. A change that the daemon itself wrote is recognised and ignored.
