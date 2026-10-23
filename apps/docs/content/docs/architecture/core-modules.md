---
title: "The core modules"
description: "The parts of the document engine, what each one owns, and the ports each one uses."
---

The core is `vellum-core`. It is pure: it does not read the file system, the network, the clock or random numbers. Those come in through
ports, which the composition root supplies. The modules below are the engine's parts.

| Module | What it owns | Ports it uses |
| --- | --- | --- |
| **Operations** | The only way to change a document: insert, update, delete, move, copy, replace, variables, components, instances, slots, scripts, grouping, booleans, alignment, reordering, flipping and scaling. Each operation is checked against the document before it is applied | Text and path engines, for operations that measure a shape |
| **Guard** | The checks before an agent's transaction: whether the actor is paused, whether a frame is claimed by someone else, and whether the batch is within its limits. A refusal is typed and goes back to the agent with a reason | None |
| **History** | Per-actor undo and redo, and undo this turn. Each commit records its inverse and the parts of the document it touched | None |
| **Layout check** | The problems an agent should fix: overflowing text, clipped children, overlapping siblings, layers outside every frame, and hug cycles that cannot resolve | The text engine |
| **Resolver** | Variables for each theme, component instances, props, variants, overrides, slots and libraries. It produces a tree of resolved occurrences, each with its address and any problems. An unresolved reference is reported, never guessed | Library sources |
| **Layout** | The layout tree from the resolved occurrences, with hug, fill and fixed sizes resolved before any engine-specific style is applied. Updates are incremental | The layout engine and the text engine |
| **Scene** | One scene for each page: selection, visibility and locking, clips, snapping, and a search for free space. It works on bounds, not on the exact shape | The spatial index |
| **Display list** | The visible occurrences as a versioned list of drawing commands, with the resources they need. It culls conservatively, so a shadow or a stroke near the edge is still drawn | Fonts, path geometry and the text engine |
| **Diff and merge** | The difference between two documents as operations, and a three-way merge by identifier and property | None |
| **Document API** | The `execute` programs and script nodes. One registry of methods also generates the TypeScript declarations | The script runtime and the services |
| **Files** | Atomic writes, the cleanup of unused assets on explicit save, fonts, libraries and their cache, and the folder format | The file system, the asset store and the codec |
| **Sessions and turns** | The session event log, usage by token kind, the cost estimate, and the turn state machine | The session store |
| **Export** | Vector, code and token exports from the same registries the tools use | The token and spec formats |

## Why the layout is computed in the core

Layout decides where every layer is. If the editor and the daemon computed it differently, an agent would see a different design from the
one a person sees. So the layout rules live in the core, and the layout engine is an adapter behind a port. An export, a screenshot, a
layout check and the canvas all use the same result.

## The display list

The renderer does not read the document. It reads the display list, which the core builds from the visible layers. The list is a compact
binary buffer, and it stays in the renderer's memory, so a frame needs only the view transform from the editor. This keeps the drawing path
short and makes the list the one place where the renderer's behaviour is defined.

## Where to read more

The [decision records](/docs/reference/decisions) for the layout, the scene and the display list are the detailed sources. The
[document model](/docs/architecture/document-model) page describes what these modules store.
