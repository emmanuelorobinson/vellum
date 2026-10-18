# AGENTS.md

Rules for every coding-agent session in this repository. Read this first, then the guide.

Vellum is an open-source design canvas that lives in your repo, where people and coding agents
design side by side. Everything runs on one Rust core (`vellum-core`); the TypeScript editor, the
`vellum` daemon/CLI/MCP binary and the hosts sit around it. Vellum is a working codename; the brand
lives in `@vellum/config` and `vellum-types::brand` only.

## Read, in this order

1. This file.
2. `reports.md` (what earlier prompts left open, decided or could not verify; "Needs a person" first).
3. `docs/implementation/vellum-implementation-guide.md` (the canonical build guide). One session
   builds one prompt. §11 is the global TDD contract; §4 is the fourteen product invariants.
4. The sources the prompt names: `docs/product/PRODUCT.md`, `docs/design/DESIGN.md`,
   `docs/architecture/ARCHITECTURE.md`.
5. The reference-repo paths in guide §3.2 for any hard problem the prompt lists (see "Reference code").

Source priority when documents disagree is guide §2: PRODUCT, DESIGN.md, `vellum.pen`,
DESIGN-DECISIONS, ARCHITECTURE, ADRs, format spec, the guide, tests, code. Where guide §8 and
ARCHITECTURE §13 differ on names or paths (`ui-tokens` vs `ui`, `docs/architecture/decisions/` vs
`docs/adr/`), the guide wins.

## Read-only files

These are READ-ONLY. Never modify them, never "fix" them, never let a build or test touch them:

- `docs/product/`
- `docs/design/DESIGN.md`
- `docs/design/vellum.pen`
- `docs/format/` (regenerate it with its script; never hand-edit)
- `refs/` (cloned reference repositories)

Related rules:

- `docs/design/DESIGN-GAPS.md` is append-only: one line per design-file vs DESIGN.md mismatch.
- `docs/architecture/` changes only by adding ADRs and the version baseline.
- `docs/design/snapshots/` and `packages/types-ts/src/generated/` are only ever regenerated.
- `pnpm checksum` proves the specification is unchanged. Run it after a build or test run. A human
  who changes the specification on purpose runs `pnpm checksum:update`.

## Reading the design (Pencil MCP)

The design is `docs/design/vellum.pen`, read through the Pencil MCP, per guide §2.1.

1. Run `scripts/design-open`. It copies the design to `.design-cache/vellum.readonly.pen` and opens
   that copy. Work on the copy, never on the original.
2. Confirm the document with `get_app_state` before anything else. If the active document is not the
   read-only copy, stop and ask.
3. Call `read_skill` first, once per session, and again for the `execute` page, before using
   `execute`. Never hardcode the tool's script syntax in our code or docs.
4. Read only. No insert, update, move, delete or replace; no `spawn_agents`; never save.
5. Ask for frames by screen ID (`C03`, `S07`), only the frames and components the prompt names.
6. Compare screenshots of the design frame and the implementation at the same size and theme.
7. Values (colour, spacing, radius, type) come from the `.pen` variables and components, never from
   eyeballing. A hardcoded hex in UI code is a lint failure.
8. pen.dev's HTML export is a reference, never product code. If the MCP is unavailable, use
   `docs/design/snapshots/` and say so in the PR. UI prompts do not start without at least the snapshots.
9. Build only the options recorded in `docs/design/DESIGN-DECISIONS.md`. An UNDECIDED row means ask.
10. If both the brief and the design are silent on a brand decision, ask. Do not improvise.

## Architecture rules

- Every document change is an Op transaction with an Origin (actor, session, turn), applied by
  `vellum-core`. Nothing else touches `DocStore`. There is no side door for scripts, importers,
  merges or external file changes.
- Vendor crates and packages live only in `crates/adapters/` or in wrapper packages
  (`@vellum/auth`, `@vellum/billing`, `@vellum/storage`, `@vellum/mail`, `@vellum/jobs`,
  `@vellum/telemetry`). Dependencies point inward: core never imports an adapter, adapters never import
  each other, only composition roots (`vellum-wasm`, `vellum`, `sync-server`, `apps/*`) know which
  adapters exist.
- `vellum-core` and `vellum-types` stay pure: no file system, network, clock or randomness. Those come
  in through ports (`Clock`, `IdGen`, `AssetStore`, ...).
- `editor-core` has no React and no renderer. UI packages never import vendor SDKs directly.
- No library's native format is ever the system of record. `.vellum` is our deterministic JSON.
- Model coordinates are `f64`, times are UTC milliseconds, ids are typed newtypes; see guide §9.
- Inject clocks and id generators. No wall-clock or randomness in logic.

## Reference code

- Before implementing a hard problem listed in guide §3.2, read the named paths under `refs/` and put a
  3-5 line note in the PR: what the reference does, what Vellum keeps, what it changes and why.
  Use `rg` on the named paths; never load whole directories.
