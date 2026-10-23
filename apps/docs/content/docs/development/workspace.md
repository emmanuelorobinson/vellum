---
title: "Workspace"
description: "The layout of the {{brand}} repository, which files are read-only and why, and where each kind of change belongs."
---

## The top level

| Path | What it is |
| --- | --- |
| `crates/` | The Rust crates: the core, its contracts, the adapters, and the composition roots |
| `packages/` | The TypeScript packages: the editor, the renderer interface, the hosts, the cloud wrappers and the shared data |
| `apps/` | The applications: the VS Code extension, the desktop app, the account application, the public API, the web app and this site |
| `docs/` | The product description, the design specification, the architecture, the decision records, the runbooks, the format specification and the guide |
| `scripts/` | The repository's commands and their tests |
| `fixtures/` | Golden documents, reference images and other test inputs |
| `refs/` | Reference repositories, cloned read-only and not committed |
| `spikes/` | Research spikes, kept as evidence for decisions |

## Read-only files

Some files are the specification, and no change to them is ever part of an ordinary task:

- `docs/product/` is the product description
- `docs/design/DESIGN.md` and the design file itself are the design specification
- `docs/format/` is generated from the document types. Regenerate it with its script, and never edit it by hand
- `refs/` holds the reference clones

A checksum guards the specification. `pnpm checksum` fails when one of those files changes during a build or a test run. A person who changes
the specification on purpose runs `pnpm checksum:update`, and that change is reviewed as a specification change.

## Where a change belongs

| A change to | Goes in |
| --- | --- |
| A document rule, such as an operation or a layout rule | `crates/vellum-core` |
| A tool an agent can call | `crates/vellum/src/tools`, with a test, and a regenerated schema snapshot |
| A file format or codec | `crates/adapters/codec-json` and the format decision |
| The editor's behaviour | `packages/editor-core`, which has no framework and no renderer |
| The editor's appearance | `packages/editor-react`, using the shared tokens |
| A vendor | Its adapter crate or wrapper package, and nowhere else |
| The documentation | `apps/docs/content/docs`, or the source the generated pages copy from |
| An architectural decision | A new decision record, and an entry in the version baseline |

## Lockfiles and catalogs

`pnpm-lock.yaml` and `Cargo.lock` record exact versions, and a change to a dependency updates them in the same change. The workspace's
`pnpm-workspace.yaml` holds a catalog of exact versions for every shared tool, so each package refers to one version. The
[version policy](/docs/development/version-policy) page explains how a version is chosen and recorded.

## Ownership

The repository's `AGENTS.md` file is the rules that every change follows, for people and for coding agents alike. Read it before your first change.
