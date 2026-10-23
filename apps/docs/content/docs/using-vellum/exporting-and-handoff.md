---
title: "Exporting and handoff"
description: "Export images, PDFs, SVG and HTML from the command line or the editor, export design tokens, and hand a frame to a developer or an agent as a spec."
---

Exporting turns a frame into a file, and handoff gives the people and agents who write the code what they need to build it.
Export never changes the document.

## Export a frame

From the command line, name the frame or give its id:

```sh
vellum export design/app.vellum --frame Home --format png --scale 2 -o home.png
vellum export design/app.vellum --frame Home --format pdf -o home.pdf
vellum export design/app.vellum --frame Home --format html-tailwind --scaffold -o home.html
```

Several `--frame` values export in the order you give them. Without `--frame`, the document's pages are exported. A
multi-image export can come back as a ZIP file, and the JSON result reports its type.

The formats are PNG, JPEG, WEBP, SVG, PDF, HTML with CSS and HTML with Tailwind. Raster formats take a scale of 1, 2 or 3.
Vector and code formats keep the canvas units. The [export formats](/docs/reference/export-formats) reference lists the
options and the limits.

## Export from the editor

The editor exports the current selection with `Mod+Shift+E`. It copies a selection as an image with `Mod+Shift+C`, and as
HTML with `Mod+Alt+C`. The [keyboard reference](/docs/reference/keyboard-shortcuts) lists these with the rest.

## Design tokens

The document's variables export to a token file, so the code can use the same values:

```sh
vellum tokens export design/app.vellum --format tailwind-v4 > app/theme.css
```

The formats are CSS variables, a Tailwind v4 theme block, and Design Tokens JSON. Import reads a token file back into the
document, as one change, and `--dry-run` shows what it would change first:

```sh
vellum tokens import design/app.vellum app/theme.css --format tailwind-v4 --dry-run --json
```

## Spec for code

An agent that writes the application code reads the frame as a spec. The spec lists the frame's layers with their layout,
sizes, fills and text; every design token the frame uses, by name, with its value in each theme; and the components its
instances use. The `get_spec` tool returns it, and the agent writes the code in your stack from it.

A spec describes the design. It does not describe your application's behaviour, its routes or its data.

## Handoff to a person

Copy as image or as HTML gives a developer something to paste into a pull request or a design note. The HTML is a standalone
fragment, or a full page with its styles, and works offline.

For a whole design, export the pages to PDF, or share the repository. The design file and its assets are in the repository,
so a developer who clones it has the design itself.