- Port only from MIT/Apache projects, and only with a header naming the source repo, path and commit
  (`ported-from:`) plus an entry in `THIRD_PARTY_NOTICES.md`.
- Never port from penpot (MPL-2.0: read it, never copy). Never port from tldraw (commercial licence:
  public docs only, never a dependency).
- Never copy anything from pen.dev: not code, text, prompts, skills, API wording, assets, or the HTML its
  export produces. It is our design tool, not a code reference. Vellum's design must not reproduce its
  interface.

## Visual language and copy

- blue = selection. violet = component. agent hues = an agent is here. amber = needs you. red = errors
  and destructive actions. The chrome stays neutral.
- Agents are rounded squares; people are circles. This holds everywhere an actor is shown.
- State is a symbol plus a label, never colour alone.
- Agents are named by hue (built-in, "Teal") or by client (external, "Claude Code"). No personas, no
  faces, no emoji, no exclamation marks, no claims of taste ("stunning", "beautiful"), no "magic".
- Say "on your computer" and "in your repo". Say "cloud" only for features that sync through a Pro account.
- Exact strings are in guide Appendix B and exported from `@vellum/config`; UI packages do not
  duplicate them.
- Example data (Sam, Juniper, Teal, juniper.vellum) appears only in fixtures and stories, marked `TEST DATA`.

## Security and privacy

- API keys and tokens live in the OS keychain only. They never appear in a `.vellum` file, a session
  store, logs, crash reports, MCP results, History, telemetry or `--json` output. The canary scanner is
  law: a hit fails the build.
- No network listener. The daemon listens on a user-only Unix socket or named pipe. TCP exists only behind
  an explicit `--listen` flag.
- No telemetry until the person opts in, and no account requirement for any local feature (editor, CLI,
  MCP, exports, the built-in agent).
- Anything an agent does outside the canvas goes through `PermissionBroker`.
- Dev-tool telemetry stays off too: `TURBO_TELEMETRY_DISABLED=1`, `NEXT_TELEMETRY_DISABLED=1`.

## Working method

Follow guide §11 on every prompt: failing tests first, confirm the failure, smallest coherent change,
re-run, then clippy, dependency rules and type-check for what you touched. Never weaken a test to
make it pass. Preserve unrelated changes. Tests that need a GPU, Chrome, VS Code, Electron, an installed
engine or the network are tagged and say what they need. Scripts that belong to a later prompt exit
non-zero with "not implemented yet" rather than pretending to pass.

## Reports

`reports.md` is the running hand-off for the person who owns the repo.
Update `reports.md` at the end of each prompt, before saying the prompt is done:

- add a `## Prompt N` entry: what was built, what differs from the guide, what failed, was skipped or could not
  be verified (say so plainly), and the exact commands that passed;
- add anything only a human can do or decide to "Needs a person", and remove items that are now done;
- add things the person should know as you go, not only at the end (a failed check, a surprising file, a decision
  you had to make).

Keep it factual and short. Do not record secrets, and do not paste design content.

## Commands

