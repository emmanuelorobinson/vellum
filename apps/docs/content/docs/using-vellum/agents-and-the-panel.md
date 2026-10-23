---
title: "Agents and the panel"
description: "Sessions, turns, plans, parallel work, usage and cost, and the controls for stopping, pausing and undoing an agent's work."
---

An agent works on a design in one of two ways. An external client, such as Claude Code or Cursor, connects to the MCP
server and makes changes through the tools. The built-in agent runs inside {{brand}}, in the agent panel. Both make changes
the same way, so everything below applies to both.

## Engines

An engine is the program that does the thinking.

| Engine | What it uses | Setup |
| --- | --- | --- |
| Built-in | Any model you connect with a provider key | A key in your keychain. See [providers and keys](/docs/reference/providers-and-keys) |
| Claude Code | The Claude Code you have installed and signed in to | Installed on your computer. {{brand}} does not sign in for it |
| Codex | The Codex you have installed and signed in to | Installed on your computer. Verified on a specific version |

The built-in agent receives only the canvas tools. It has no unrestricted access to files, commands or the network. An
installed engine keeps its own tools under its own permissions, and {{brand}} does not turn those on for it.

## Sessions

A session is the conversation about one file. It holds your messages, the plan, each step the agent took with its result,
the usage, and each turn's state. The session is stored on your computer, in the daemon's private folder. It is not written
into the design file, so a conversation never shows up in a diff or a pull request.

Sessions are keyed by the repository and the file's path inside it. Renaming a branch does not lose them. Deleting a session
removes its record and the cached images that no other session uses.

## Turns and plans

A turn is one request and everything the agent does for it. A turn goes from queued to running, and ends done, failed or
stopped. Only a person can stop a turn. A step that runs longer than two minutes is flagged as stuck, but the turn keeps
running until someone stops it.

A request that touches several frames gets a plan first. The plan lists the frames, the pages and what each is for. When you
approve it, the planned areas are reserved. Other agents see the reservation through the app state, so they keep clear of
it. A plan that is never approved leaves nothing in the file.

## Parallel work

An agent can split a turn across lanes, from two to six, which run at the same time. Each lane has its own agent and its own
frame reservations. The modes are:

- **Split** divides the plan's items among the lanes, so each lane builds different frames.
- **Side by side** gives every lane the same brief and its own area, so you can compare the results.
- **Explore layout** and **Explore style** build successive variants that change only the layout, or only the style.

The lanes share one turn, so one undo reverses all of them.

## Usage and cost

Each turn reports the tokens it used, by kind: input, output, and cache reads and writes. The cost is an estimate, worked out
from the price table that ships with {{brand}}, and it is always labelled as an estimate. A model with no price in the table
shows no estimate, rather than a guess.

## Controls

- **Stop** ends the current turn. Changes already made stay.
- **Pause all agents** makes every agent's next change fail, even if its model is still thinking.
- **Undo this turn** reverses every change the turn made, and leaves your own edits and other agents' changes alone.
- **Undo** in the editor reverses your own last change.

The [keyboard reference](/docs/reference/keyboard-shortcuts) lists the shortcuts for these controls.

## From the command line

The same agent turns run from the command line, for scripts and continuous integration:

```sh
vellum prompt design/app.vellum "Add a settings screen" --engine builtin --model <provider/model> --json
```

The [command-line reference](/docs/reference/cli) lists the options, including the usage report and the parallel modes.
