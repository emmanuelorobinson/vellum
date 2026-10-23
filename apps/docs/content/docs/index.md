---
title: "Documentation"
description: "What {{brand}} is, how the documentation is organised, and where to start."
---

{{brand}} is an open-source design canvas in your repo. You and your coding agents design the same files: frames,
components, variables and pages. The files are plain JSON, so they diff in Git, review in a pull request and roll back
with the branch they belong to.

The canvas runs in the editor, in the desktop app and in the command line. The command line is the same program as
the MCP server that agents use, so an agent can design with the editor open or closed.

## Where to start

- **New to {{brand}}?** Read [Getting started](/docs/getting-started), then [Using {{brand}}](/docs/using-vellum).
- **Connecting an agent?** Read [Connect clients](/docs/agents/connect-clients) and [Working with the tools](/docs/agents/working-with-the-tools).
- **Looking something up?** The [reference](/docs/reference) lists the command-line tool, every MCP tool, the script API,
  the file format and the decision records.
- **Running it, or contributing?** Read [Operations](/docs/operations) and [Development](/docs/development).

## Before you begin

{{brand}} is pre-release. No signed public release is published, so the paths in these docs build from source. The
[availability](/docs/getting-started/availability) page says what is ready to try, what is a preview, and what is not
available yet.

## How these docs are built

The pages in this site are written by hand, except for three kinds of page that are copied from the repository on
every build: the runbooks, the decision records and the MCP tool reference. Those pages say where they came from. The
[documenting the site](/docs/development/documenting-the-site) page explains how the site is built and checked.

The site includes an index for agents at `/llms.txt`, which lists every page with its description.
