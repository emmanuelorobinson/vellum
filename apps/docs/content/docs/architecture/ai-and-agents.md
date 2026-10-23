---
title: "AI and agents"
description: "How models and agents fit around the design tools: model-agnostic engines, the quality levers the product owns, and how a frame becomes code."
---

{{brand}} is built for agents first, and it does not depend on one model. Any client that speaks MCP can design. The built-in agent runs on any
model that a provider adapter supports. The installed Claude Code and Codex run under their own sign-in.

## Engines

An engine turns a request into tool calls. There are three kinds:

- **The built-in engine** runs its own loop against any model that a provider adapter can reach. It receives the canvas tools, not
  unrestricted access to files, commands or the network.
- **Installed engines**, Claude Code and Codex, run the program you have installed and signed in to. Their own tools stay under their own
  permissions, and {{brand}} gives them the canvas tools through the same registry.
- **External clients**, such as Cursor or Gemini CLI, connect to the MCP server and bring their own model. {{brand}} sees only their tool calls.

Each engine records its steps in the same session format, so the agent panel and the history show them the same way.

## What the product owns

A model is only as good as the context it gets. {{brand}} owns the parts of that context that make the difference:

- **Skills.** Instructions the agent reads before it designs, served by the `read_skill` tool and versioned as files, not code.
- **Styles.** Visual recipes the agent can load when a design has no system yet.
- **Notes for agents.** Text on the canvas that the agent reads before it changes a frame.
- **`execute`.** One program that makes many changes, which costs fewer tokens and fewer round trips than many small calls.
- **Self-checks.** The layout check and the screenshot, which let an agent look at its own work before it stops.
- **Plans.** A multi-frame request is planned before it is built, so the work is split and reserved.

Prompts and skills are plain files in the repository, so they are reviewed and versioned like the rest of the project.

## Keys

Provider keys are stored in the keychain and read only where a call needs them. They are never written into a design, a session or a log. The
[providers and keys](/docs/reference/providers-and-keys) reference lists the providers and where each key comes from.

## From design to code

An agent that writes application code reads the frame as a spec. The spec describes the layout, the sizes, the fills and the text, every design
token the frame uses by name, and the components its instances use. The agent then writes the code in the project's own stack. The spec is
deterministic, so the same frame always gives the same spec.

The reverse path, from existing code to a design, is agent-driven: the agent reads the code and recreates it on the canvas with the same tools.

## Token sync

Tokens are not an AI problem. They are variables, and they export to CSS and to a Tailwind theme, and import back, with no model involved.
The mapping is deterministic, so exporting the same variables always gives the same file.

## Evaluation

The project measures agents with fixed briefs, run against each engine and model, and scored on layout problems, token use and a review of the
screenshots. The results guide changes to the skills and the prompts. A change that makes an agent worse is not accepted because it reads better.
