# Reports

What a person should know after each prompt: what was built, what was decided on the way, what was skipped or
could not be verified, and what only a human can do. AGENTS.md tells every session to read this file first and
to update it at the end of each prompt. Newest prompt last. Keep entries short and factual; say what failed.

## Needs a person

Open items, newest first. Remove an item when it is done and say so in the prompt's entry.

- **Review ADR-002** (Prompt 6). It accepts D2 and fixes Vellum's sizing rules where CSS's defaults disagree with
  hug, fill and fixed (15 rows, each with a fixture). Decisions worth a look: fill shares space equally and ignores its
  content (`flex-basis: 0`, `min-width: 0`); fixed and hug never shrink; fill on the main axis of a hugging parent uses
  its fallback, else hugs; hug on a frame without auto-layout, and fill with nothing to fill, need a fallback; two padding
  values are vertical then horizontal, as in CSS.
- **Define grid tracks and `layoutIncludesStroke` in ARCHITECTURE §4.1** (Prompt 6). ARCHITECTURE gives a grid no
  track fields and does not say what `layoutIncludesStroke` includes. The oracle assumes `columns`/`rows` counts with
  equal tracks and leaves strokes out of layout. Prompts 7 and 18 need the real definitions; only a person edits
  ARCHITECTURE.md.
- **Watch the first CI run of the new `layout-oracle` job** (Prompt 6). It has not run on GitHub yet. It installs Chrome
  for Testing 155.0.8059.39 on Ubuntu and runs `--check` against boxes written by Chrome on macOS. No fixture has text,
  so the boxes should not depend on the OS, but that is unproven until the job passes.
- **Review ADR-004 and ADR-001's new "Text" section** (Prompt 5). ADR-004 keeps Loro behind DocStore and sets the S06
  directory-codec suggestion at 10,000 nodes or 4 MB of `.vellum` JSON. ADR-001 now says: Parley lays text out, Skia
  only draws glyphs, no skia-safe `textlayout`, and every renderer uses Skia's FreeType font manager.
