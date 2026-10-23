---
title: "Getting started"
description: "The three steps to a working {{brand}} setup: build it, make a document, and connect an agent."
---

This section takes you from a clone of the repository to a design that an agent has drawn. It assumes you are
comfortable in a terminal and that you have a project you want to design for.

## What you need

| Requirement | Why | Where it comes from |
| --- | --- | --- |
| Node.js 24 | The web and editor parts of the workspace | `.nvmrc` in the repository |
| pnpm 12 | Installs the JavaScript workspace | The catalog in `pnpm-workspace.yaml` |
| The Rust toolchain 1.99.0 | Builds the command line, the daemon and the editor engine | `rust-toolchain.toml` (installed automatically by `rustup`) |
| A C++ compiler, Python 3 and ninja | The first build compiles Skia from source | The [renderer runbook](/docs/operations/runbooks/renderer) |

You do not need an account to design locally, to export, or to use the command line. Pro features such as share links
need an account, and none is publicly available yet. See the [availability](/docs/getting-started/availability) page.

## The path

1. [Install from source](/docs/getting-started/install). Build the command line, check the daemon, and optionally build
   the VS Code extension or the desktop app.
2. [Make your first document](/docs/getting-started/first-document). Create a `.vellum` file in your repo, draw a frame,
   and export it.
3. [Tour the canvas](/docs/getting-started/canvas-tour). Learn the parts of the editor you will use most.
4. [Connect an agent](/docs/getting-started/connect-an-agent). Register the MCP server with Claude Code, Codex, Cursor
   or another supported client, and ask for a screen.

If something does not work, start with [troubleshooting](/docs/getting-started/troubleshooting).
