---
title: "The big picture"
description: "The parts of {{brand}} and the three rules that hold them together: one document engine, every change as an operation, and no vendor inside the core."
---

{{brand}} has three parts that a person touches, and one that runs everything.

- **The editor.** The canvas, the panels and the agent panel. It is one bundle that runs the same way in VS Code, in the desktop app
  and, later, in a browser. Its engine and its renderer are one WebAssembly module.
- **The daemon.** A native process on your computer. It owns the documents, runs the tools, starts the agents' turns and writes the files.
  The command line and the MCP server are the same program.
- **The cloud.** Optional services for the Pro features and for live collaboration: an account service, a public API, a sync server and
  background workers. Local work never needs them.

The editor and the daemon talk over a private local connection. Agents talk to the daemon through the MCP server, which runs in the same
program. The cloud is reached only through actions a person takes, such as publishing a share snapshot from the command line or the
desktop app.

## The three rules

**There is one document engine.** The core, `vellum-core`, holds the rules for a design: what a change is, what it may do, what its
layout is, and how it is drawn. The editor runs that core as WebAssembly. The daemon, the command line and the cloud workers run the same
core natively. A rule written once behaves the same everywhere.

**Every change is an operation.** Whether a person drags a layer or an agent sends a batch, the change is an operation that the core
applies. Nothing else writes to a document. A script, an importer, a merge and a file change in the repository all go through the same
path, so the same checks, attribution and undo apply to each of them.

**The core knows no vendor.** Every third-party part, such as the data store, the layout engine, the text shaper, the renderer, the storage,
the model providers and the billing service, sits behind an interface the project owns. Each program has one place, its composition root,
where it chooses which implementation to use. Replacing a vendor means writing a new adapter and changing one composition root, not
rewriting the core.

## Why it is shaped this way

Designs are read and written by people and by agents, in an editor, on a server and in a pull request. Keeping the rules in one engine
means each of those surfaces agrees with the others. Keeping the vendors outside the engine means the project can change a vendor without
changing a design file, and without changing what an agent may do.

The [principles](/docs/architecture/principles) page lists the rules that every change must keep, and the
[core modules](/docs/architecture/core-modules) page describes the engine's parts.
