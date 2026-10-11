# Vellum

> Design where your code lives.

An open-source design canvas that lives in your repo, where you and your coding agents design side by
side. Designs are plain, diffable `.vellum` files; the editor runs in VS Code, Cursor, Windsurf, a
desktop app and (for share links) the browser; the same Rust core powers the CLI and an MCP server so
agents can design with the app open or closed.

Vellum is a working codename.

## Status

Prompt 2 of the build guide: the reference repositories, the version baseline, and the dependency,
clean-room and copy checks. Every crate, package and app still exists as a shell with a smoke test. There is
no document model, no editor and no daemon behaviour yet.

## Requirements

- Node 24 and pnpm 12 (`packageManager` pins pnpm; recent pnpm switches to it automatically)
- Rust, installed through [rustup](https://rustup.rs); `rust-toolchain.toml` pins the toolchain and the
  wasm targets and rustup installs them on first use
- `cargo-deny` 0.20.2 for `pnpm cargo:deny`: `cargo install cargo-deny --locked --version 0.20.2`
- Docker, only for the cloud apps' dev services (`docker compose -f docker-compose.dev.yml up -d`)

## Quick start

```sh
pnpm install
pnpm build
pnpm test
```

See `AGENTS.md` for every command and the rules for working in this repo, and
`docs/implementation/vellum-implementation-guide.md` for the build plan. The versions in use are recorded in
`docs/architecture/version-baseline.md`. `pnpm refs` clones the reference repositories into `refs/` (optional,
about 2 GB, needs the network); `docs/reference/REFERENCE-INDEX.md` says what to read in them.

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
