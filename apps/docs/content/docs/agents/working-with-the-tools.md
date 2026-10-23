---
title: "Working with the tools"
description: "The order an agent should follow to change a design: read first, claim the frame, change in batches, check the layout, and release."
---

The tools are simple on their own. The sequence matters more: an agent that reads first, changes in batches and checks its
work produces designs a person can use. This page describes that sequence, and the tools each step uses.

## 1. Look at the state

Call `get_app_state` first. It lists the open documents, the active one, the selection and the viewport, the frames other agents
hold or have planned, the paused agents, and the notes for agents that apply to the selection or the page.

## 2. Read the rules

Call `read_skill` for the format and script references, and for the workflow that matches the brief. A design system's guidelines
are read the same way, and they take precedence over a style recipe. `list_skills` lists what is available. The
[skills and styles](/docs/agents/skills-and-styles) page explains the order of precedence.

## 3. Read the design

Use the reading tools before you change anything:

- `get_nodes` returns a subtree by id, down to a depth you choose.
- `query_nodes` finds layers by name, type, text, component or variable.
- `find_properties` lists every value a property takes in a scope, such as the font sizes a page already uses.
- `get_variables` and `list_components` show the tokens and components to reuse.
- `snapshot_layout` shows the computed bounds and the layout problems.
- `screenshot` draws a page or a layer as an image you can look at.

Reuse an existing variable or component before you add a new one. A design that already uses `color/text` should keep using it.

## 4. Claim the frame

Call `claim_frame` on the frame you are about to change. Other agents then cannot change that frame until you release it. A claim
that is idle for sixty seconds is released by itself, so keep working or claim again.

`find_space` finds a free area for a new top-level frame, so it does not land on another frame.

## 5. Change in batches

Use the typed tools for one kind of change, and `execute` for a program that makes many changes:

- `insert_nodes` adds layers, from JSON or from a compact spec.
- `update_nodes` sets properties on layers.
- `move_nodes`, `copy_nodes`, `replace_nodes` and `delete_nodes` do what their names say.
- `replace_properties` changes every matching value in a scope, such as turning `#111111` into a variable.
- `set_variables` adds, changes or removes variables.
- `arrange_nodes`, `instance_nodes`, `manage_components` and `manage_pages` change structure, instances, components and pages.

Each call is one change. A refused call changes nothing, and its error names the item that caused the refusal, its field, and
the reason. Fix that item and call again.

## 6. Check the work

Call `snapshot_layout` for the problems, and fix each one. Then `screenshot` the frame and look at it. Check the text in
particular, because overflow is the most common problem and the easiest to miss.

## 7. Release the frame

Call `release_frame` when you have finished with it. Then call `report_progress`, so the person sees what is done and what
comes next. `report_progress` changes nothing in the design.

## Export and spec

When the design is ready for code, `get_spec` returns the frame's layout, its tokens and its components for the agent that
writes the code. `export` returns an image or a PDF of the frame, and `export_tokens` writes the variables as a token file.

The [tool reference](/docs/reference/mcp-tools) lists every tool with its parameters.
