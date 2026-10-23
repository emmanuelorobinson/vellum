---
title: "Importing"
description: "Bring SVGs, pictures, Figma copies, .pen files and web pages onto the canvas as editable layers, and what each import leaves out."
---

An import puts outside content on the canvas as ordinary layers, in one change. You can undo it like any other change, and
the imported layers get new ids.

## What you can import

| Source | How | Becomes | Left out |
| --- | --- | --- | --- |
| SVG file or text | `import` with a path, or the text | Frames, rectangles, ellipses, vectors, text and images | Gradients keep only the first stop. Patterns, clip paths, masks, filters, linked images and most CSS are left out, and the import says so |
| PNG, JPEG or WEBP | `import` with a path or data | One image layer, at most 1600 pixels on its long side | Pixels are not decoded; the file is stored as it is |
| Figma copy (clipboard HTML) | `import` with the HTML as text | Frames, groups, rectangles, ellipses, lines, vectors and text, with fills, strokes, corners, shadows and blurs | Images, gradients, auto-layout, component links, mixed text styles and blend modes. Zstd copies and FigJam are refused with a reason |
| Figma file, through the REST API | `import` with a Figma URL or key | As for a Figma copy, from the file's data | The same limits. Needs a Figma token. Not verified against the live service |
| `.pen` file | `import` with a path, text or data | Layers with new ids | Read as the public `.pen` format, version 2.21. The source file is not changed |
| Web page or element | the `browser` tool | An HTML import, or a live website layer | Needs Chromium. Only the origins you permit are loaded |

The [imports reference](/docs/operations/runbooks/imports) lists the limits in full.

## From the command line

```sh
vellum tool import --document design/app.vellum --parent <page id> --path assets/logo.svg
```

An import that reads a file outside the canvas asks for your permission. The daemon does not have a prompt yet, so when it
cannot ask, the call fails and suggests passing the content as `data` or `text`, which needs no permission.

## Pasting from Figma

Copy a selection in Figma and paste it into the editor, which reads the clipboard with the same importer that `import` uses.
The conversion is partial by design: it keeps the editable parts and reports the rest. A paste that carries a field the
importer does not know ignores that field.

The Figma decoder was checked against a reference decoder with fixtures, but a live paste from Figma has not been recorded
in the repository. A paste may carry something the importer does not yet understand, so review the result.

## After an import

Imports keep the geometry the source gives them, so check the layout after an import. Then rename the layers you will reuse,
so agents can find them.
