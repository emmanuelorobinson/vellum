---
title: "Make your first document"
description: "Create a .vellum file in your repo, add a frame from the command line, and export it to an image."
---

This walkthrough makes one document with one screen, from the command line, so you can see every step. The editor and
the agents use the same steps underneath.

## 1. Create the file

A document is one `.vellum` file. The folder that holds it must exist first.

```sh
mkdir -p design
vellum new design/app.vellum
```

`new` refuses to overwrite an existing file. The new document has one page, named `Page 1`.

## 2. Read its pages

Every change to a document needs a page or a frame id. Ask the document for its pages:

```sh
vellum tool open_document --path design/app.vellum --json
```

The result lists each page with its `id`. Copy the id of the page you want.

## 3. Add a frame

The write tools take a compact spec. This one adds a 390 by 844 frame with a title inside it:

```sh
vellum tool insert_nodes --document design/app.vellum --parent <page id> \
  --spec '<frame name="Home" width={390} height={844} layout="column" gap={16} padding={24}><text size={32}>Welcome</text></frame>'
```

Each call is one change. Either every node in it is added, or none is. The result lists the ids of what was created.
The `insert_nodes` entry in the [MCP tools](/docs/reference/mcp-tools/changing) reference documents the spec, and the
JSON form that the spec compiles to.

## 4. Check the layout

Layout problems are the things an agent should fix before it stops: text that overflows its box, children that are
clipped, siblings that overlap, and layers outside every frame.

```sh
vellum tool snapshot_layout --document design/app.vellum --json
```

An empty problem list means nothing to fix.

## 5. Export the frame

```sh
vellum export design/app.vellum --frame Home --format png --scale 2 -o home.png
```

`--frame` takes the name of a frame, or its id. Names must be unique. The export writes an image and does not change
the document. The [export formats](/docs/reference/export-formats) reference lists the other formats, including HTML
with Tailwind classes and PDF.

## 6. Open it in the editor

If you built the VS Code extension, open `design/app.vellum` in VS Code. The file opens in the canvas. Changes made from
the command line appear in the open editor, and changes you make in the editor are saved when you save the file.

If the editor shows the file as read-only, the daemon cannot hold it. The [troubleshooting](/docs/getting-started/troubleshooting)
page covers that case.

## Where to go next

- [Using {{brand}}](/docs/using-vellum) explains pages, frames, components and variables.
- [Connect an agent](/docs/getting-started/connect-an-agent) lets an agent draw the next screen while you watch.
