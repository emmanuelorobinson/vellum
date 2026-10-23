---
title: "Verification"
description: "The gates a change must pass, the tagged lanes that need extra tools, and how to read a result that was skipped rather than passed."
---

A change is verified when the gates that cover it pass on the commit you propose. A gate that was skipped is not a pass. This page says which gates
cover which changes, and what the tagged lanes need.

## Always

Before a change is proposed, these checks run for any change:

- `pnpm checksum`: the specification is unchanged
- `pnpm clean-room`: no denied string, and each ported file has its attribution
- `pnpm copy-lint`: the exact copy strings are unchanged
- `pnpm format:check` and `pnpm lint`: the formatting and the lints pass
- `pnpm type-check`: the types check in every package

## For a Rust change

Run the tests of the crate you changed, then the wider suite:

```sh
node scripts/skia-build.mjs test -p vellum-core
node scripts/skia-build.mjs test -p vellum
node scripts/skia-build.mjs clippy --workspace --all-targets -- -D warnings
```

A change to a tool also needs its schema snapshot reviewed. The protocol test fails when a schema changes, and the diff shows what changed.
Regenerate the snapshot, read the diff, and keep the change only if it is additive, as the
[tool registry](/docs/reference/decisions/adr-021-tool-registry) decision requires.

## For a TypeScript change

```sh
pnpm --filter @vellum/editor-core test
pnpm --filter @vellum/editor-react test
pnpm --filter @vellum/docs test
```

A change to the shared tokens also runs `pnpm tokens:check` and `pnpm tokens:lint`.

## For a change to the format

A change to the file format runs the codec's round-trip tests, and the format's specification must match the types:

```sh
pnpm format-spec:check
pnpm types:check
```

## The tagged lanes

Some tests need things an ordinary run does not have. They are tagged, and they are skipped unless you run their lane. A skipped lane is reported
as skipped. It is not a pass.

| Lane | What it needs |
| --- | --- |
| Chrome | A Chromium, named by `VELLUM_CHROME`. Used for the layout oracle and the browser checks |
| Native IPC | Permission to open local sockets, and the daemon binary |
| Electron | The pinned Electron runtime, a desktop session and the native daemon |
| VS Code | A VS Code installation, named by `VELLUM_VSCODE_EXECUTABLE` |
| Installed engines | Claude Code or Codex signed in on your computer. Their tests are marked as ignored, so run them with the ignored-test flag |
| Keychain | The operating system's keychain. Linux needs an unlocked secret service |
| Fonts | The pinned fonts, fetched once from the network |
| Renderer and GPU | A machine with a GPU, for the golden renders and the performance budgets |

The lane's own documentation names the exact command. Each runbook in the [runbook library](/docs/operations/runbooks) gives the lanes for its part of
the product.

## Reading a result

A pass is a pass for the platform and the lane it ran on. A result from one machine is not presented as a result for another. When a check fails,
keep the output, and read the first failure rather than the last one. Most later failures come from the first.
