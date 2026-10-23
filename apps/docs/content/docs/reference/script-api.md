---
title: "Script API"
description: "The doc object that execute programs and script nodes call: its methods, its rules, and where its exact signatures are generated."
---

A script that runs through the `execute` tool, or a script node in the design, works on the document through one object, `doc`. This
page describes that object and its rules. The exact TypeScript declarations are generated from the same registry that runs the
calls, and they are on the [script API declarations](/docs/reference/script-api-declarations) page.

## Version

The object reports its version as `doc.apiVersion`, which is `1.0` in this release. A program can check it before it relies on a
method.

## Methods

| Method | Purpose |
| --- | --- |
| `page` | Find a page by its exact name or id. The page must exist |
| `get` | Copy one stored node, including its notes and context |
| `query` | Find nodes by exact type or name, optionally inside one page or subtree |
| `insert` | Add a node under a page or a node |
| `update` | Change properties of a node |
| `copy` | Copy a node and its descendants, with new ids |
| `replace` | Replace a node with another |
| `move` | Move a node to another parent, at an index |
| `delete` | Remove a node and everything inside it |
| `setVariables` | Add, change or remove variables |
| `createComponent` | Make a component from a node |
| `makeSlot` | Mark an empty frame inside a component as a slot |
| `instance` | Place an instance of a local component |
| `findSpace` | Find a free area on a page for a frame of a given size |
| `layoutProblems` | List the layout problems under a node or a page |
| `screenshot` | Draw a node as an image |
| `export` | Export a node as PNG, JPEG, WEBP, SVG or PDF |
| `fillImage` | Fill a node with an image that is already in the document |
| `icon` | Place an icon, named `library:name` |
| `claim` and `release` | Hold a frame against other agents, and let it go |

## Rules

- **One program is one change.** A successful program makes one undo step. An error, a refused change or a limit leaves the document as it
  was.
- **`validate_only` shows the change without making it.** The result lists the prospective document and the counts of what would change.
- **A program is the body of a function.** It must `return` its result, and the result must be plain data.
- **Methods that create return ids.** Keep the id that `insert` returns, and use it in later lines of the same program.
- **Queries return copies.** Changing a copy changes nothing. Use `update` to change a node.
- **Unscoped `query` also visits component definitions.** It does not expand instances.
- **Local instances inherit the definition's size.** An instance of an imported component needs its size given.
- **A program cannot reach the network, the file system or a timer.** Files and remote data must already be in the document.

## Script nodes

A script node is a `.js` file beside the design. It declares its inputs in a header comment, and it returns nodes. It uses the same
methods, except that it cannot change the document. Its output is shown and exported, but it is not saved into the file. Convert the
node to layers when you want to edit them. The [scripts and slides](/docs/using-vellum/scripts-and-slides) page shows a header with each
input type.

## Limits

| Limit | Value |
| --- | --- |
| Time | 5 seconds |
| Calls to `doc` | 2,000 |
| New layers | 2,000 |
| Memory | 64 MiB |
| Source length | 100,000 bytes |

The [execute guide for agents](/docs/agents/scripts-with-execute) gives an example program, and the
[decision record on the execute boundary](/docs/reference/decisions/adr-005-execute) explains why the API is synchronous.
