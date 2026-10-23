---
title: "Tour the canvas"
description: "The parts of the {{brand}} editor you will use first: the canvas, the panels, pages, components, the agent panel and notes for agents."
---

The editor is the same in the VS Code extension and in the desktop app. This tour names the parts you will use first
and what each one is for. The [using {{brand}}](/docs/using-vellum) pages go into the detail, and the
[keyboard reference](/docs/reference/keyboard-shortcuts) lists every default shortcut.

## The canvas

The canvas is a plane where a page's layers sit. A frame is a container with a size, a fill and optional auto-layout.
Screens are top-level frames. Press `F` and drag to draw a frame. Press `Space` and drag to pan with the hand tool.

Selecting a layer shows its handles on the canvas. Agents that hold a frame to work on it show that frame as held, so
you can see where they are working.

## Panels

The editor has four side panels, each with a shortcut:

| Panel | Shortcut | What it is for |
| --- | --- | --- |
| Agent panel | `Alt+1` | Your conversations with agents for this file, the steps they took, and the usage of each turn |
| Layers panel | `Alt+2` | The layers of the current page as a tree, in stacking order |
| Assets panel | `Alt+3` | The icons, pictures and other assets the document uses |
| History panel | `Alt+4` | The changes made to the document, by you and by each agent, with the means to undo them |

`Tab` hides every panel, and `Mod+\` shows or hides them.

## Properties

The properties of the selected layer include its position and size, its layout, its fills and strokes, its text, and any
variables it is bound to. A value bound to a variable shows the variable's name, so changing the variable changes every
layer that uses it.

## Pages

A document has one or more pages. A page is a canvas, or a set of slides. The editor lists the document's pages so you
can move between them, and the page list adds, renames and reorders them. The [documents and pages](/docs/using-vellum/documents-and-pages)
page covers the details.

## Components

A component is a reusable layer tree. An instance shows the component with overrides for its props, and changing the
component changes its instances unless they override the change. The editor lists the document's components and the
libraries it imports from. The [components and instances](/docs/using-vellum/components-and-instances) page explains
props, variants and slots.

## The agent panel

The agent panel is where you talk to an agent about the document. Changes an agent makes appear on the canvas as they
happen, with the agent's name beside them. Stop and pause controls stop an agent, and pause all agents at once. Undo
this turn reverses what one turn changed, and leaves your own edits alone. The
[agents and the panel](/docs/using-vellum/agents-and-the-panel) page describes each control.

## Notes for agents

A note for agents is a text block on the canvas that agents read before they change the frame it sits next to. Use one
to say what a frame is for, which components it should use, or what it must not change. Context blocks work the same
way for a whole page. An agent reads them through the app-state tool.

## The command palette

Press `Mod+/` to open the command palette. It lists the commands you can run now, and it finds layers, pages,
components, variables and sessions in the document by name.
