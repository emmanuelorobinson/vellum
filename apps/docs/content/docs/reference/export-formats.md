---
title: "Export formats"
description: "Each export format, its options, what it keeps and what it changes, and the limits on how large an export can be."
---

Every format below is available from the command line with `vellum export`, and from the editor with the export command. An export
writes a file, or returns the bytes to the caller. It never changes the document.

## Formats

| Format | Use it for | Options | What it keeps |
| --- | --- | --- | --- |
| PNG | Screens, previews and documentation | Scale 1, 2 or 3 | The rendered frame, with transparency |
| JPEG | Photographs and flat images, where size matters | Scale 1, 2 or 3 | Transparent areas are placed on white, at quality 95 |
| WEBP | Web images that need transparency and a small size | Scale 1, 2 or 3 | Transparency, at quality 100 |
| SVG | Vector artwork you will edit in another tool | None | Shapes, paths and embedded images, in canvas units |
| PDF | Documents and slide decks | None | One page per frame, in selection order. Slides keep deck order. Vectors stay vectors, in canvas units |
| HTML and CSS | A standalone component or page, styled with plain CSS | `scaffold`, and `assets` set to `inline` or `files` | Layout, text and solid fills. Complex appearance is embedded as an image, with a warning |
| HTML and Tailwind | The same, styled with Tailwind v4 utility classes | As above | As above, with Tailwind classes instead of a stylesheet |

The HTML formats need no runtime and no network. Their `scaffold` option, on by default, writes a complete page, and turning it off writes a
fragment with its own style element. The `assets` option, `inline` by default, puts images and fonts into the file. `files` writes them
beside it.

Text is drawn in two layers: a rendered picture of the text, and the real text, kept transparent so that it can be selected and read.
Bundled fonts are represented by the picture, so an export of a text-heavy frame stays inside the synchronous budget.

## Limits

| Limit | Value |
| --- | --- |
| Frames in one export | 100 |
| Size of one side of a raster image | 8,192 pixels |
| Pixels in one frame | 16.7 million |
| Pixels in one selection | 67.1 million |
| Size of an export bundle | 32 MiB |

A larger export fails and says which limit it reached. Reduce the scale, or split the export into several.

The synchronous tool that agents use has a smaller result budget. A large export asked for through an agent may need to be split, or made from
the command line.

## Several files

Several raster frames in one export come back as one ZIP file, with unique, safe file names. A PDF of several frames is one file, with
one page per frame. An export of several pages at once follows the order of the selection.

## What an export does not do

An export does not run scripts or fetch remote files. It reads the design as it is, including unsaved changes in the editor. A missing font,
image or library fails the export and names it, rather than leaving a gap.

## Tokens are not an export format

Design tokens are a separate command. `vellum tokens export` writes CSS variables, a Tailwind theme, or Design Tokens JSON. See
[command line](/docs/reference/cli#design-tokens).
