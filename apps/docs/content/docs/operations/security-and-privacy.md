---
title: "Security and privacy"
description: "What stays on your computer, what leaves it and when, how keys are stored, what the checks test, and the security items that are still open."
---

## What stays on your computer

Everything you do locally stays on your computer: the documents, the sessions, the thumbnails, the drafts and the command-line work.
Local features never contact the account service. Telemetry is off until you opt in.

## What leaves your computer

Some features reach outside, and each one asks first:

- **A model provider**, when you use the built-in agent. The request carries the conversation and the canvas context the agent needs.
  The provider is the one you chose, with the key you supplied.
- **A stock library or an image provider**, when you search or generate. Each origin needs your permission.
- **A Figma API**, when you import from Figma with a token. Each origin needs your permission.
- **A web page**, when you capture one. Only the origins you permit are loaded.
- **The account service and the sync server**, only for Pro features and live collaboration you choose to use.

An installed agent such as Claude Code or Codex uses its own network access under its own sign-in. {{brand}} does not change that.

## Keys

Provider keys and tokens live in the operating system's keychain. They are never written to a design file, a session, a log, a usage
report or a share. In continuous integration, a key comes from a variable that is set only when `CI` is set, and it is never saved.

## Permissions

Changes to the canvas never need permission. Reading or writing a file outside the canvas, running a command and reaching the network do.
The [permissions and safety](/docs/using-vellum/permissions-and-safety) page explains the defaults and what the daemon does today.

## Scripts

A script runs in a sandbox with no network, no files and no timers, with time, call and memory limits. Its inputs are the document's own
values.

## The checks

The repository runs its security checks with one command for each part:

```sh
pnpm security:check
pnpm security:sbom
pnpm security:fuzz
pnpm security:canary
```

`security:check` runs the boundary tests, the escape corpus, the dependency rules and the advisory policy. `security:canary` runs a real
turn against a fake provider and scans everything it wrote for a planted key, and fails if it finds one. Each check writes its results to
`target/security`. A result from an earlier run is not evidence about the current code.

The [threat model](/docs/architecture/threat-model) names the trust boundaries, and the
[security runbook](/docs/operations/runbooks/security) lists the checks for each boundary.

## Open items

These are known and recorded in the repository:

- The sharing endpoint has no credential of its own that is separate from the local client. A decision about whether publishing needs an
  approval step or a host capability is still open.
- The dependency advisory checks report open advisories, some with no patched version available. They are reviewed, and none has been
  ignored silently.
- Native acceptance on Windows and Linux, signing, and a remote continuous-integration run have not been completed.
