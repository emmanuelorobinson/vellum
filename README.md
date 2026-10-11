# Vellum

> Design where your code lives.

An open-source design canvas that lives in your repo, where you and your coding agents design side by
side. Designs are plain, diffable `.vellum` files; the editor runs in VS Code, Cursor, Windsurf, a
desktop app and (for share links) the browser; the same Rust core powers the CLI and an MCP server so
agents can design with the app open or closed.

Vellum is a working codename.

## Status

Scaffold only (Prompt 1 of the build guide). Every crate, package and app exists as a shell with a
smoke test. There is no document model, no editor and no daemon behaviour yet.

## Requirements

- Node 24 and pnpm 12 (`packageManager` pins pnpm; recent pnpm switches to it automatically)
- Rust, installed through [rustup](https://rustup.rs); `rust-toolchain.toml` pins the toolchain and the
  wasm targets and rustup installs them on first use
- `cargo-deny` (used from Prompt 2 onwards)
- Docker, only for the cloud apps' dev services (`docker compose -f docker-compose.dev.yml up -d`)

## Quick start

```sh
pnpm install
pnpm build
pnpm test
```

See `AGENTS.md` for every command and the rules for working in this repo, and
`docs/implementation/vellum-implementation-guide.md` for the build plan.

## Layout

| Path | What |
|---|---|
| `crates/` | Rust: types, core, port contracts, adapters, the wasm engine, the `vellum` binary |
| `packages/` | TypeScript packages (`@vellum/*`) |
| `apps/` | VS Code extension, desktop app, account app, API, web viewer, docs |
| `docs/` | product, design, architecture and the implementation guide |
| `scripts/` | repo tooling |

## Licence

Not yet chosen (an open question in the architecture document).
