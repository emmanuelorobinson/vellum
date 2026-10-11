# Reports

What a person should know after each prompt: what was built, what was decided on the way, what was skipped or
could not be verified, and what only a human can do. AGENTS.md tells every session to read this file first and
to update it at the end of each prompt. Newest prompt last. Keep entries short and factual; say what failed.

## Needs a person

Open items, newest first. Remove an item when it is done and say so in the prompt's entry.

- **Decide how to commit `docs/design/snapshots/`** (Prompt 3). It is 101 MB: 504 PNGs at 1x and 2x for 252
  screen frames. Options: commit as is, Git LFS, or drop the 2x set. Nothing from Prompt 3 is committed yet.
- **Run `pen login`, then `pnpm design:snapshot` once** (Prompt 3). The CLI path has only run against a fake
  `pen` in tests. If it works, the output should match the committed inventory byte for byte. If it does not,
  the likely cause is the `execute({ input: "<json string>" })` line format in `scripts/lib/pen-queries.mjs`.
- **Fill in `docs/design/DESIGN-DECISIONS.md`** (Prompt 3). All ten rows are UNDECIDED. A UI prompt that needs
  one must ask.
- **Settle six Component library entries with no reusable component in `vellum.pen`** (Prompt 3): Script node,
  Script section, blend mode menu, Canvas nodes for collaboration, Notes for agents, Guidelines editor. Either
  add them to the design or change DESIGN.md. They are recorded in `docs/design/DESIGN-GAPS.md`.
- **Decide the licence** (Prompt 1). ARCHITECTURE open question 11. There is no LICENSE file and no `license`
  field until you do.
- **Add distinctive strings to `scripts/clean-room-denylist.txt`** (Prompt 2): pen.dev, Penpot, tldraw. The file
  is empty, so that half of the clean-room check does nothing yet.
- **Revisit TypeScript 7** (Prompt 2). The repo is on 5.9.3 because dependency-cruiser 18.5.0 cannot read 7.
- **Push to a remote** (Prompt 2). CI has never run. It now runs cargo-deny, the dependency, clean-room and
  copy checks, and `pnpm design:check`.

## Prompt 1: monorepo scaffold

Committed as `d3e46d9`. Entry written later from the session notes, so it is brief.

- Cargo workspace, pnpm workspaces, Turborepo, crate and package shells, AGENTS.md, CI skeleton.
- Scripts for later prompts exit non-zero with "not implemented yet" instead of pretending to pass.

## Prompt 2: dependencies, reference repositories, copy checks

Committed as `498f6a8`. Entry written later from the session notes, so it is brief.

- Version baseline and ADRs added under `docs/architecture/`. Dependency rules for Rust (allowlist, cargo-deny)
  and for `packages/` and `apps/` (dependency-cruiser). Clean-room check and copy lint.
- Pins changed from Prompt 1: Rust 1.98.1 to 1.99.0 (C1) and `@types/node` 26.6.5 to 24.19.2 (C2).
- TypeScript stayed at 5.9.3 instead of the guide's 7.0.x (C3).
- The guide itself was edited once: §3.2 R3 `main.rs` became `src/main.rs`, because the path check found the file
  sits in `scripts/gentest/src/`.
- `refs/` is cloned locally (about 2 GB, gitignored). `pnpm refs` needs the network and runs only locally.
- cargo-deny warns about wrapper entries for adapters that do not exist yet. Expected until they land.
- App tests import `brand` from `@vellum/config`, because the copy lint forbids repeating Appendix B strings.

## Prompt 3: design source

Not committed. Verified: `pnpm test:scripts` (120 tests), Biome, `copy-lint`, `clean-room`, `deps:check`,
`checksum --strict`, `design:check` (0 errors, 11 recorded warnings). Rust and TypeScript package gates were
not re-run, because nothing there changed.

Built:
- `scripts/design-open`, `design-snapshot` (+ `design-snapshot.mjs`), `design-check` (+ `design-check.mjs`), with
  their logic in `scripts/lib/design-frames.mjs`, `design-check.mjs` and `pen-queries.mjs`. All pen.dev `execute`
  syntax lives in `pen-queries.mjs`; `design-snapshot` reads the current `execute` page first and stops if the
  functions it uses are gone.
- `docs/runbooks/design-mcp.md`, `docs/design/DESIGN-DECISIONS.md` (Appendix G, all UNDECIDED),
  `docs/design/DESIGN-GAPS.md` (14 lines), `scripts/design-component-aliases.json`.
- `docs/design/snapshots/`: `inventory.json` (313 frames, 252 reusable components) and 504 PNGs.
- A CI step for `pnpm design:check`, AGENTS.md command rows, and this file.

Things that differ from the guide or could not be done as written:
- **Frame naming.** `vellum.pen` names frames `C03 · Agent workspace · building · 1512 · hc-light`, has no pages,
  uses one variable axis called `appearance`, and names components `Component / Family / State`. The guide says
  otherwise (§2.1). The guide ranks the design file above itself, so the check follows the file and
  `DESIGN-GAPS.md` records each difference.
- **The inventory's `page` field is `null`**, and the smoke step "frames on the Editor Agent page" could not be
  done as written, because the format has no pages.
- **The snapshots were made through the Pencil MCP, not the pen CLI**, because the CLI is not logged in on this
  machine. The same read-only query and `Export` calls were used, and the transcription of the query output was
  checked against a digest computed inside the app.
- **Smoke test** (get_app_state, read_skill, a read-only frame listing, a C03 light 1280 screenshot saved to
  `.design-cache/`) passed apart from the page step.
- Five variables (`vellum-ink`, `-muted`, `-line`, `-paper`, `-teal`) are not named `group/name`. Only the landing
  page frame uses them.
- The pen.dev app still has `.design-cache/vellum.readonly.pen` open.
- Two test bugs and one real defect were fixed along the way: `parseGaps` was reading prose in the gaps file's
  header as a recorded gap, and now only reads dated lines.