| Command | Does |
|---|---|
| `pnpm install` | install JS dependencies (pnpm 12, Node 24; Rust comes from `rust-toolchain.toml`) |
| `pnpm build` | `cargo build --workspace`, the wasm build, then every package's `build` |
| `pnpm test` | Rust tests, script tests, every package's tests |
| `pnpm type-check` | `cargo check` plus `tsc --noEmit` per package |
| `pnpm lint` | Biome, the copy lint (guide Appendix B), and `cargo clippy -D warnings` |
| `pnpm format` / `pnpm format:check` | Biome and rustfmt |
| `pnpm types:gen` | regenerate `packages/types-ts` from `vellum-types` (ts-rs); `pnpm types:check` fails when it differs (CI) |
| `pnpm format-spec:gen` | regenerate `docs/format/` (the JSON Schema and the reference page) from `vellum-types`; `pnpm format-spec:check` fails when it differs (CI) |
| `pnpm checksum` | prove the read-only specification is unchanged |
| `pnpm clean` | remove build output (never `node_modules`, `refs/`, `.design-cache/`) |
| `pnpm refs` | clone the 15 reference repositories at their pins into `refs/` (about 2 GB, needs the network), then check that `docs/reference/REFERENCE-INDEX.md` is current and every path it names exists. Local only, not in CI |
| `pnpm refs:index` | regenerate `docs/reference/REFERENCE-INDEX.md` from guide §3.2 and §3.3 |
| `pnpm deps:check` | the Rust allowlist for the pure crates (`scripts/rust-deps-allowlist.txt`), cargo-deny bans, licences and sources, and the dependency-cruiser rules for `packages/` and `apps/` |
| `pnpm cargo:deny` | `cargo deny check`, all four checks; advisories need the network |
| `pnpm clean-room` | no file contains a string from `scripts/clean-room-denylist.txt`, and every `ported-from:` header has an MIT or Apache-2.0 row in `THIRD_PARTY_NOTICES.md` |
| `pnpm copy-lint` | `@vellum/config` holds exactly the guide's Appendix B strings, and no UI code repeats them |
| `pnpm design:open` | copy `docs/design/vellum.pen` to `.design-cache/vellum.readonly.pen` (no write bit) and open that copy in pen.dev; `--no-open` only copies |
| `pnpm design:snapshot` | regenerate `docs/design/snapshots/` (PNG at 1x and 2x, `inventory.json`) with the pen CLI on the read-only copy. Needs `pen login` or `PEN_CLI_KEY`; run locally, not in CI. Only `inventory.json` is committed; the PNGs are gitignored |
| `pnpm design:check` | no pen.dev needed: `inventory.json` matches `vellum.pen`'s checksum, every PNG it names exists when any are present locally, every screen, theme and width DESIGN.md asks for exists, names follow the file's convention. Runs in CI |
| `pnpm design:tokens` | export the design's variables to `packages/ui-tokens/source/tokens.json` and generate the CSS and TS tokens; needs `pen login` or an MCP session (`--print-query`, `--raw`). See `docs/runbooks/design-tokens.md` |
| `pnpm tokens:check` | `tokens.json` matches `vellum.pen` and the inventory, generated token files are current, every DESIGN.md difference is accepted in DESIGN-GAPS.md. Runs in CI |
| `pnpm tokens:lint` | contrast (text 4.5:1, icons and focus 3:1, every appearance) and token usage (no colour literals in UI code, selection and component colours never on buttons, agent hues only on agent marks). Part of `pnpm lint` |
| `pnpm --filter @vellum/ui-tokens storybook` | Storybook for the token page (port 6006); `test:storybook` renders every story in headless Chromium. See `docs/runbooks/design-tokens.md` |
| `pnpm ui-icons:gen` / `pnpm ui-icons:check` | write `packages/editor-react/src/icons/lucide.generated.ts` (the Lucide paths the chrome uses, from `crates/adapters/icons/data/lucide.json`); `--check` fails when it differs (list in `lucide-names.json`) |
| `pnpm --filter @vellum/editor-react test:e2e` | Playwright over the editor chrome in Chromium (chrome lane): draw, select, reorder, drop, panels, undo, responsive rules, and the C03, C18 and S13 chrome against `docs/design/snapshots/` (skipped when the PNGs are absent; failures leave comparison sheets in `target/screen-goldens/`) |
| `pnpm layout-oracle` | lay out `fixtures/layout-oracle/*.html` in headless Chrome and write the expected boxes (ADR-002); `--check` compares with the committed ones (CI's chrome lane). Needs Chrome (`VELLUM_CHROME`); see `docs/runbooks/layout-oracle.md` |
| `cargo run -p vellum-codec-json --example canonicalize -- [--check] <files>` | rewrite `.vellum` files in canonical form (how `fixtures/documents/` is made); `--check` writes nothing and fails if a file is not canonical |
| `fixtures:record` | later prompts; exits non-zero until then |

## Layout

```text
crates/        Rust: vellum-types, vellum-core, contracts, adapters/*, vellum-wasm, vellum, sync-server, vellum-testkit
packages/      TypeScript packages (@vellum/*); types-ts is generated
apps/          extension, desktop, account, api, web, docs
docs/          product/, design/, architecture/, implementation/ (the guide)
scripts/       repo tooling and its tests (scripts/__tests__)
fixtures/      layout-oracle/ (Chrome layout oracle, Prompt 6); golden documents and reference images (later)
refs/          reference repositories, gitignored, read-only
```

<!-- BEGIN:turborepo-agent-rules -->

# This is NOT the Turborepo you know

Turborepo configuration, task behavior, and CLI commands can vary between installed versions and may differ from your training data. Resolve the `turbo` package from this file's directory or relevant workspace; in monorepos, it may not be visible from the repository root. For example, run `node -p "require.resolve('turbo/package.json')"` from a workspace that depends on `turbo`.

Read `docs/README.md` inside that installed package first, then read the relevant pages from its `docs/` directory before changing Turborepo configuration or commands. Heed deprecation notices. These bundled docs match the installed package version and are available without network access.

This block is written and re-added by `turbo` before repository-scoped commands when an AI agent is detected. In the Turborepo source repository, its template is defined in `crates/turborepo-cli/src/cli/agent_guidance.rs`. Removing the managed block while updates are enabled means a later qualifying invocation will add it again. Set `"agentGuidance": false` in the root `turbo.json` or `turbo.jsonc` to opt out; this does not remove an existing block. Keep the block committed with your work to avoid an uncommitted change on the next agent invocation.
<!-- END:turborepo-agent-rules -->
