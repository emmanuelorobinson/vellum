---
title: "Principles"
description: "The fourteen invariants every change to {{brand}} must keep, and the engineering rules that keep the boundaries clean."
---

The repository's build guide names fourteen invariants. Each one is a promise to a person, and each one has a check that protects it.
A change that breaks an invariant is not finished, however well it works otherwise.

## The invariants

1. **One write path.** Every change to a document is an operation, applied by the core. Nothing else writes to a document.
2. **Every change is attributed.** Each change records who made it: a person, an agent, or a file change.
3. **Undo is yours.** Undo reverses your own changes and never another actor's. Undo this turn reverses one agent turn.
4. **Your edits win, and paused means paused.** A person's edit is never blocked by a claim. A paused agent's next change is refused.
5. **The file is the truth, and it is deterministic.** The same design always gives the same bytes, and a diff shows what changed.
6. **What agents see is what people see.** An agent that reads a frame reads the same layout the canvas draws.
7. **Layout matches the browser.** Layout is checked against a real browser's layout, to half a pixel.
8. **Scripts cannot escape.** A script runs with no network, no files and no timers, and within fixed limits.
9. **Local by default.** Local work needs no account and sends nothing to a service.
10. **Keys live in the keychain.** Provider keys and tokens are never written to a file, a log or a report.
11. **Outside the canvas needs permission.** Reading a file, writing a file, running a command and reaching the network each need a person's answer.
12. **Always interruptible.** A person can stop an agent's turn, and a stop is honoured even while a model is still thinking.
13. **Honest about the machine.** Measurements and tests say what they ran on, and a result is not claimed for a platform that was not tested.
14. **Clean room.** The project's code, text, prompts, skills and interface are its own. Other products are read for ideas, and where a source is
   permitted, ported with attribution.

Each invariant has tests in the repository and, where the risk is high, a fuzzer or an escape corpus. The [quality gates](/docs/architecture/quality-gates)
page lists them.

## Engineering rules

**Dependencies point inward.** The core and its rules depend on nothing outside themselves. Adapters depend on the core, and the composition
roots depend on the adapters. An adapter never depends on another adapter, and the core never depends on an adapter.

**Every port has a fake and a contract suite.** A port is an interface the core uses. Each port ships with an in-memory fake, and a shared test
suite that every implementation must pass. A new adapter is checked against the same suite as the one it replaces.

**Boundaries are versioned.** A name in the tool registry is stable. A schema can gain fields. A breaking change is a new name, so an agent that
worked yesterday still works today.

**Decisions are written down.** A decision that shapes the code gets a numbered record, with its context and its consequences. A later decision
that changes an earlier one says so.

**Time, randomness and identifiers are injected.** The core never reads the clock or generates an identifier on its own. Tests control both,
and so do the composition roots.

## Where the rules are enforced

The dependency rules are checked by the workspace's dependency tools. The invariants that touch the document are checked by the core's tests. The
clean-room rule is checked by a script that scans the repository for denied strings. The [verification](/docs/development/verification) page lists
the commands.
