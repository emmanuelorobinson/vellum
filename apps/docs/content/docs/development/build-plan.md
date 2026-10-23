---
title: "Build plan"
description: "The numbered prompts the product was built from, the phases they belong to, the invariants they keep, and where the open items are recorded."
---

{{brand}} was built from a single build guide, divided into 72 numbered prompts. Each prompt is one bounded piece of work with a test to write first, a
verification to run and a record to keep. The prompts are grouped into phases, and the repository's runbooks name them, so this page explains the labels.

## The phases

| Phase | Name | What it covered |
| --- | --- | --- |
| 0 | Foundation | The workspace, the dependency rules, the reference repositories and the copy checks |
| 1 | Spikes | Experiments that settled the renderer, the text engine, the document store and the layout checks before the product depended on them |
| 2 | Types and the write path | The document types, the codec, the operations, the guard and the undo |
| 3 | Engine | Layout, text, the scene, the display list, the renderer and the file's assets and libraries |
| 4 | Daemon, tools and agents | The daemon, the tool registry, the MCP server, the write tools, the agents and their sessions |
| 5 | Import and export | Icons, pictures, the importers, and the exporters |
| 6 | Editor user interface | The canvas, the panels, the agent panel and the editor's ports |
| 7 | Hosts and command line | The VS Code extension, the desktop app and the command line |
| 8 | Cloud, for Pro | Accounts, billing, share snapshots, the sync server and the workers |
| 9 | Version 1.1 | Features after the first release: shaders, slides, presentation and more |
| 10 | Release | The release pipelines, the evidence they check, and the acceptance gates |

The phases are an order of work, not a promise that every item is complete. The status of each part is in the [availability](/docs/getting-started/availability)
page, and the detail is in the runbooks.

## The invariants

The guide gives fourteen invariants that every prompt must keep. They are summarised on the [principles](/docs/architecture/principles) page, and every
prompt is checked against them.

## How a prompt is recorded

Each prompt leaves an entry in the repository's `reports.md`. The entry says what was built, what differs from the guide, what failed, what was skipped or could
not be verified, and the exact commands that passed. Anything only a person can do is listed under "needs a person", and an item is removed from that list
when it is done.

## Where the open items are

The open items are kept in `reports.md`, under two headings: engineering work that an agent or a contributor can do, and decisions or provisioning that only a
person can do. Examples are a signing identity, a publisher account, a design decision, or a test lane that needs a machine nobody has yet. The
[release runbook](/docs/operations/runbooks/release) lists the release blockers in detail.

## Reading the prompt labels

Runbooks and decision records use prompt numbers, such as "Prompt 59", because the work was recorded that way at the time. The number tells you which
prompt built a feature and where its entry is in the report history. It does not describe the feature, so read the page for the feature itself.
