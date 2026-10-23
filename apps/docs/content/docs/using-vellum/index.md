---
title: "Using {{brand}}"
description: "The parts of a design in {{brand}}, how they fit together, and the order to read the guides in."
---

A design in {{brand}} is a set of ordinary files in your repo. The parts below are the ones the editor, the agents and
the command line all work with. Read them in this order; each page builds on the one before.

| Part | What it is | Read |
| --- | --- | --- |
| Document | One `.vellum` file. Deterministic JSON that diffs and merges in Git | [Documents and pages](/docs/using-vellum/documents-and-pages) |
| Page | A canvas, or a set of slides, inside a document | [Documents and pages](/docs/using-vellum/documents-and-pages) |
| Frame | A container with a size, a fill and optional auto-layout. Screens are top-level frames | [Frames and layout](/docs/using-vellum/frames-and-layout) |
| Layer | Anything on a canvas: frames, groups, text, shapes, paths, images and instances | [Frames and layout](/docs/using-vellum/frames-and-layout) |
| Component | A reusable layer tree with props and variants. An instance overrides some of its props | [Components and instances](/docs/using-vellum/components-and-instances) |
| Variable | A named colour, number, string or boolean, with a value per theme | [Variables and themes](/docs/using-vellum/variables-and-themes) |
| Library | A document of components and variables that other documents import | [Components and instances](/docs/using-vellum/components-and-instances) |
| Session | A conversation with an agent about one file. It is stored on your computer, not in the file | [Agents and the panel](/docs/using-vellum/agents-and-the-panel) |
| Note for agents | A text block that agents read before they change the frame it is beside | [Tour the canvas](/docs/getting-started/canvas-tour) |

## What is in the file and what is not

The document file holds the design: its pages, layers, components, variables, libraries and the fonts and images it
uses. It does not hold your conversations with agents, your keys, your usage, or the frames an agent is holding at the
moment. Those live on your computer, in the daemon's private folder.

Images and custom fonts are stored beside the file, in a folder named after it, so the file and its assets move
together. The [file layout](/docs/reference/file-layout) reference describes that folder.

## The guides

- [Documents and pages](/docs/using-vellum/documents-and-pages): files, pages, slides and the folder format
- [Frames and layout](/docs/using-vellum/frames-and-layout): frames, auto-layout, grouping, and arranging layers
- [Components and instances](/docs/using-vellum/components-and-instances): props, variants, slots, libraries
- [Variables and themes](/docs/using-vellum/variables-and-themes): tokens, aliases, theme axes, and tokens in CSS and Tailwind
- [Text and fonts](/docs/using-vellum/text-and-fonts): text layers and the fonts they use
- [Images and icons](/docs/using-vellum/images-and-icons): icon libraries, pictures, and the image tools
- [Importing](/docs/using-vellum/importing): SVG, pictures, Figma, `.pen` files and web pages
- [Exporting and handoff](/docs/using-vellum/exporting-and-handoff): images, HTML, tokens and specs for code
- [Sharing](/docs/using-vellum/sharing): share snapshots, which need a Pro account
- [Agents and the panel](/docs/using-vellum/agents-and-the-panel): sessions, turns, parallel work and cost
- [Permissions and safety](/docs/using-vellum/permissions-and-safety): what an agent may do without asking
- [History, Git and recovery](/docs/using-vellum/history-git-and-recovery): undo, Git review and the recovery copies
- [Scripts and slides](/docs/using-vellum/scripts-and-slides): script nodes and presentation
- [Collaboration](/docs/using-vellum/collaboration): live editing, and what is not available yet
