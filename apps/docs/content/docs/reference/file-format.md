---
title: "File format"
description: "How a {{brand}} document is shaped: its JSON structure, stable identifiers, rounding, versions, assets, and the generated specification."
---

A `.vellum` document is JSON, written in a fixed order, so the same design always gives the same bytes. This page describes the
shape of the format. The complete, generated specification, with every field, is on the [file format specification](/docs/reference/file-format-spec)
page, and the way a document is laid out on disk is on the [file layout](/docs/reference/file-layout) page.

## The top level

A document has a metadata block, the themes it defines, its variables, its fonts, its components, its pages, and the libraries it imports.
A library is a document with the kind `library`, saved as `.lib.vellum`.

## Identifiers

Every page, layer, component and variable has an identifier, and an identifier never changes. A layer that moves keeps its identifier, so a
diff shows a move, and an agent's reference to a layer stays valid. Identifiers are created by the daemon. A file that someone else edits by
hand should keep the identifiers it already has.

## Rounding

The codec rounds values when it writes the file, and not before. Positions, sizes and radii round to one hundredth of a pixel. Opacities,
ratios and gradient stops round to one thousandth. Angles round to one hundredth of a degree. Rounding at write time keeps the file
stable, and the editor keeps full precision while you work.

## Versions

Each document records the version of the format it was written in. A version of {{brand}} refuses a file written in a newer version, and it
says which version the file needs. A file from an older version is migrated when it is read, and the migration is covered by a fixture.

## Assets

Images and custom fonts are stored beside the document, in a folder named after it, and each is named by the hash of its contents. The
document refers to an asset by that hash. An asset that nothing refers to is removed only when you save explicitly, and an asset the undo
history can still bring back is kept.

## Writing and reading

Writes are atomic: the file is written to a temporary file beside the target, flushed to disk, and then renamed over the target. A reader that
finds a file it cannot parse reports the problem and does not guess. The [file layout](/docs/reference/file-layout) page describes the folder
form that large documents use.

## Editing by hand

You can read the file, and you can diff it. Edit it by hand only when you must, because the editor and the agents keep references that a hand
edit can break. If you do edit it, run the layout check and open the file in the editor before you commit.
