---
title: "Permissions and safety"
description: "What an agent may change without asking, what needs your permission, how frames are protected, and how a refused change is reported."
---

The rule behind every permission in {{brand}} is simple. A change to the canvas never needs permission, because it is
attributed to the agent that made it, it shows on the canvas, and you can undo it. Anything outside the canvas does need
permission: reading a file, writing a file, running a command, or reaching the network.

## What needs permission

| Action | Default |
| --- | --- |
| Read a file in the repository, such as specs, code or CSS | Ask once per session, or allow by a rule |
| Write a file, such as a token export, a script file or an exported image | Ask |
| Run a command | Ask |
| Read a file outside the repository, or reach the network from the built-in agent | Ask every time |

When the editor asks, it shows a permission card in the agent panel with three choices: allow once, allow for this session,
or deny. Skip-all, which allows everything without asking, exists and is off by default. While it is on, the editor shows a
warning that stays on screen.

**Today the daemon has no prompt.** A call that needs a permission it cannot ask for is refused, and the message says what to
pass instead. Reading a file through `path` and searching the stock library through `query` both fail there, so pass the
content as `data` or `text` instead. A refused request is recorded as a denial, and nothing in the daemon grants access
without a person's answer.

## How frames are protected

When an agent claims a frame, no other agent can change the frame until it releases it, or until the claim is idle for
sixty seconds. A change to a claimed frame by another agent is refused, and the refusal names the agent that holds it.
Your own edits are never blocked by a claim, so you can always change what an agent is working on. The claim is a
reservation in the daemon, not part of the design file.

Pause all agents makes every agent's next change fail, even if its model is still running. Stop ends one turn.

## Limits on each change

Every change an agent makes is one transaction, and a transaction is limited to 1,000 operations and 1,000 new layers by
default. A larger change is refused with a message that says to split it.

A script runs in a sandbox with no network, no files and no timers. It can change the document only through the document API,
and it has a time limit, a call limit and a memory limit. A script that goes over a limit changes nothing.

## Keys and secrets

Provider keys live in your operating system's keychain. They are never written into a design file, a session, a log or a usage
report. In continuous integration, a key comes from a variable that is set only when `CI` is set, and it is never written back
anywhere.

## Text in the document is data

A note, a context block, a skill, a tool's result and a library's content are all text the agent reads. The agent treats them as
information about the design, not as instructions it must follow. The security checks include a set of hostile inputs for this
case. The [threat model](/docs/architecture/threat-model) describes the trust boundaries in full.

## Where to read more

The [security and privacy](/docs/operations/security-and-privacy) page lists the checks that exist and the open items, and the
[permissions in the architecture](/docs/architecture/daemon) describe the broker and its policy rules.
