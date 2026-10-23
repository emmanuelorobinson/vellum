---
title: "Commands"
description: "The commands for installing, building, testing, linting, generating and checking the {{brand}} workspace, and what each one does."
---

Run these from the repository root, unless a row says otherwise. The telemetry of the build tools should be off: set
`TURBO_TELEMETRY_DISABLED=1` and `NEXT_TELEMETRY_DISABLED=1`.

## Install and build

| Command | What it does |
| --- | --- |
| `pnpm install` | Installs the JavaScript workspace, with Node 24 and pnpm 12. Rust comes from `rust-toolchain.toml` |
| `pnpm build` | Builds the Rust workspace, the WebAssembly module, and every package's build |
| `node scripts/skia-build.mjs build -p vellum` | Builds the command line and the daemon |
| `pnpm wasm:build` | Builds the WebAssembly module that the editors run |

## Test and check

| Command | What it does |
| --- | --- |
| `pnpm test` | Runs the Rust tests, the script tests and every package's tests |
| `pnpm type-check` | Checks the Rust code, and every package's TypeScript |
| `pnpm lint` | Runs Biome, the copy check, the token check and Clippy with warnings denied |
| `pnpm format` | Formats the TypeScript with Biome and the Rust with rustfmt |
| `pnpm format:check` | Reports formatting that would change, without changing it |
| `node scripts/skia-build.mjs test -p vellum --test write_tools` | Runs one Rust test target |

## Generated code and specifications

| Command | What it does |
| --- | --- |
| `pnpm types:gen` and `pnpm types:check` | Regenerates the TypeScript types from the Rust types, and checks they match |
| `pnpm format-spec:gen` and `pnpm format-spec:check` | Regenerates the format specification, and checks it matches |
| `pnpm doc-api:gen` and `pnpm doc-api:check` | Regenerates the script API declarations, and checks them |
| `pnpm checksum` | Checks that the read-only specification is unchanged |
| `pnpm parity:check` | Reports the parity inventory's active and pending rows |

## Repository policy

| Command | What it does |
| --- | --- |
| `pnpm deps:check` | Checks the dependency rules for the Rust core and for the packages and applications |
| `pnpm cargo:deny` | Runs the Rust advisory, licence, ban and source checks |
| `pnpm clean-room` | Scans the repository for denied strings, and checks each ported file's attribution |
| `pnpm copy-lint` | Checks that the exact copy strings match the guide, and that no interface repeats them |
| `pnpm clean` | Removes build output. It never removes `node_modules`, `refs/` or `.design-cache/` |

## Design and tokens

| Command | What it does |
| --- | --- |
| `pnpm design:open` | Copies the design file to a read-only copy and opens it in the design tool |
| `pnpm design:snapshot` | Regenerates the design snapshots. Needs the design tool signed in, so run it locally |
| `pnpm design:check` | Checks the snapshots and inventory without the design tool. CI runs it |
| `pnpm tokens:check` | Checks that the generated tokens match the design file |
| `pnpm tokens:lint` | Checks contrast, and that the interface uses tokens rather than colour literals |
| `pnpm ui-icons:gen` and `pnpm ui-icons:check` | Regenerates the icon paths the interface uses, and checks them |

## Specialised lanes

| Command | What it does |
| --- | --- |
| `pnpm layout-oracle` | Lays out the layout fixtures in headless Chrome, and writes the expected boxes |
| `pnpm refs` | Clones the reference repositories at their pins. About 2 GB, and it needs the network |
| `pnpm security:check` | Runs the security checks. See the [security and privacy](/docs/operations/security-and-privacy) page |
| `pnpm performance:native` and `pnpm performance:browser` | Runs the performance measurements. Release builds only |
| `pnpm release:status` | Lists the release blockers, with the evidence for each one. It never reports the local state as a release |

## The documentation site

Run these from `apps/docs`, or with `--filter @vellum/docs` from the root.

| Command | What it does |
| --- | --- |
| `pnpm --filter @vellum/docs dev` | Starts the site on port 3002, after generating its pages |
| `pnpm --filter @vellum/docs build` | Builds the site |
| `pnpm --filter @vellum/docs start` | Serves the built site on port 3002 |
| `pnpm --filter @vellum/docs test` | Regenerates the pages, runs the site's checks, and runs its unit tests |
| `pnpm --filter @vellum/docs type-check` | Checks the site's TypeScript |

Most lanes are described on the [verification](/docs/development/verification) page, with what each one needs.
