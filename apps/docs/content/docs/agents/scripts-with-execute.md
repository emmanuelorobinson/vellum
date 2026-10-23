---
title: "Scripts with execute"
description: "The execute tool runs a short JavaScript program against the document. One program is one change and one undo step, with strict limits."
---

The `execute` tool runs a short program that uses the document through a `doc` object. A loop over twenty rows, or a set of frames
built from one template, takes one call instead of dozens. The whole program is one change: if any part of it fails, nothing is
applied, and the error names the line that failed.

## The shape of a program

A program is the body of a function. It must `return` its result, and the result must be plain data. An image, from a screenshot
or an export, comes back as an image.

The page must already exist. Create it first with `manage_pages`.

```js
const page = doc.page("Onboarding");
const steps = ["Welcome", "Choose a plan", "Confirm"];
const ids = steps.map((title, index) =>
  doc.insert(page, {
    type: "frame",
    name: `Step ${index + 1}`,
    width: 390,
    height: 844,
    layout: "column",
    gap: 16,
    padding: 24,
  }),
);
return ids;
```

Use `validate_only: true` to see what a program would change without changing anything. The result lists the counts.

## The document API

The `doc` object has these methods. The [script API](/docs/reference/script-api) reference explains each one, and the
[generated declarations](/docs/reference/script-api-declarations) give the exact signatures.

`page`, `get`, `query`, `insert`, `update`, `copy`, `replace`, `move`, `delete`, `setVariables`, `createComponent`, `makeSlot`,
`instance`, `findSpace`, `layoutProblems`, `screenshot`, `export`, `fillImage`, `icon`, `claim` and `release`.

Methods that change the document return the id of what they made, so later lines can refer to it.

## Limits

A program runs with these limits:

| Limit | Value |
| --- | --- |
| Time | 5 seconds |
| Calls to `doc` | 2,000 |
| New layers | 2,000 |
| Memory | 64 MiB |
| Source | 100,000 bytes |

A program that goes over a limit fails and changes nothing. Split a large job into several programs, and keep each one to a
single kind of change, so that a failure is easy to read.

## Frames and claims

A program can claim and release frames. The claims it makes are taken at the document when the change applies, and the releases
are made then too. A frame another agent holds refuses the change, and the error says so.

## Checking a program

Run it with `validate_only` first. Then run it for real and look at the frame with `screenshot`. If the result is wrong, change the
program and run it again. Editing a program is cheaper than undoing a half-finished batch.

The [scripts](/docs/using-vellum/scripts-and-slides) page describes the sandbox, which has no network, no file access and no timers.
