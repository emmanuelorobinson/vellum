---
title: "Documents and pages"
description: "How a {{brand}} document is stored, how pages and slides work, the folder format for large files, and libraries."
---

## The document

A document is one `.vellum` file. The file is JSON written in a fixed order, with a fixed indent and normalised numbers,
so the same design always produces the same bytes. That is what makes a design diff in Git, merge with the merge driver
and read well in a pull request.

Create one with the command line:

```sh
vellum new design/app.vellum
```

The parent folder must exist. `new` refuses to overwrite an existing file. An agent can make a document too, with the
`open_document` tool and `create`.

The file records its format version. A file written by a newer version of {{brand}} is refused, with the version it
needs, rather than being read wrongly.

### Stable identifiers

Every layer, page, component and variable has an id that does not change when the layer moves or is renamed. A moved
layer therefore shows in a diff as a move, not as a delete and an insert. Agents refer to layers by id, so an id that
changes would break their references.

### Positions and rounding

The file rounds positions, sizes and radii to one hundredth of a pixel, opacities and gradient stops to one thousandth,
and angles to one hundredth of a degree. The rounding happens only when the file is written, so the editor works with
full precision and the file stays stable.

## Pages

A document has one or more pages. A page is either a **canvas**, where you draw freely, or a set of **slides**, where
the order of the frames is the order of the deck.

- Add a page from the editor, or with `manage_pages` from an agent. Pages are added at the end unless you give an index.
- Rename or change a page's kind with `update`. A page with layers that its new kind does not allow is refused.
- Move a page with `move`, and remove it with `delete`. Removing a page removes every layer on it, so read it first.

Use `manage_pages` for batches: it applies inserts first, then updates, moves and removals, all as one change.

## Slides

On a slides page, a top-level frame is a slide. The order of the frames is the order of the deck. Notes, context blocks
and prompts on a slides page are for editing, and are not slides.

Presenting and exporting slides as a PDF are covered in [scripts and slides](/docs/using-vellum/scripts-and-slides).

## The folder format

A small document is one file. A large one can be a folder instead, with an index file and one file per page. The folder
keeps each page in its own file, so two people can edit two pages without a conflict, and a diff shows which page changed.

The file format suggests the folder format at about 10,000 layers or 4 MB. Nothing requires it: a single file always
works.

## Libraries

A library is a document of components and variables, saved as a `.lib.vellum` file. Another document imports it by
path, and its instances follow the library's components. A local override always wins over a library change.

If a library moves, the editor shows it as missing and offers to locate it. Instances keep rendering from the last
copy the editor cached, marked as missing.

## Keeping files in the repo

Commit the `.vellum` file the way you commit code. Commit the `<name>.assets` folder beside it when the document uses
images or custom fonts, because the file refers to those by content hash.

The daemon writes recovery copies to `~/.vellum/recovery`, not beside your files, so they never appear in Git. Git is the
history; the [history and recovery](/docs/using-vellum/history-git-and-recovery) page explains the difference.
