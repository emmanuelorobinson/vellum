---
title: "The document model"
description: "What a design is in {{brand}}: the types, how a document is stored and undone, the libraries it imports, and how external changes and Git merges reach it."
---

## The types

A document is a tree. Its top level holds metadata, the themes it defines, its variables, its fonts and its libraries, and then its
pages in order. Each page holds the layers on it. A layer is a node with a kind, such as a frame, a text, a shape, a path, an image, a
group, a boolean or an instance, and it holds its children. A component is stored once, as a tree of its own, and an instance refers to it.

Nodes refer to one another by identifier, not by position in the tree. A variable is referred to by its identifier, so renaming a variable
does not break a layer that uses it.

## How a document is stored

The engine keeps the document in a store behind a trait. The store used today is built on a conflict-free replicated data type, which
means concurrent changes merge. The engine keeps its own undo, on top of that store, so undo does not depend on the store's internals.

Undo is per actor and per turn. The history records each change's inverse, and replays it against the document as it is now. A later change
by someone else to the same property is kept. A layer that someone built on top of the change stays in place. Where a change cannot be
taken back cleanly, the undo says so.

## The file

The file is JSON with a fixed key order, a fixed indent and normalised numbers. It is versioned and it is refused, not guessed, when it is
newer than the program. The [file format](/docs/reference/file-format) reference describes its shape, and the
[file layout](/docs/reference/file-layout) reference describes the folder form that large documents use.

## Libraries

A library is a document of components and variables, saved with the library extension. A document imports a library by path. An instance of a
library component keeps a link to the library, so when the library changes, the instance follows, unless the instance has overridden the change.

Pasting between documents has two choices: link to the library, which adds the import, or paste independent copies. A variable that clashes by
name is resolved by the person, not by the engine.

When a library cannot be found, its instances keep rendering from the last copy the editor cached, marked as missing.

## External changes and Git

A file can change outside the editor: a checkout, a merge, or an agent that writes the file directly. The daemon's watcher notices, reads the
file, and checks whether the change is one it wrote itself. If it is not, the daemon runs a three-way merge between the last file it knew, the
engine's document with any unsaved edits, and the new file. The result is applied as operations.

The merge driver that Git uses does the same three-way merge, by layer and by property. A change beats a removal. When the same property changed
on both sides, the driver keeps both versions in a conflict layer, so neither version is lost without a trace.

## Limits

A transaction is limited to 1,000 operations and 1,000 new layers by default. The limits protect the engine from a runaway batch.
