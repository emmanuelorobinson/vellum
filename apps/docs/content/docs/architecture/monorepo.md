---
title: "Monorepo"
description: "The layout of the repository: the Rust crates, the TypeScript packages, the applications, the documentation, the scripts and the fixtures, and what each one is for."
---

{{brand}} is one repository with two build systems: Cargo for the Rust crates, and pnpm with Turborepo for the TypeScript packages and
applications. The layout follows the dependency rule: the core depends on nothing, and the programs that run it sit at the edges.

## Rust crates

| Crate | What it is |
| --- | --- |
| `vellum-types` | The data types of a document, the display list and the protocols. It depends on as little as possible |
| `vellum-core` | The engine: operations, the resolver, layout, scene, display list, diff and merge, the document API, and the ports the engine uses |
| `contracts` | The contract suites for every port, and fakes for each one |
| `adapters/*` | One crate per vendor or format: the store, the layout engine, the text engine, the renderer, the codecs, the importers, the exporters, the model clients, the keychain and the script runtime |
| `vellum-wasm` | The composition root for the browser: the engine and the renderer as one WebAssembly module |
| `vellum` | The composition root for the daemon: the tool registry, the MCP and command-line front ends, and the live and headless targets |
| `sync-server` | The composition root for the room service, with its hosted and self-hosted adapters |
| `vellum-testkit` | Test helpers: a stub model endpoint, fixture documents, the layout oracle runner and pixel comparisons |

## TypeScript packages

| Package | What it is |
| --- | --- |
| `types-ts` | Types generated from the Rust types. Never edited by hand |
| `renderer-api`, `renderer-skia`, `renderer-canvas2d` | The renderer interface and its two implementations |
| `host-api`, `host-vscode`, `host-electron`, `host-browser`, `host-fake` | The host interface and each host's implementation |
| `engine-client` | The editor's client for the WebAssembly engine |
| `editor-core` | The editor's controller: tools, commands and registries. It has no framework and no renderer |
| `editor-react` | The editor's interface |
| `auth`, `billing`, `storage`, `mail`, `jobs`, `telemetry` | Wrappers around the cloud vendors, which are the only code that touches them |
| `skills`, `styles`, `prompt-packs`, `design-systems` | The built-in skills, visual styles, prompts and starter design systems |
| `ui-tokens`, `config` | The shared design tokens, and the brand and the exact copy strings |

## Applications

| Application | What it is |
| --- | --- |
| `extension` | The VS Code extension |
| `desktop` | The Electron desktop app |
| `account` | The account application, with sign-up and the share viewer |
| `api` | The public API and the background worker |
| `web` | The web application, which serves the same viewer |
| `docs` | This documentation site |

## Everything else

| Folder | What it holds |
| --- | --- |
| `docs/` | The product description, the design specification, the architecture, the decision records, the runbooks, the format specification and the build guide |
| `scripts/` | The repository's tools: builds, checks, generators and release helpers, with their tests in `scripts/__tests__` |
| `fixtures/` | Golden documents, reference images, layout-oracle pages and other test inputs |
| `refs/` | Read-only clones of reference repositories, which are not committed |
| `spikes/` | Research spikes that proved a decision before the product code was built |

## Ownership of the specifications

The product description, the design specification and the format specification are read-only. A checksum guards them, and the build fails when
one changes. The architecture changes only by adding a decision record and updating the version baseline. The [workspace](/docs/development/workspace)
page explains what a contributor may change.
