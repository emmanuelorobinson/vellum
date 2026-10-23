---
title: "Several agents"
description: "How several agents, or several sessions of one agent, work on different frames at once, and what happens when two of them want the same frame."
---

Several agents can work on one design at the same time. Each MCP connection is its own agent, named after the client that opened
it, so you can tell at a glance which agent made which change. The coordination is simple: a frame belongs to whoever claimed it,
and everyone else waits or works elsewhere.

## Claims

`claim_frame` reserves a frame. While the claim is held:

- the claimant can change the frame and the layers inside it
- another agent's change to the frame is refused, and the refusal names the agent that holds it
- your own edits in the editor are never blocked, so you can always change what an agent is working on

A claim lasts until it is released with `release_frame`, or until it has been idle for sixty seconds. An agent that finishes
should release its frames. An agent that stops should not leave them held, and the timeout will free them if it does.

## Plans and reservations

For a request that touches several frames, the engine proposes a plan first: the frames, the pages and what each one is for.
When the plan is approved, the planned areas are reserved as placeholders. Every agent sees them in `get_app_state`, and
`find_space` keeps new frames clear of them. A plan that is never approved reserves nothing, and it leaves nothing in the file.

## Parallel lanes

A turn can run as several lanes at once, in the modes the [agents and the panel](/docs/using-vellum/agents-and-the-panel) page
describes. Each lane is its own agent with its own claims, so lanes never change each other's frames.

## Pausing and stopping

Pause all agents, and every agent's next change is refused, even if its model is still running. That is the fastest way to stop
a run that is going wrong, because it works on every agent at once.

Stop ends one turn. Only a person can stop a turn, so an agent cannot decide on its own that another agent should stop.

## Notes for agents

Each agent reads the notes for agents that apply to the frame it is changing. Notes are the place to record a decision that the
next agent must respect. An agent that finds a note it cannot follow should report that with `report_progress`, rather than
change the frame against the note.

## Reporting

`report_progress` tells the person what is done and what comes next. It changes nothing in the design. Use it at the end of each
frame, so the person can follow several agents without reading every change.