- **Decide how the native FreeType Skia gets built** (Prompt 5). Matching text natively needs Skia built from source with
  FreeType on macOS (rust-skia's prebuilt binaries have only CoreText), plus a one-line patch to bindgen output for the
  macOS 27 SDK (`spikes/renderer/scripts/build-native.sh`). Prompt 22 should either publish Vellum's own binaries or
  build them in CI. Windows was not tried.
- **Undo cost grows with the document** (Prompt 5, ADR-004). Undoing a 200-node batch took 13 ms at 10k nodes and 109 ms
  at 50k, while applying it took under 2 ms. Prompt 14 has to measure per-actor undo and get it under 16 ms at 10k, or
  ADR-004 is revisited. Nothing for you to do now; it is here so it is not lost.
- **Measure the renderer spike on an M1 and a mid-range Windows laptop** (Prompts 4 and 5). Prompt 5's text and
  engine numbers are also from the M5 Pro only. ADR-001's frame and latency
  numbers come from one Apple M5 Pro only; the guide asks for both machines (or a GPU CI lane, which does not exist).
  Copy `spikes/renderer/web/` with its built `dist/` (or build it, see `docs/runbooks/renderer-spike.md`), run
  `pnpm install`, then `node measure.mjs --frames-only` there. It writes `results/frames-<host>.json`. If B is slower
  than A on either machine, ADR-001 says to revisit.
- **Review ADR-001 and update ARCHITECTURE §3.2** (Prompt 4). ADR-001 accepts D1: skia-safe in WASM for the editor,
  the same code natively for headless, no CanvasKit. ARCHITECTURE.md still describes CanvasKit in TypeScript; only a
  person edits it.
- **Decide where the gpu lane runs** (Prompt 4). The gpu-tagged tests (`VELLUM_LANES=gpu`, plus `fonts` for the text
  test) need Chrome with a real GPU, emsdk 4.0.6, ninja and the fixture fonts. CI has no such runner, so they have only
  run on this machine. Prompt 5 added two more lanes, `fonts` (needs `pnpm spike:fonts`, network once) and `wasm`
  (needs emsdk's clang); neither runs in CI either.
- **Save the updated design in pen.dev, then say so** (Prompt 3). The file the app has open,
  `~/.pencil/documents/2c247f85-6e53-4acd-be65-f3662ca89428/vellum.pen`, is byte-identical to
  `docs/design/vellum.pen` (SHA-256 `d82995c9…`, 16,095,916 bytes, modified 20:22). The app state does not show
  whether there are unsaved changes. No other copy turned up in `~/Documents`, `~/Downloads`, `~/Desktop`,
  `~/.pencil` or Spotlight. After a save, the agent copies the file, runs `pnpm checksum:update` and
  `pnpm checksum --strict`, and regenerates the snapshots. `pnpm design:check` fails after a copy until they are
  regenerated.
- **Snapshot PNGs are gitignored now, but still in history** (decided 2026-10-12). Only `inventory.json` is committed;
  `pnpm design:snapshot` makes the PNGs locally, so a fresh clone has none until someone runs it (needs `pen login`).
  The 504 PNGs (86 MB) committed in `b836d9c` stay in history, so clones stay large until history is rewritten
  (`git filter-repo`) and `main` is force-pushed. That is your call.
- **Run `pen login`, then `pnpm design:snapshot` once** (Prompt 3). The CLI path has only run against a fake
  `pen` in tests. If it works, the output should match the committed inventory byte for byte. If it does not,
  the likely cause is the `execute({ input: "<json string>" })` line format in `scripts/lib/pen-queries.mjs`.
- **Fill in `docs/design/DESIGN-DECISIONS.md`** (Prompt 3). All ten rows are UNDECIDED. A UI prompt that needs
  one must ask.
- **Settle six Component library entries with no reusable component in `vellum.pen`** (Prompt 3): Script node,
  Script section, blend mode menu, Canvas nodes for collaboration, Notes for agents, Guidelines editor. Either
  add them to the design or change DESIGN.md. They are recorded in `docs/design/DESIGN-GAPS.md`.
- **Add distinctive strings to `scripts/clean-room-denylist.txt`** (Prompt 2): pen.dev, Penpot, tldraw. The file
  is empty, so that half of the clean-room check does nothing yet.
- **Decide whether the reference clones run in CI** (Prompt 2). `pnpm refs` needs about 2 GB and the network. It
  runs only on a developer machine, so the reference index check and its real-clone test never run in CI.
- **Confirm the guide edit** (Prompt 2). §3.2 R3 now says `src/main.rs` instead of `main.rs`, because the path
  check found the file in `scripts/gentest/src/`.
- **Revisit TypeScript 7** (Prompt 2). The repo is on 5.9.3 because dependency-cruiser 18.5.0 cannot read 7. The
  dependency check now fails if dependency-cruiser cruises zero modules.
- **Decide the licence** (Prompt 1). ARCHITECTURE open question 11. There is no LICENSE file and no `license`
  field until you do. The licence list in `deny.toml` follows from the decision, so update it then.

## Prompt 1: monorepo scaffold

Committed as `d3e46d9`. Entry written later from the session notes, so it is brief.

- Cargo workspace, pnpm workspaces, Turborepo, crate and package shells, AGENTS.md, CI skeleton.
- Scripts for later prompts exit non-zero with "not implemented yet" instead of pretending to pass.

## Prompt 2: dependencies, reference repositories, copy checks

Committed as `498f6a8`, and pushed: `origin/main` now has it. CI for `498f6a8` passed (run 38165213624) in all three
jobs, including cargo-deny, `pnpm deps:check`, `pnpm clean-room` and `pnpm copy-lint`. The old "push to a remote"
item is done and is removed from Needs a person. Entry written later from the session notes, so it is brief.

- Version baseline and ADRs added under `docs/architecture/`. Dependency rules for Rust (allowlist, cargo-deny)
  and for `packages/` and `apps/` (dependency-cruiser). Clean-room check and copy lint.
- Pins changed from Prompt 1: Rust 1.98.1 to 1.99.0 (C1), `@types/node` 26.6.5 to 24.19.2 (C2), and TypeScript
  7.0.2 to 5.9.3 (C3).
- The guide itself was edited once: §3.2 R3 `main.rs` became `src/main.rs`, because the path check found the file
  sits in `scripts/gentest/src/`.
- `refs/` is cloned locally (about 2 GB, gitignored). `pnpm refs` needs the network and runs only locally.
- cargo-deny warns about wrapper entries for adapters that do not exist yet. Expected until they land. The wrapper
  names match the adapter crates' package names (checked).
- App tests import `brand` from `@vellum/config`, because the copy lint forbids repeating Appendix B strings.

Decided on the way:

- No reference repo was re-pinned. None of the 15 has a release at or after its pin (C6).
- ADRs live in `docs/architecture/decisions/`, the guide's path. ARCHITECTURE §2.6 says `docs/adr/`.
- cargo-deny allows workspace path dependencies (`allow-wildcard-paths`). All crates are `publish = false`.
- The clean-room check skips `.pen` files. The design file is encrypted and is read only through the Pencil MCP.
- Two bugs found while testing, both fixed: dependency-cruiser's `exclude` hid `node_modules`, so the vendor-SDK
  rule could never fire; and the TypeScript 7 check cruised zero modules without failing.

Verified on the Prompt 2 tree: `pnpm test`, `pnpm lint`, `pnpm type-check`, `pnpm format:check`, `pnpm checksum`,
`pnpm deps:check`, `pnpm clean-room`, `pnpm copy-lint`, `pnpm refs`, `cargo deny check`. All exited 0. At that
time the script suite had 64 tests, none skipped. CI confirmed the same checks on `498f6a8`.

Not done or open:

- Not in CI, by design: `pnpm refs` and the real-clone index test. That test skips in CI.
- Not installed, by design: most lines of the version baseline (Vite, Tailwind, Zustand, Zod, i18next, Electron
  builder and updater, vsce and ovsx, Playwright, Storybook, better-auth, drizzle, Hono, the MCP SDK, and most
  Rust crates). Each prompt installs its own line and re-resolves it first.
- Versions were resolved on 2026-10-11. They go stale, so re-resolve a line when a prompt first installs it.
- No porting has happened yet, so the `ported-from:` rule has only fixture tests.
- The clean-room denylist is empty, so that check cannot catch anything yet (see Needs a person).
- The licence list in `deny.toml` is provisional until the licence is decided. At the last run cargo-deny warned
  about 9 allowed licences nothing uses, 8 wrapper entries for adapters not yet written, and one duplicate `syn`
  version. I did not investigate the duplicate.
- The licences in `scripts/refs.tsv` match each repo's LICENSE file, checked 2026-10-11. rmcp is mid-transition
  from MIT to Apache-2.0, which is why the guide says "Apache-2.0 (MIT legacy parts)".

## Prompt 3: design source

Committed as `b836d9c`. CI for that commit failed in its TypeScript job at `pnpm exec biome check .`: Biome wanted
`scripts/__tests__/reports.test.mjs` wrapped differently. The fix is formatting only. It is uncommitted in the
working tree, and `biome check .` passes locally. The repo checks job in that run passed, including
`pnpm design:check`.

Verified before the commit: `pnpm test:scripts` (120 tests), Biome, `copy-lint`, `clean-room`, `deps:check`,
`checksum --strict`, `design:check` (0 errors, 11 recorded warnings). Rust and TypeScript package gates were not
re-run, because nothing there changed. Re-checked afterwards: `design:check` passes (11 recorded gaps).

Built:
- `scripts/design-open`, `design-snapshot` (+ `design-snapshot.mjs`), `design-check` (+ `design-check.mjs`), with
  their logic in `scripts/lib/design-frames.mjs`, `design-check.mjs` and `pen-queries.mjs`. All pen.dev `execute`
  syntax lives in `pen-queries.mjs`; `design-snapshot` reads the current `execute` page first and stops if the
  functions it uses are gone.
- `docs/runbooks/design-mcp.md`, `docs/design/DESIGN-DECISIONS.md` (Appendix G, all UNDECIDED),
  `docs/design/DESIGN-GAPS.md` (14 lines), `scripts/design-component-aliases.json`.
- `docs/design/snapshots/`: `inventory.json` (313 frames, 252 reusable components) and 504 PNGs (gitignored since
  2026-10-12; `design:check` checks them only where present).
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

## Prompt 4: Spike A, renderer (ADR-001)

Not committed. Decision: **D1 accepted**, skia-safe compiled to WASM renders the editor and the same drawing code runs
natively; see `docs/architecture/decisions/ADR-001-renderer.md` for every number.

Built:
- `vellum-testkit::pixel_diff` (TDD, 11 tests): RGBA8 images, PNG read and write, comparison with a channel and
  differing-share tolerance, max / p95 / mean delta, and expected, actual and diff PNG artifacts on failure. Adds the
  `png` 0.18.1 crate to vellum-testkit.
- `spikes/renderer/`: a separate Cargo workspace (root `Cargo.toml` excludes `spikes`) and pnpm package
  (`@vellum/spike-renderer`, added to `pnpm-workspace.yaml`). Generated documents of 1,000 / 5,000 / 10,000 nodes,
  20 non-text fixtures, one display list; Renderer A (Rust display list, CanvasKit 0.42.0 in JS), Renderer B
  (skia-safe in WASM over WebGL2 from Rust), the native PNG path; a spike page with pan, zoom, FPS meter and a
  screenshot button; `measure.mjs`; a gpu-tagged test for fixture #1. Measurements in
  `spikes/renderer/results/results.json` (31 KB, committed); the fixture captures and diffs (1.1 MB) are gitignored and
  regenerated by `pnpm spike:measure`.
- `docs/runbooks/renderer-spike.md`; version-baseline rows for skia-safe, canvaskit-wasm, wasm-bindgen, emsdk 4.0.6
  and png (note C15); the `rust-toolchain.toml` comment; Biome ignores `spikes/*/results` (generated JSON).

Headline numbers (M5 Pro, Chrome 155 headless, vsync off): B is 1.9x / 2.5x / 2.6x A's fps at 1k / 5k / 10k nodes
(10k: 232 vs 88.5 fps, frame p95 10.9 vs 18.7 ms) and about half the drag latency at 5k and 10k. A and B are
pixel-identical on all 20 fixtures. Native CPU raster vs WASM CPU raster: within 1. CPU raster vs the GPU: edge
pixels only, max delta 85, p95 at most 1, at most 1.85% of pixels. Size gzip: A 3.02 MB, B 1.55 MB, B with text
layout 2.65 MB. Cold load: A 243 ms, B 224 ms.

Things that differ from the guide, failed, or could not be verified:
- **Only one machine.** No M1, no Windows laptop, no GPU CI lane (see Needs a person). The ADR says so.
- **Skia is built from source for emscripten.** rust-skia's prebuilt emscripten binaries do not link with Rust 1.99
  (JS longjmp vs wasm exceptions). The build needs emsdk 4.0.6 (installed in `~/emsdk`) and ninja (installed with
  Homebrew in this session). First build downloads about 1 GB of Skia sources.
- **The documents' text uses Geist Regular from the installed `next` package** at runtime; no font is committed. Text
  parity is Prompt 5.
- **Test lanes:** there was no convention yet, so this prompt introduced `VELLUM_LANES` (comma-separated). Without
  `gpu` in it the test reports as skipped. Its failure was confirmed (missing build) before it passed.
- The spike uses plain C exports and `unsafe` FFI; the spike workspace does not inherit the product's
  `unsafe_code = "deny"`.
- `cargo fmt` in the spike workspace also formatted `crates/vellum-testkit` (a path dependency); only files from this
  prompt changed.

Verified: `cargo test -p vellum-testkit` (11 pixel-diff tests), `cargo clippy --workspace --all-targets -D warnings`,
`cargo fmt --check`, `cargo test --workspace`, `cargo deny check advisories`, `pnpm deps:check`, `pnpm test:scripts`
(122), `biome check .`, `pnpm copy-lint`, `pnpm clean-room`, `pnpm checksum`; in `spikes/renderer`: `cargo test`,
`cargo clippy -D warnings` (native and emscripten), `VELLUM_LANES=gpu node --test test/fixture1.gpu.test.mjs` (pass:
both paths pixel-identical to native for fixture #1). `pnpm type-check` was not re-run: no TypeScript package changed.

## Prompt 5: Spike B, text parity, Loro at scale, QuickJS in WASM (ADR-004)

Not committed. Prompt 4's work was committed and pushed before this session (`fde1998`, `96b6388`), including the Biome
fix, so that item is gone from "Needs a person".

Built:
- **Text (renderer spike):** 30 text fixtures (`spikes/renderer/crates/scene/src/text_fixtures.rs`, TEST DATA): Latin,
  Arabic, CJK, emoji, mixed bidi, wrapping. Parley 0.12.0 lays them out in Rust (`text.rs`, no system fonts) inside
  each module; a new `GLYPHS` display-list op carries positioned glyphs; Renderer A draws them with CanvasKit
  `drawGlyphs`, Renderer B and the native path with `draw_glyphs_at`. The native path also renders SkParagraph's layout
  of the same text, as a layout cross-check. Fonts: five Noto files pinned by SHA-256 in `spikes/renderer/fonts.json`,
  fetched by `scripts/fetch-fonts.mjs` (not committed, OFL-1.1). `measure-text.mjs` writes `results/text.json`.
- **Loro and QuickJS:** a new spike workspace, `spikes/engine/` (pnpm package `@vellum/spike-engine`, added to
  `pnpm-workspace.yaml`). `spike-loro` maps the generated documents onto a Loro movable tree and measures 1k to 50k
  nodes natively; `loro-wasm` runs the same steps in a `wasm32-unknown-emscripten` module from Node. `spike-script`
  runs rquickjs with a toy `doc` API, limits and an interrupt budget, natively and as a `wasm32-unknown-unknown`
  cdylib that links vellum-wasm (the size baseline). Renderer B also got a size-only `quickjs` feature.
- `docs/architecture/decisions/ADR-004-loro.md`; a "Text (Prompt 5)" section appended to ADR-001;
  `docs/runbooks/engine-spike.md`; `docs/runbooks/renderer-spike.md` updated; version-baseline rows for loro, parley,
  skia-safe, rquickjs and note C16.

Headline numbers (M5 Pro):
- Text: Renderer A and B are pixel-identical on all 30 fixtures; native CPU vs either on the GPU differs by at most 3 on
  at most 1 pixel per fixture; native vs B's CPU raster in WASM by at most 1. Parley and SkParagraph agree on line
  counts for all 30 but their pixels differ by up to 8.4%. B with Parley is 1.95 MB gzip (1.55 without text layout,
  2.65 with Skia's own paragraph module). ADR-001 should prefer B for text, with Parley layout and no `textlayout`.
- Loro at 10k nodes: open (load + first full read) 29 ms native / 56 ms WASM; 0.89 MB snapshot; about 3.8 MB heap; a
  200-node batch applies in 0.63 ms native / 0.97 ms WASM.
- QuickJS: the 500-op script runs in 0.22 ms in wasm32 with a new runtime, 0.11 ms on a kept one. It adds 444 KB gzip to
  `vellum-wasm` (`wasm32-unknown-unknown`) and 356 KB gzip to Renderer B's emscripten module.

Things that differ from the guide, failed, or could not be verified:
- **Native Skia is now built from source** (with FreeType), not from rust-skia's prebuilt binaries. Without it, the
  native path used CoreText, which could not load the colour emoji font at all. The build needs a patch to one line of
  bindgen output (macOS 27 SDK); see "Needs a person". `scripts/build.sh` does it.
- **SkParagraph first produced empty paragraphs** until the font provider was set as the collection's asset font
  manager instead of its default one. Recorded in the code.
- **Loro's output is not byte-stable across targets.** For the same document, native and WASM give equal content with
  map keys in a different order, and different snapshot bytes (95,792 vs 94,896 at 1k). ADR-004 makes the codec own
  key order; nothing outside the adapter may rely on Loro's.
- **Loro on `wasm32-unknown-unknown` needs wasm-bindgen** (loro-internal enables it there), so the WASM numbers are on
  emscripten, ADR-001's target, where it needs neither.
- **Undo is slow on large documents** (see "Needs a person").
- **Smaller findings for later prompts** (details in the ADRs): Loro decodes lazily, so a peer's first remote import
  after opening a document costs about 10x a later one (6.3 vs 0.5 ms at 10k; Prompt 14/63 should warm it up).
  rquickjs on wasm32 imports one host clock function, `__rquickjs_host_now_us`; Prompt 25 decides what it returns,
  since scripts must not read the wall clock. Noto Color Emoji is a bitmap font (CBDT), so large emoji look soft;
  Prompt 19 should try the COLRv1 version.
- **Only the M5 Pro.** Automerge and Yrs were not measured. Loro's WASM size without the scene generator was not
  separated.
- **Test order:** the Rust text, Loro and QuickJS tests were written first and failed (compile errors on the missing
  API) before the code. The kept-engine QuickJS test was added after the wasm run found the bug, and failed before the
  fix. The gpu text test was written after the measurement harness; its failure was checked by removing the fonts
  (it fails with the fetch instruction), not against missing code.
- `cargo deny` was not run on the spike workspaces (as in Prompt 4); the product workspace's dependencies did not change.

Verified: in `spikes/renderer`: `cargo test -p spike-scene` (8 passed, 5 ignored), `cargo test -p spike-scene --release
-- --ignored` (5 layout tests, lane fonts), `cargo clippy -D warnings` for every crate (native, emscripten with and
without `quickjs,textlayout`, wasm32-unknown-unknown), `cargo fmt --check`, `bash scripts/build.sh` from scratch,
`VELLUM_LANES=gpu,fonts node --test "web/test/*.gpu.test.mjs"` (2 passed: fixture #1 and the 30 text fixtures),
`node web/measure-text.mjs`; in `spikes/engine`: `cargo test` (14 passed), `cargo clippy --all-targets -D warnings`,
`cargo fmt --check`, `bash scripts/build-wasm.sh`, `VELLUM_LANES=wasm node --test "test/*.wasm.test.mjs"` (1 passed),
the three measurements; at the root: `biome check .`, `pnpm run copy-lint`, `pnpm run clean-room`, `pnpm run checksum`,
`pnpm run deps:check`, `pnpm run test:scripts` (123), `pnpm run design:check`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -D warnings`, `cargo fmt --check`. `pnpm type-check` was not re-run: no
TypeScript package changed.

## Prompt 6: Spike C, Chrome layout oracle (ADR-002)

Not committed. Prompt 5 was committed before this session (`78175eb`, `37be68d`). Decision: **D2 accepted**; see
`docs/architecture/decisions/ADR-002-layout-oracle.md` for the CSS mapping, the 15 semantic mismatches and how each is
resolved.

Built:
- `vellum-testkit::layout_oracle` (TDD, 6 tests): loads expected-box JSON (strict fields, unique ids, finite
  non-negative boxes), saves it deterministically, `load_dir` (skips `manifest.json`, file name must match the
  fixture), and `compare` at `TOLERANCE_PX = 0.5`. Adds `serde` and `serde_json` (workspace versions) to vellum-testkit.
- `scripts/gen-layout-oracle.sh` (now real) → `scripts/gen-layout-oracle.mjs`: `--check`, `--out`, `--only`, `--help`.
  `scripts/lib/layout-oracle.mjs` holds the fixture parser, the semantic resolution and the CSS mapping;
  `scripts/lib/chrome.mjs` drives headless Chrome over `--remote-debugging-pipe` (no TCP port, no npm dependency).
- `fixtures/layout-oracle/`: 51 fixtures (`TEST DATA`), written in Vellum semantics as `data-*` attributes, and
  `expected/` with Chrome's boxes and `manifest.json`. A full regeneration takes under 2 s.
- `spikes/layout/`: a separate Cargo workspace; taffy 0.14.0 lays out every fixture through a mapping that mirrors the
  CSS one, compared at 0.5 px, with an exact `known-mismatches.txt` (empty).
- CI: a `layout-oracle` job (chrome lane) that installs Chrome for Testing 155.0.8059.39, runs `--check`, then the Taffy
  spike test. `docs/runbooks/layout-oracle.md`; version-baseline taffy row and note C17; AGENTS.md command row.

Results: Taffy matches Chrome on all 51 fixtures (186 visible nodes); the largest difference is 0.0104 px (Chrome's
1/64 px layout units). Breaking the spike mapping on purpose makes 11 nodes in 4 fixtures fail, so the comparison is not
vacuous.

Things that differ from the guide, surprised me, or could not be verified:
- **Fixtures are Vellum semantics, not CSS.** The guide says "fixtures written as HTML (our sizing semantics mapped to
  CSS)". Each fixture is HTML, but its elements carry only Vellum properties; the generator applies the mapping and
  renders the CSS equivalent. That way the mapping is tested too, and Prompt 18 can feed the same props to Taffy.
- **No text in any fixture.** Hugging text needs `TextEngine` (Prompt 19) and a shared font. Also not covered:
  `layoutIncludesStroke` and real grid tracks (undefined in ARCHITECTURE, see "Needs a person"), wrap, min/max,
  percentages, RTL. The guide wants at least 150 fixtures by Prompt 18.
- **Turborepo was dropping `VELLUM_LANES`.** In strict env mode, `VELLUM_LANES=chrome pnpm test` reached the script
  tests without the variable, so lane tests always skipped through Turbo. `turbo.json` now passes `VELLUM_LANES` and
  `VELLUM_CHROME` through to `//#test:scripts` (`passThroughEnv`; `globalPassThroughEnv` is deprecated in 2.11.7).
  Prompt 4's and 5's lane tests run outside Turbo, so they were not affected.
- **taffy 0.14.0's API differs from the `refs/taffy` pin**: alignment constants are `AlignItems::FLEX_START` on
  crates.io, `AlignItems::FlexStart` at the pin and in its `docs/style-properties.md` (note C17).
- **Biome ignores `fixtures/layout-oracle/expected`**: the generator writes one value per line, and Biome would collapse
  short arrays. The files are generated, like `spikes/*/results`.
- **`cargo fmt` in a spike workspace reformats `crates/vellum-testkit`** (a path dependency) differently from the root,
  as in Prompt 4. Use `cargo fmt -p spike-layout` there; root `cargo fmt --all` restored the testkit.
- **Only macOS Chrome has written boxes so far.** The CI job (Linux) is unproven; see "Needs a person".
- **Test order:** the testkit tests failed to compile before the module existed; the script tests failed (missing
  module, then missing fixtures and the not-implemented script) before the code; the Taffy test failed to compile before
  `compute` existed and then exposed the `manifest.json` gap in `load_dir`, which got its own failing test first.

Verified: `cargo test -p vellum-testkit --test layout_oracle` (6), `VELLUM_LANES=chrome node --test
"scripts/__tests__/*.test.mjs"` (140, chrome lane included), `VELLUM_LANES=chrome pnpm exec turbo run //#test:scripts`
(140, chrome lane runs), `bash scripts/gen-layout-oracle.sh --check` (51 match), in `spikes/layout`: `cargo test` (2),
`cargo clippy --all-targets -D warnings`, `cargo fmt --check -p spike-layout`; at the root: `pnpm exec biome check .`,
`pnpm run copy-lint`, `pnpm run clean-room`, `pnpm run deps:check`, `pnpm run checksum`, `pnpm run design:check`,
`pnpm run type-check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -D warnings`,
`cargo fmt --all --check`, `cargo deny check`.
