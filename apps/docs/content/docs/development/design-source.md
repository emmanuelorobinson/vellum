---
title: "Design source"
description: "How the project's own design file is read, copied and checked, how tokens are generated from it, and the rules for changing it."
---

The project's own screens are designed in a separate design tool, and the design file is the specification for the interface. It is read-only.
Code reads it through the design tool's MCP server, on a read-only copy, and continuous integration reads the committed inventory instead.

## The rules

- **Read a copy.** The design file is copied to a read-only copy before anything reads it. Work on the copy. Never write to the original, and never save from
  the design tool.
- **Confirm the document first.** Check that the design tool has the read-only copy open. If it has any other document open, stop and ask.
- **Ask for frames by screen id.** Read only the frames the task names, not the whole document.
- **Take values from variables.** A colour, spacing, radius or type size comes from a variable or a component in the design file. A hard-coded hex value in interface
  code fails the token check.
- **Build only the options that are decided.** The design decisions file records each choice. A choice marked undecided is a question for a person.
- **Compare like with like.** A comparison of a screenshot of the design with one of the implementation uses the same size and the same theme.

## Commands

```sh
pnpm design:open
pnpm design:check
pnpm design:tokens
pnpm tokens:check
```

`design:open` copies the design file and opens the copy. `design:check` checks the committed inventory against the file, and it needs no design tool. `design:tokens`
exports the variables into the generated token files, and `tokens:check` confirms they match the design file. The generated tokens are never edited by hand.

## Snapshots

The design snapshots are images of each screen at each size and theme. Only the inventory is committed, because the images are large. A person regenerates the
images locally with `pnpm design:snapshot`, which needs the design tool to be signed in. CI checks the inventory without the images.

## Changing the design

A change to the design is a change for a person to make in the design tool, and it is reviewed as a change to the specification. When the file changes, the
checksum changes, and the snapshots and tokens must be regenerated before the interface is built against them. A person then runs `pnpm checksum:update`.

The [design source runbook](/docs/operations/runbooks/design-mcp) describes the setup for the design tool's MCP server and the steps when it is unavailable.
The [design tokens runbook](/docs/operations/runbooks/design-tokens) describes how the tokens are generated.
