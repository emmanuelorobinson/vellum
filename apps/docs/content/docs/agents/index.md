---
title: "Agents"
description: "How coding agents connect to {{brand}}, what they can do to a design, and how their changes stay attributed, permitted and undoable."
---

An agent is any program that changes a design through the MCP server: Claude Code, Codex, Cursor, a script in continuous
integration, or the built-in agent in the editor. They all use the same tools, and every change they make is treated the
same way.

## What an agent can do

An agent can read the whole design: its pages, layers, components, variables, fonts and the layout problems. It can change
it in batches, where each call either applies every change or applies none. It can claim a frame to work on it alone for a
while. It can export, look at a screenshot of its own work, and read the skills and styles that tell it how to design.

It cannot change a file outside the canvas, run a command, or reach the network unless you give it permission. The
[permissions and safety](/docs/using-vellum/permissions-and-safety) page explains exactly what needs your permission and
what happens when it does not get it.

## How a change is treated

Every change carries the agent's name, its session and its turn. The canvas shows the agent beside what it changed. You can
undo a single turn, or pause every agent at once. A refused change is reported with the reason and the item that caused it,
so the agent can fix it and try again.

## The guides

- [Connect clients](/docs/agents/connect-clients): the configuration file and entry for each supported client
- [Working with the tools](/docs/agents/working-with-the-tools): the order an agent should follow for a design change
- [Scripts with execute](/docs/agents/scripts-with-execute): one program that makes many changes in one step
- [Several agents](/docs/agents/several-agents): claims, reservations and how agents avoid working on the same frame
- [Skills and styles](/docs/agents/skills-and-styles): the instructions and visual styles an agent reads before it designs

The [tool reference](/docs/reference/mcp-tools) lists every tool, its parameters and whether it changes the design.
