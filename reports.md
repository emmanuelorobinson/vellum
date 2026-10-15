# Reports

What a person should know after each prompt: what was built, what was decided on the way, what was skipped or
could not be verified, and what only a human can do. AGENTS.md tells every session to read this file first and
to update it at the end of each prompt. Newest prompt last. Keep entries short and factual; say what failed.

## Needs a person

Open items, newest first. Remove an item when it is done and say so in the prompt's entry.

- **Review ADR-015 and synchronize ARCHITECTURE** (Prompt 21). Display commands now have a versioned
  binary ABI and typed shape/appearance resources. Transform sets an absolute device matrix after f64
  viewport rebasing. The ADR records layout diagnostic semantics, conservative culling and deferred
  renderer lowering. Only a person edits ARCHITECTURE.md.

- **Review ADR-014 and synchronize ARCHITECTURE** (Prompt 20). `SpatialIndex` now takes occurrence
  paths, matching layout and resolved components. The ADR records bounds-based selection, rounded clipping,
  snapping ties and the bounded free-space search. Only a person edits ARCHITECTURE.md.

- **Review ADR-013 and synchronize ARCHITECTURE** (Prompt 19). Text ports now carry renderer-ready glyph
  offsets, variation/synthesis, decorations/links, shared font bytes and explicit missing-font/glyph ranges.
  Auto-width hugs both dimensions; auto-height hugs height; fixed text keeps its sizes and reports overflow.
  Font sources are explicit host-supplied bytes, prioritized by tier. Only a person edits ARCHITECTURE.md.
  The new `text-fixtures` fonts CI job was verified locally, not on GitHub.

- **Review the existing Loro dependency advisories** (Prompt 17). `pnpm cargo:deny` now fails on
  `bitmaps` ([RUSTSEC-2026-0247](https://rustsec.org/advisories/RUSTSEC-2026-0247)),
  `im` ([RUSTSEC-2026-0248](https://rustsec.org/advisories/RUSTSEC-2026-0248)) and
  `sized-chunks` ([RUSTSEC-2026-0251](https://rustsec.org/advisories/RUSTSEC-2026-0251)), all unmaintained
  through the existing Loro dependency chain. Its report says no safe upgrade is available. Prompt 17 changes
  neither dependencies nor advisory policy; decide the upstream/pin response separately.
- **Review ADR-011 and synchronize ARCHITECTURE** (Prompt 17). The person selected explicit prop bindings;
  the ADR also records the new optional instance `library` alias, precedence, text-content binding rule,
  library scope, cache and paste behavior. Only a person edits ARCHITECTURE.md.

- **Review the Prompt 16 choices that the guide or ARCHITECTURE leave open** (ADR-010 "Prompt 16"). Only a person edits
  ARCHITECTURE.md. (1) The last entry that applies wins by position, not specificity. (2) A script input
  (`inputs {name → value | $var}`) takes a variable of any type; a colour arrives as the text `#RRGGBB`. I chose it
  because `fixtures/documents/content.vellum` already binds a colour variable to a script input, and ARCHITECTURE puts no
  limit on it; the alternative is to refuse colour there and change that fixture. (3) Variable types are checked when a
  binding is made, once per transaction, as `OpErrorKind::BindingType`; a variable that changes type while something
  uses it is refused. (4) A bad theme choice on a frame is ignored and reported, not fatal. (5) The resolver visits every
  node on every resolve (cheap, no allocation) and recomputes only the dirty ones; skipping whole subtrees needs
  `ChangeSet` to name the parents of inserted, removed and moved nodes, which I did not change.
- **Review the conflict node and the new document-level ops** (Prompt 15). ADR-010's "Prompt 15" section. The
  format gains a 19th node kind, `conflict` (`ours`, `theirs`, `properties`, `children`), which ARCHITECTURE §4.1's
  table does not list; only a person edits ARCHITECTURE.md. Ops gain `InsertPage`, `DeletePage`, `UpdatePage`,
  `MovePage`, `SetShell`, `SetComponent`, `UpdateComponent`; `ChangeSet` gains `pages`, `components`, `shell`; `Kept`
  gains `Page`, `Shell`. Merge rules I chose: a change beats a removal; where no node can hold both versions (both
  moved one node, both reordered the same children, a variable, style, page or the name changed both ways) ours is
  kept and the conflict is only in the merge's report, so the editor must show those from the report.
- **References are now checked once per transaction** (Prompt 15), not after every op. It changes what is accepted:
  a transaction whose ops pass through a broken state but end valid (an instance inserted before its component is
  added) is now accepted. A refused one still names the op that first broke a reference. Say if you want the old
  rule back; the diff would then need another ordering scheme.
- **Op planning cost** (Prompt 14, mostly fixed in Prompt 15). Checking references once per transaction took planning
  a 200-op batch at 10,000 nodes from about 292 ms to 13 ms. Undo of that batch is 15.0 ms (budget 16, was 14.5).
  What is left: an insert still lists every id in the document, and undo copies the document; an id index would cut
  both. `crates/adapters/doc-loro/tests/undo_cost.rs` (ignored by default; run in release).
- **Guide Prompt 15's FINAL command does not run as written.** `cargo test diff:: merge::` fails with "unexpected
  argument 'merge::'" (cargo takes one filter). Use `cargo test -p vellum-core -- diff:: merge::`.
- **Decide whether the fuzz targets run anywhere but a laptop** (Prompts 9 and 15). `crates/vellum-core/fuzz/`
  (`merge`) and `crates/adapters/codec-json/fuzz/` (`decode`) need nightly and cargo-fuzz; CI runs neither. The same
  properties run on a fixed corpus in `cargo test` (`fuzz_smoke`, `robustness.rs`).
- **Review the `DocStore` additions and the undo rules** (Prompt 14). Only a person edits ARCHITECTURE.md. New on
  the port: `undo_turn(turn, by) -> UndoTurn` (`Nothing`, `Undone(ChangeSet)`, `Revert(Revert)`), `Revert` and
  `Kept`, `SyncError::DifferentDocument`, and `vellum_core::ops::revert` to apply a `Revert` as a new transaction.
  Rules I chose (ADR-004 "Prompt 14"): ⌘Z never returns a Revert; it takes the actor's latest change back as far as
  it can and keeps what later work by others depends on, so pressing it again goes further back. "Undo this turn"
  is undone outright only when nothing is kept; otherwise it returns a Revert and changes nothing. Setting a
  property to the value it already has is not a change, so it is not protected from another actor's undo.
  Changes from another store are attributed to the newest change's origin and cannot be reached by "Undo this turn"
  here. `ops::revert` does not go through the Guard: a person asks for it.
- **Guide Prompt 14's FINAL command does not run as written.** `cargo test -p doc-loro` fails ("did not match any
  packages"); the package is `vellum-doc-loro`, as every adapter is `vellum-<dir>`. Use
  `cargo test -p vellum-doc-loro`.
- **Review the Guard's reason strings and limits** (Prompt 13). Only "Frame leased by {agent}" is in Appendix B. The
  others are mine, built on DESIGN.md's verbs: "Agents paused", "{agent} is paused", "Stopped by you", "Stopped by
  you on this frame", "{count} changes at once · the limit is {limit}", "{count} new layers at once · the limit is
  {limit}". They live in `@vellum/config` as `guardReasons` (outside the Appendix B `copy` object) and in
  `vellum_core::guard::reasons`; a Rust test keeps them equal. The default limits (1,000 ops, 1,000 inserted nodes
  per agent transaction) have no source; I matched the script output budget. Change either in both places.
- **Guide Prompt 13's FINAL command does not run as written.** `cargo test guard:: lease::` fails with "unexpected
  argument 'lease::'" (cargo takes one filter). The equivalent is `cargo test -p vellum-core -- guard:: lease::`.
  The guide is read-only to me.

- **Review Prompt 12's operation representation and path port addition**. The choices/limits below record the
  off-page component representation, sparse instance inheritance, geometry inputs and unsupported outline styles.
  `PathOps::outline_stroke` is needed by the new op and is absent from ARCHITECTURE's port signature. Only a person
  edits ARCHITECTURE.md; no format/schema change was made.
- **Review the port signatures that differ from ARCHITECTURE §3.1** (Prompt 10). Only a person edits ARCHITECTURE.md.
  `Codec::encode` returns a `Result` (done: Prompt 9's open item). `FontSource::resolve` takes a weight as well as a
  style (a text run has both). `PathOps::bounds` returns `Option<Rect>` (a path can draw nothing) and takes a resolved
  `StrokeStyle`, not the file's `Stroke`, whose widths can still be variables. `DocStore` gains `adapter_id()` and
  `cursor()` (`export_updates` needs a cursor to be given); `undo`/`redo` take `&Actor` (Prompt 11); `apply` takes a `Commit`, not a `Transaction` (Prompt 11). `SpatialIndex::query_point`
  takes `x, y`. `AssetStore::put` takes `&[u8]`, not owned bytes, and there is no `bytes` crate in core. `ScriptApi` and
  `ScriptValue` are minimal (Prompt 26 fills in the functions).
- **Review the Prompt 11 choices that the guide or ARCHITECTURE leave open** (Prompt 11). Only a person edits
  ARCHITECTURE.md and the guide, so these live in the code until you say otherwise:
  (1) Invariant 1 says `DocStore::apply` is "`pub(crate)` to `vellum-core::ops`". A trait method cannot be, and
  adapters live in other crates, so `apply` takes a `Commit` that only the op layer can build (private fields, a
  `pub(crate)` constructor, no `Clone` or `Default`). The trybuild cases in `crates/vellum-core/tests/write-path/` show
  that a transaction, a hand-built commit and a copied commit do not compile.
  (2) `Actor`'s inner words are mine: `FileReason` is `external-change`, `merge` or `recovery`; `Subsystem` is
  `importer`, `migration` or `sync`. A person is known by a `DeviceId`; `DeviceId`, `AgentId`, `SessionId` and `TurnId`
  are ULIDs (upper-case Crockford, as guide §9 says for ids outside the file).
  (3) Parent kinds: frames, groups and canvas pages hold anything; a boolean holds rect, ellipse, polygon, star,
  line, vector, boolean, text and group; a slides page holds frames, notes, context and prompts at its top level.
  Nothing in ARCHITECTURE says this; tell me if a kind is wrong.
  (4) Page boundaries: a node may move between pages, but not between a page and a component, nor between components.
  A component's root cannot be moved or deleted by these ops (Prompt 12's component ops will).
  (5) Wrap, group and ungroup rebase absolute positions by the wrapper's position and refuse a rotated or flipped
  wrapper or group (positions cannot be kept through a rotation without baking it into each child). Ungroup drops the
  group's opacity, blend mode and effects. A wrapper at `auto` leaves its children's coordinates as they were.
  (6) In an `Update`, `null` puts a property back to its default (and fails for a required one such as `width`).
  (7) `Move` and `Reorder` count the index where the node ends up, after it left its old place.
- **Decide whether `ops::apply` may snapshot the whole document per transaction** (Prompt 11). It validates against
  `DocStore::snapshot()`, which clones the document. That is fine for the fake and for tests; for Loro (Prompt 14) a
  10,000-node document would clone on every keystroke-sized edit. Prompt 14 kept the port as it is: the Loro store
  keeps the document in our types beside Loro, so `snapshot()` is a clone, not a read from Loro. The clone is small
  next to planning (see the planning-cost item above); a borrowed read view on the port is still open.
- **Decide the open numeric units and the decode limits the codec assumes** (Prompt 9). ARCHITECTURE and the guide give
  units for positions, sizes, ratios, opacities, gradient stops and angles only. I read the rest as: transform and crop
  entries, mesh coordinates, `innerRadius` (a ratio of the radius, per its doc comment) and `zoom` are ratios (1/1000);
  blur, spread, font size, line height, letter spacing, dash lengths and `scroll` are lengths (1/100 px). Number
  variable values and number prop defaults are written exactly, because they can be a length, a ratio or a count.
  `quantization_table.rs` fails when a number is added to the types without a decision. If a crop of a large image
  needs finer than 1/1000, say so now.
- **A newer file is refused, not shown read-only** (Prompt 9). Invariant 5 and DESIGN S04 say a newer format "opens
  read-only" with a banner. `decode` returns `CodecError::NewerFormat { found, supported }` for it, which is enough for
  the banner, but there is no lenient reader that would show the content: the types refuse fields they do not know. A
  host can only show the banner until such a reader exists. Decide whether v1 needs one.
- **`pnpm test:scripts` fails on a clean checkout** (found in Prompt 9, not caused by it). The test "REFERENCE-INDEX.md is
  current and every path exists in refs/" reports `missing: R3 taffy: main.rs`: guide §3.2 row R3 names `main.rs` bare
  inside `scripts/gentest/`, and the real file is `scripts/gentest/src/main.rs`. Reproduced on `HEAD` in a separate
  worktree with this machine's `refs/`. The fix is in the guide row or in `scripts/reference-index.mjs`; I did not touch
  either.
- **Review ADR-010 and update ARCHITECTURE §4.1 and §4.4** (Prompt 8). The ADR makes three decisions that differ from
  ARCHITECTURE: a `$var` (and an alias) names a variable by its id, not its name path, so the example
  `{"$var": "color/primary"}` now describes a name, not the file; variants are override sets on the component's one tree,
  not separate trees; and an override is one of `props`, `replace` or `children`, with props checked against the node
  property names. Only a person edits ARCHITECTURE.md. The ADR lists the costs and the things it leaves undecided.
- **Confirm the two CI steps added in Prompt 8 pass on GitHub**: `pnpm types:check` and `pnpm format-spec:check`. Both
  pass on this Mac; the generated schema and bindings have not been checked on the Ubuntu runner.
- **Review the Prompt 7 choices where ARCHITECTURE §4.1 was silent or contradicted itself** (Prompt 7). Only a person
  edits ARCHITECTURE.md, so these live in the code until you say otherwise: (1) an `icon` node's icon name is `icon`,
  not `name`, because `name` is already the layer name on every node; (2) a frame gets `cornerRadius` (the table gives it
  to `rect` and `polygon` only); (3) a variable reference is `{"$var": "<key>"}` with an opaque non-empty key, and it is
  not settled whether the key is a variable id (no `/` allowed) or a name path like `color/primary` (the examples use
  that); Prompt 8 must decide; (4) text runs have no colour of their own, so a text node's `fills` colour the whole
  node; (5) a prototype transition is `instant`, `dissolve` or four slides plus `durationMs`, which is my guess; (6) a
  mesh paint is `columns` x `rows` points row by row, each with a colour and optional four handles, also my reading;
  (7) padding is `n`, `[vertical, horizontal]` or `[top, right, bottom, left]`, corner radius `n` or four values, and a
  stroke width `n` or four values; (8) node structs are named `FrameNode`, `TextNode` and so on, so the TypeScript
  names do not shadow `Boolean`, `Text` or `Image`. Item (3) is decided by Prompt 8: a reference names a variable by
  its id (ADR-010), so the name path in the examples is the variable's name.
- **Decide whether `proptest` stays** (Prompt 7). It is now a dev-dependency of `vellum-types` and on the allowlist
  (version-baseline C18). The guide asks for property tests in Prompts 7, 9, 10, 14 and later; the alternative is a
  hand-rolled generator, which loses shrinking.
- **Review ADR-002** (Prompt 6). It accepts D2 and fixes Vellum's sizing rules where CSS's defaults disagree with
  hug, fill and fixed (15 rows, each with a fixture). Decisions worth a look: fill shares space equally and ignores its
  content (`flex-basis: 0`, `min-width: 0`); fixed and hug never shrink; fill on the main axis of a hugging parent uses
  its fallback, else hugs; hug on a frame without auto-layout, and fill with nothing to fill, need a fallback; two padding
  values are vertical then horizontal, as in CSS.
- **Review ADR-012 and synchronize ARCHITECTURE** (Prompt 18). Grid keeps equal column/optional row counts;
  `layoutIncludesStroke` reserves the frame's own inward stroke beside padding (inside full, center half,
  outside zero), preserving absolute outer-edge coordinates. Layout ids/results/dirty entries use occurrence
  paths, and results expose typed problems. Preparation scans all inputs; Taffy reuses clean computations.
  Only a person edits ARCHITECTURE.md.
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

## Prompt 7: vellum-types I, nodes, paints, strokes, effects, sizing

Committed as `2452a26`. Prompt 6 was committed as `374767d`. Everything is in `crates/vellum-types` (new modules `ids`, `value`,
`color`, `geometry`, `enums`, `paint`, `stroke`, `effect`, `text`, `node`, `defaults`) plus a fix to
`scripts/types-gen.mjs` and the regenerated `packages/types-ts`.

Built:
- Typed ids `NodeId`, `PageId`, `VariableId`, `ComponentId`, `StyleId`, `AssetId` (1 to 128 characters of
  `A-Za-z0-9_-`, so never a `/`) and `NodeIdPath` (prints and parses `a/b/c`, usable as a JSON object key).
- `Value<T>` = literal or `{"$var": ...}`; `Color` (`#RRGGBB` / `#RRGGBBAA`, upper case out, either case in);
  `Size` = fixed (number or var), `"hug"`, `"fill"`, `{"hug": n}`, `{"fill": n}` (one spelling each).
- The 18-kind `Node` union with every common field, all kinds from the brief, `Paint` (solid, gradient, mesh, image,
  shader), `Stroke`, `Effect` (drop and inner shadow, layer and background blur, shader), 18 `BlendMode`s.
  A field at its default is left out when printing, so a value has one spelling; `deny_unknown_fields` everywhere except
  `metadata`.
- `Instance` has real `componentId`, `variant` and `slotContent`; `props` and `overrides` are placeholder JSON until
  Prompt 8. A `script` node has no `children` field (derived output is never saved).
- 80 TypeScript types generated into `packages/types-ts`, and a type-level test in its `index.test.ts`.

Differs from the guide or ARCHITECTURE: see the first "Needs a person" item (icon field, frame corner radius, `$var`
key, no run colour, transitions, mesh). Also: the common fields are repeated on each node struct by a macro, because
serde cannot combine `flatten` with `deny_unknown_fields`; `proptest` joined the allowlist and `serde_json` got
`float_roundtrip` (version-baseline C18); the sizing fallback is a plain number, not a variable.

Found along the way:
- **serde reads `{"word": null}` as the unit variant `"word"`**, a second spelling for every enum. The string enums and
  `SizeKeyword` now have hand-written (de)serialisation, with a test (`a_word_has_one_spelling_not_two`). Ids, `Color`
  and `Size` are hand-written too, because ts-rs warns on every build about `try_from`, `into` and `transparent`.
- **`scripts/types-gen.mjs` ignored subdirectories.** ts-rs writes `serde_json/JsonValue.ts` (borrowed type), which the
  node types import, so `pnpm type-check` failed. The script now walks subdirectories (drift, missing and stale
  checks too), with three new tests.
- **The property tests did not catch a wrong default at first.** Random floats never equal `1.0`, so a mutation of the
  opacity default survived. The generators now favour 0, 1 and 360; mutating the opacity, ellipse sweep and weight
  defaults is now caught (the identity-transform mutation is an equivalent mutant).
- Debug builds overflow a 2 MiB test thread while building the node strategies, so those properties run on a 64 MiB
  thread. Deserialising deep documents does not overflow: serde_json returns an error past its recursion limit, and
  `a_document_nested_too_deeply_is_an_error_not_a_crash` pins that on a 2 MiB thread.

Not verified or not done: the JSON Schema is only checked to build (Prompt 8 publishes it); the lenient numeric rules
(a `weight` outside 100 to 700, `sides` under 3, a grid with no `columns`, a negative size) are not validated, because
Prompt 10 owns validation; `Value<T>` for string-typed properties is used only for an icon's `icon`.

Test order: the four example-based test files were written first and failed to compile (types missing); they passed
after the implementation. The property tests (`roundtrip.rs`) were written after the types, because their strategies
build the structs, so they were never seen failing on missing code; the mutation checks above stand in for that.

Verified: `cargo test -p vellum-types` (79 unit/export tests, 10 ids, 13 paints, 20 nodes, 7 properties, 4 smoke, 10
values), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `pnpm test` (33 tasks),
`pnpm type-check`, `pnpm lint`, `pnpm format:check`, `pnpm types:gen` then `node --test scripts/__tests__/types-gen.test.mjs`,
`pnpm deps:check`, `pnpm checksum`, `pnpm clean-room`, `pnpm copy-lint`.

## Prompt 8: vellum-types II, the document, variables, components, schema and TypeScript

Committed (`1f6b77f`). ADR-010 records the model decisions.

Built:
- The document (`document.rs`): meta (`formatVersion`, `name`, `createdWith`, `kind` document or library), `imports`,
  `themes`, `variables`, `styles`, `fonts`, `components`, and `pages` in order (canvas or slides). `Document::new(name)`.
- Variables (`variable.rs`): one kind per type (`color`, `number`, `string`, `boolean`), each reading its literals as
  that type. An entry is a literal (`value`) or an alias (`$alias`), with an optional `when`. Choosing the entry that
  applies is resolution (Prompt 16).
- Styles and fonts (`style.rs`): text styles (a text run's settings without its characters), effect styles, and font
  faces keyed by the asset that holds the file.
- Components (`component.rs`): a name, typed props (`boolean`, `text`, `number`, each with a default of its type), the
  root tree, and variants. Overrides are `props`, `replace` or `children`. `NodePatch` checks its keys against the node
  property names and refuses `id` and `children`.
- Instances and scripts (`node.rs`): an instance takes a variant choice, prop values (`Scalar`), overrides and slot
  content. `ScriptInput` is now `Scalar`, shared with instance props. `NODE_PROPERTIES` lists the property names, and a
  test checks them against the schema.
- The schema and the TypeScript: `examples/format-schema.rs` prints the schema. `scripts/gen-format.mjs` writes
  `docs/format/vellum.schema.json` and a reference page, `docs/format/README.md`, rendered by
  `scripts/lib/format-spec.mjs`. `packages/types-ts` is regenerated (101 types). CI runs `pnpm types:check` and
  `pnpm format-spec:check`. AGENTS.md has the command rows.
- Tests: four new Rust test files (`document`, `variables`, `components`, `styles`), a property test for variables, and
  a Node test for the renderer and the drift check. Samples in five existing tests now use ids for `$var`.

Decided on the way (ADR-010 has the reasons and the costs):
- **A reference names a variable by its id**, `{"$var": "<id>"}`, not by its name path. The name lives in the variable's
  entry. A rename or a merge of a rename does not leave a dangling reference (merge driver, Loro, multiplayer). Every
  other reference in the file is an id too. This is the open question Prompt 7 left, and it departs from ARCHITECTURE's
  example.
- **Variants are override sets on the component's one tree**, applied when an instance's variant equals the variant's
  values. The first draft was a whole tree per variant; that duplicated the tree.
- **Overrides are typed by form, but the props are a checked JSON object.** Typing every property of every node kind
  would be a parallel struct for each of 18 kinds. Keys are checked now; values are checked at apply time (Prompt 17).
- **Pages are a list; every other collection is a map sorted by id** (the same bytes for the same document).
- Fixed for the build: ts-rs warned about `#[serde(transparent)]` and enum-level `deny_unknown_fields`, so `NodePatch`
  is written by hand and `PropDef` became three structs. `Override::Replace` is boxed (clippy `large_enum_variant`).
  Doc comments that ts-rs copies into TypeScript no longer hold rustdoc links (`VarRef`, and `SizeRepr` from Prompt 7).
- `EmptyVarRef` is gone; a bad reference is an `IdError`, like any bad id.

Not done, or differs from the guide or ARCHITECTURE:
- **Props do not reach nodes yet.** The declaration and the values are typed; the binding is not decided (Needs a person).
- **Overrides are not typed per node kind** (see above). The schema says `props` is an object of node property names.
- **Validation is not here.** Aliases resolving to the right type, alias cycles, unique names, variant axes, and default
  types are for Prompts 10, 11 and 16. The types only say what each entry is.
- **Schema names for generic instances** come out as `Value2`, `VariableValue3` and so on, because schemars numbers
  them. The reference page shows those names as they are.
- Appendix E does not list ADR-010; it is added here because it records a change to ARCHITECTURE.

Found along the way:
- **I ran `pnpm checksum:update` by mistake** while listing gates. It is a human step, and it rewrote
  `scripts/spec-checksums.json` with the same three hashes. `git diff` is empty for that file, so nothing changed.

Verified (each exited 0 unless noted):
- `cargo test --workspace`: 263 passed, 0 failed, with every test file of this prompt in place.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- `cargo deny check licenses bans sources` and `cargo deny check advisories` (the wrapper warnings from Prompt 2 remain).
- `pnpm test:scripts`: 150 tests, 149 passed, 0 failed, 1 skipped. The skipped one is the design-check test that runs
  only when snapshot PNGs are absent, and it was skipped before this prompt too.
- `pnpm test` (33 turbo tasks), `pnpm type-check` (32), `pnpm lint`, `pnpm format:check`, `pnpm exec biome check .`.
- `pnpm deps:check`, `pnpm clean-room`, `pnpm copy-lint`, `pnpm checksum` (3 files intact), `pnpm types:check`
  (101 types), `pnpm format-spec:check` (2 files), `pnpm design:check` (11 recorded gaps).
- In `packages/types-ts`: `tsc --noEmit` and `vitest run` (4 passed).

Test order: the new Rust test files and the Node renderer test were written first and failed to compile or to load
(the types, the functions and the `format-spec` module did not exist). The variable property tests passed the first
time, so they have not been seen failing.

Not verified: the two new CI steps on GitHub (Ubuntu). The generated schema and bindings come from macOS.

## Prompt 9: JSON codec, deterministic bytes, quantization, migrations

Committed (`2061cc8`).

Built (`crates/adapters/codec-json`, package `vellum-codec-json`):
- **`JsonCodec`, `encode`, `decode`.** The bytes are 2-space indent, LF, one key or item per line, a trailing newline.
  A struct's keys come out in the order its type declares them; every map comes out sorted by key bytes (the serializer
  sorts them itself, so a `HashMap` or a `serde_json` built with `preserve_order` elsewhere cannot change the file). To
  keep struct order the codec builds its own ordered tree with a custom serde `Serializer` (`tree.rs`), because
  `serde_json::Value` sorts every key. Whole numbers print without a fraction; other numbers print as the shortest text
  that reads back (`float_roundtrip` is on); `-0` prints as `0`. Strings use only the escapes JSON requires.
- **Quantization** (`quantize.rs`), in the codec only: lengths 1/100 px, ratios 1/1000, angles 1/100 degree. The unit
  follows the field name, with two context rules: mesh `x`/`y` (under `points` or `handles`) are ratios, and anything
  under `metadata`, `uniforms`, `inputs` or an instance's `props` is the user's and is written as is (a props override
  inside `overrides` is a node patch and is rounded). Halves round away from zero on the binary value, so `1.005`
  becomes `1.00`, not `1.01`; this is stable across platforms and I did not add decimal-aware rounding. Values too large
  to have a fraction (past 2^50 steps) are left alone. `quantize` is idempotent (a property test over normal, subnormal and huge values, and unit tests at the extremes).
- **A value that rounds to its type's default** (for example a `startAngle` of 0.004) would be left in by the type, which
  judged the unrounded value, and then left out on the next save. When rounding changes anything, `encode` reads back
  what it would write and writes that, so saving what was loaded is always the same bytes. The property test catches it
  if this pass is switched off (checked by hand: two properties fail).
- **Reading** accepts any key order, whitespace, CRLF and a UTF-8 byte order mark, and refuses unknown fields. The
  version is read first (`serde` ignores the rest), so a newer file is `NewerFormat` whatever else it holds; a version
  that is not a whole number from 1 is `Invalid` at `meta.formatVersion`. **Duplicate keys are refused** (`"va1"` twice
  in `variables`): serde would keep the last without a word, which is what a bad textual merge leaves behind.
- **S05 errors** carry a path (`pages[0].children[2].width`), a line and a column counted in characters. serde reports a
  fault inside a node at the node, because a tagged enum is read through a buffer, so `decode.rs` reads the node again
  as its own kind to find the field, and finds an unknown field by looking for the key whose siblings are all expected
  ones (`locate.rs` indexes the text only when there is an error to explain). A file that was migrated first has a path
  and no position, because its text is no longer what the user wrote. Only nodes are re-read; a wrong value inside a paint
  or an effect is reported at the paint or effect.
- **Migrations**: `Migration` (source version plus `migrate(&mut serde_json::Value)`), `Migrations` registry (steps must
  be 1→2, 2→3, ... in order, none missing, none past the current version; the registry sets `meta.formatVersion` after each
  step), `MigrationError`, `CodecError::Migration { from, .. }`. `FORMAT_VERSION` is 1, so the standard registry is empty.
  The machinery is tested with a made-up chain in `tests/example-migrations/` (versions 1 to 3 never existed), and a test
  walks the real registry and demands `fixtures/migrations/v{N}-to-v{N+1}/{before,after}.vellum` for every step. There are
  none yet, so that test passes with nothing to check.
- **Fixtures**: `fixtures/documents/` (five canonical files marked `TEST DATA`, 970 lines) use every node kind, paint, effect,
  variable type, style type, override form, a library, and a slides page; a test checks that. The `canonicalize` example
  rewrites or checks files.
- **Fuzz target**: `crates/adapters/codec-json/fuzz/` (its own workspace, `cargo +nightly fuzz run decode`). It builds
  (`cargo check`); it has not been run, because that needs nightly and cargo-fuzz. The same property runs on every
  `cargo test` over 400 mutations of each fixture plus a hostile corpus (`robustness.rs`).
- The Prompt 7 property-test generators moved to `crates/vellum-types/tests/common/strategies.rs` so the codec tests can
  include them by path instead of copying 700 lines. `roundtrip.rs` is otherwise unchanged and passes.
- New dependency: `serde_path_to_error` 0.1.20, in the adapter only (workspace dependency, MIT/Apache-2.0).

Measured (release, this Mac): a document of 10,000 rects, 5.4 MB, encodes in 48 ms (rounding changes values), in 11 ms
when already canonical, and decodes in 32 ms. Ten thousand trivial nodes already pass ADR-004's 4 MB suggestion
threshold because every array item gets its own line; the threshold is "10,000 nodes or 4 MB", so it fires on size.

Differs from the guide, or limits:
- `encode` is fallible; the `Codec` port takes the `Result` (Prompt 10), and `CodecError` and `Position` moved to `vellum_core::ports`.
- **Nesting limit.** serde_json stops at 128 levels of JSON, and a nested node costs two (the object and its `children`
  array), so about 60 frames inside frames. A deeper file is refused with a recursion-limit message. `encode` has no such
  limit unless rounding changed something. Real designs are far shallower; say if you want the limit raised (it needs
  `serde_stacker` or an iterative reader).
- Free-form JSON (`metadata`, patches) prints an integral float as `1`, not `1.0`, so `1.0` reads back as the integer `1`.
  The bytes are stable after the first save; the in-memory value differs once.
- Only nodes are re-read to find a field, as described above.

Verified (each exited 0 unless noted):
- `cargo test -p vellum-codec-json`: 85 tests (canonical 24, errors 31, fixtures 5, migrations 11, properties 8, quantization
  table 3, robustness 2, smoke 1). `cargo test --workspace`: 347 passed, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo build -p vellum-codec-json
  --target wasm32-unknown-unknown`, `cargo check` of the fuzz crate.
- `cargo deny check licenses bans sources` and `cargo deny check advisories`.
- `pnpm deps:check`, `pnpm clean-room`, `pnpm copy-lint`, `pnpm checksum`, `pnpm types:check`, `pnpm format-spec:check`,
  `pnpm design:check`, `pnpm format:check`, `pnpm lint`, `pnpm type-check`, `pnpm exec biome check .`.
- The document properties re-run at 1500 cases each in release: all pass.
- Failed: `pnpm test:scripts`, 148 passed, 1 failed, 1 skipped. The failure is the pre-existing REFERENCE-INDEX test
  described in Needs a person; it fails the same way on a clean `HEAD`.

Test order: every test file was written before any codec code existed, so they did not compile. After the code compiled,
six tests failed for reasons in the tests themselves (a first-match search that found the page's `type`, `.0` found in the
string `Vellum 0.0.0`, inputs like `1.005` that are below the tie in binary, an `auto` position that is never written, a
`null` version read as missing, and an unknown-field path that serde_path_to_error ends at the field); each was fixed by
correcting the test or the code, not by loosening what it checks. The property tests passed the first time; to see them
fail I switched off the settle pass by hand and two failed.

## Prompt 10: Ports, fakes, contract suites

Committed as `eb529c5`.

Built:
- **The twelve ports** in `crates/vellum-core/src/ports/`: `DocStore`, `LayoutEngine`, `TextEngine`, `FontSource`,
  `SpatialIndex`, `PathOps`, `Codec`, `AssetStore`, `Clock`, `IdGen`, `ScriptRuntime`, `ShaderCompiler`, with the data
  types their signatures need (`Rect`, `Extent`, `RichText`, `LayoutTree`, `Path`, `ChangeSet`, `AssetHash`, ...).
  `vellum-core` still depends on `vellum-types` alone, and `pnpm deps:check` passes; scripts/__tests__/deps.test.mjs
  already proves that loro or tokio in core fails the build. Coordinates are `f64`, times `i64` ms. `IdGen` has `node_id`
  and provided `page_id`, `component_id`, `variable_id` and `style_id` (same alphabet).
- **`CodecError` and `Position` moved** from `vellum-codec-json` into `vellum_core::ports` (the codec re-exports them);
  `JsonCodec` implements `Codec`. `Node::children_mut` was added to `vellum-types` (no TypeScript change).
- **`crates/contracts`**: a generic suite per port in `suites/` (a function that panics naming the rule) and a fake per
  port in `fakes/`: `InMemoryDocStore` (per-actor undo and redo as inverse steps, sync by replaying a step log),
  `FixedMetricText` (0.6 em per glyph, 1.2 em lines, greedy wrap at spaces), `StackLayout` (row/column, fixed/hug/fill,
  gap, padding, justify, align, ADR-002 rows 3, 5 and 7), `NaiveSpatialIndex`, `IdentityPathOps` (returns `a`; bounds from
  control points), `FakeJsonCodec`, `InMemoryAssets`, `ManualClock`, `SequentialIds`, `StaticFonts`, `NoopScriptRuntime`,
  `PassthroughShaderCompiler`. `samples` holds the shared `TEST DATA` document.
- **Every fake passes its suite** (`tests/fakes_pass_their_suites.rs`), the real `JsonCodec` passes the `Codec` suite on
  the five fixture documents (`codec-json/tests/contract.rs`), and **every suite rejects a deliberately broken
  implementation** (`tests/suites_catch_broken_implementations.rs`, 17 cases, each `should_panic` on the rule's message).
  That second file is what shows the suites check something.
- `vellum-core/tests/ports.rs` tests the small types (closed rectangle edges, hash alphabet, origins, dirty sets).

Differs from the guide, or limits:
- **The suites for `PathOps`, `ScriptRuntime` and `ShaderCompiler` are short on purpose.** The guide's fakes are identity,
  no-op and passthrough, which cannot satisfy a suite that asks for real booleans, a syntax error or an escape attempt.
  They ask what every sound implementation does (bounds hold the shape and its stroke, a boolean stays inside its inputs,
  the same script gives the same result, empty source does not compile). Prompt 12 adds boolean semantics and Prompt 25
  the sandbox corpus as extra suites the real adapters run.
- **The `DocStore` suite does not test concurrent edits converging**, or undo across dependent changes (Invariant 3's
  property test). The fake replays steps in arrival order, so two stores editing at once would not converge. Prompt 14
  adds both for Loro. The suite does test one-way and back-and-forth sync, importing twice, a foreign adapter and damaged bytes.
- The layout suite covers only what Chrome and Vellum agree on; Prompt 18 adds text wrapping, grid and stroke-in-layout.
  `StackLayout` treats a grid as a column and ignores the dirty set (the suite checks that a dirty-set layout equals a
  full one, which it passes trivially).
- `Op::Update` goes through `serde_json` in the fake (merge the patch into the node's JSON, read it back as a `Node`),
  because core has no serde. Prompt 11 must decide where patch application lives; `vellum-types` is the natural place.
- Empty text with at least one run measures as one empty line; a transaction with no ops is refused (`OpErrorKind::Empty`).
  Both are my choices; the suites encode them for the fakes only where every implementation should agree (the empty
  transaction, an empty string's width of 0).

Verified (each exited 0):
- `cargo test -p vellum-core -p vellum-contracts` (core 9, contracts 31), `cargo test -p vellum-codec-json` (87),
  `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- `pnpm deps:check`, `pnpm types:check`, `pnpm format-spec:check`, `pnpm checksum`, `pnpm format:check`, `pnpm lint`,
  `pnpm type-check`.
- Failed: `pnpm test:scripts`, 148 passed, 1 failed, 1 skipped: the same pre-existing REFERENCE-INDEX test (Needs a person).

Test order: the twelve suites and `fakes_pass_their_suites.rs` were written first and did not compile (no fakes). All
twelve fakes then passed on the first run, so I wrote the broken-implementation tests next to show the suites bite; one
(`store_whose_undo_is_not_per_actor`) first failed on a different assertion than intended, and I changed the saboteur
(and added a message to the undo-attribution assertion) so it fails on the rule it names. Clippy then asked for a boxed
`Node` in `Op::Insert` and the fake's `Step`.

## Prompt 11: Ops I, structural ops, validation, origins

Committed as `fbf5f4d`. Prompt 10 is committed (`eb529c5`).

Built:
- **Origins in `vellum-types`** (`origin.rs`): `Actor` (person, agent, file, system), `Origin { actor, session?, turn? }`
  with no default, and the ULID ids `SessionId`, `TurnId`, `AgentId`, `DeviceId`, with TypeScript bindings. `FileReason`
  and `Subsystem` are string enums. `ActorId` is gone; undo is keyed by `Actor`.
- **`vellum-types` helpers the op layer needs** (core has no serde): `Node::patched` (applies a `NodePatch`, names the
  property at fault), `Node::set_position`, `Variable::aliases`, `referenced_variables` (every `$var`, skipping
  `metadata`).
- **`vellum_core::ops`**: `Transaction`, the eleven ops (insert, update, delete, move, copy, replace, set_variables,
  reorder, wrap_in_frame, group, ungroup), `plan` (checks a batch against a copy, each op seeing the ones before it) and
  `apply` (the one write path). Errors carry the op index, a path inside the op (`to.parent`, `node.children[1]`,
  `patch.opacity`, `set.<id>`) and a kind; `OpError::location()` prints `ops[2].patch.opacity`. Ops lower to five
  `Edit`s (insert, remove, move, set, set-variable), each of which checks the tree (ids unique across pages and
  components, parent kinds, cycles, boundaries, index) and returns its exact inverse. `apply_edits` is what stores use
  for commits, undo, redo and sync. `check_document` is the whole-document validator the property test uses.
- **`DocStore` port**: `apply(Commit) -> ChangeSet` (infallible: the commit was checked against this store);
  `ChangeSet` gains `variables`; `undo`/`redo` take `&Actor`.
- **`vellum-contracts`**: the fake store applies commits with `apply_edits` and keeps its own serde spelling of edits
  for sync; the suite writes through `ops::apply`. The "not atomic" saboteur became "applies part of a commit" (a store
  can no longer cut a commit down), and the shared-undo saboteur now pops whoever wrote last.
- Dev-dependencies `serde_json`, `proptest`, `trybuild` (1.0.122) on `vellum-core`, allowlisted, recorded as C19 in the
  version baseline.

Reference note (no code ported): Graphite's `document_history.rs` keeps whole-document snapshots in capped undo and
redo stacks; Excalidraw's `delta.ts`/`store.ts` keep per-change deltas with `inverse`, `applyTo` and `squash`, committed
by the store. Vellum follows the delta approach (each op lowers to edits with their exact inverse), because undo is
per actor and must revert one transaction without rolling back others; whole snapshots cannot do that. There is no
squash yet; Prompt 14 decides how undo steps merge in Loro.

Differs from the guide, or limits:
- Invariant 1's `pub(crate)` wording is met with a witness type instead (see "Needs a person").
- Not validated yet: an instance naming a component that exists, variable type matches at bind time (Prompt 16), alias
  cycles at resolution (Prompt 16 still owns the typed error; `set_variables` refuses cycles now). A `$var` must name a
  variable of the same document (ADR-010), and removing a variable still in use is refused.
- A `$var` introduced in an op must exist when that op runs; put `SetVariables` first in a batch.
- The property test (1,024 cases, up to 9 batches of 1-2 random ops on a sample document) accepts at least one batch
  in a little over half the cases; most random batches are refused, which also exercises "refused leaves no trace".
- I could not run the trybuild cases before the code existed (they need the `Commit` type to fail in the right way);
  before Prompt 11 the first case, calling `DocStore::apply` with a transaction, compiled.

Verified (each exited 0):
- `cargo test -p vellum-core ops::` (33), `cargo test -p vellum-core --test trybuild` (5 compile-fail cases),
  `cargo test -p vellum-types` (incl. new `origin` 5, `patching` 4), `cargo test -p vellum-contracts` (31),
  `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- `pnpm types:gen` (8 new files), `pnpm types:check`, `pnpm format-spec:check`, `pnpm deps:check` (same cargo-deny
  warnings as before), `pnpm checksum`, `pnpm format:check`, `pnpm lint`, `pnpm type-check`, `pnpm clean-room`,
  `pnpm copy-lint`.
- Failed: `pnpm test:scripts`, 148 passed, 1 failed, 1 skipped: the same pre-existing REFERENCE-INDEX test.

Test order: the types tests (`origin`, `patching`) failed to compile first, then passed. The op tests (structural,
validation, inverse, Invariant 1) were written against the planned API and failed to compile; after the
implementation the first runs failed on my own fixtures (text runs need `font` and `size`, booleans need `op`, stars
need `points` and `innerRatio`), not on the code. The first property-test generator was nearly vacuous (83% of runs
accepted nothing), so I biased it toward valid places and shorter batches. One contracts saboteur failed on the wrong
assertion (its redo forgot to re-record the actor) and was fixed to fail on the rule it names.

## Prompt 11: acceptance follow-up

The existing implementation is committed in `fbf5f4d`; the old hand-off incorrectly said it was uncommitted.
This follow-up is uncommitted.

Built:
- `SetVariables` checks unchanged aliases pointing to changed definitions, against the final definitions of the op.
  A type change that breaks an alias rejects the entire batch, with the op index and the exact `set.<target>` path.
  Changing the alias and its target to the same new type together remains valid and undo restores both definitions.
- Three regression tests cover rejection without document/history changes, changing both definitions, and an alias
  with multiple values whose second target fails. The 1,024-case property test now generates literal definitions and
  type changes as well as aliases, and independently round-trips each accepted document through the format types.
- Three compile-fail cases prove a transaction needs an origin, an origin needs an actor, and a transaction has no
  default. The existing five write-path compile-fail cases still pass.

Reference note (no code ported): Graphite retains capped snapshot stacks and a working-copy history cursor;
Excalidraw represents inserted/deleted property deltas, reverses them for undo, and applies them through its store.
Vellum keeps transaction edits and exact inverses, with mandatory origins for actor attribution. This follow-up adds
validation only; the existing Prompt 11 design choices and Prompt 14 history/performance work remain as recorded above.
Sibling projects were checked; their storage transactions and UI undo patterns do not provide a reusable pure document op layer.

Verified (each exited 0):
- `cargo test -p vellum-core ops::` (36), `cargo test -p vellum-core --test trybuild` (8 compile-fail cases),
  `cargo test --workspace`, `cargo deny check` (advisories, bans, licences and sources; existing unused-wrapper warnings).
- `bash scripts/check-deps.sh`, `pnpm deps:check`, `pnpm lint` (includes workspace clippy with `-D warnings`), `pnpm type-check`,
  `pnpm format:check`, `pnpm types:check`, `pnpm format-spec:check`, `pnpm clean-room`, `pnpm checksum`, `git diff --check`.
- Failed: `pnpm test`, only the pre-existing reference-index script test (`missing: R3 taffy: main.rs`);
  script tests: 148 passed, 1 failed, 1 skipped. The full Rust suite passed independently.
- Environment: sandboxed pnpm could not verify its registry signature, and sandboxed cargo-deny could not lock the
  advisory cache. Both checks succeeded outside the sandbox; no verification was disabled.

Test order: the alias-target regression failed on the original code because the invalid batch was accepted. After
the initial fix, the multiple-value alias regression failed because the error named the first changed target rather
than the one with the mismatched type. Both pass with the final validation. The new compiler cases first generated
diagnostics showing the missing fields/default, which were reviewed and saved as their expected failures.

## Prompt 12: component, slot and geometric ops

Built:
- `CreateComponent`, `ToggleComponent`, `Detach`, `ReplaceInInstance`, `MakeSlot`, `Scale`, `Boolean`,
  `OutlineStroke`, `Align`, `Distribute`, `Flip`, and the explicit `ConvertScript` placeholder, through the existing
  op/commit path. `SetComponent` edits include exact inverses and the fake store's sync wire format.
- Detach applies variants, overrides and slot content while keeping nested instances and variable bindings.
  Replacement addresses remain stable across variant replacements. Inherited layers cannot be moved into instances.
  Local component references, cycles, override paths, owned payload ids and slot placement are checked on writes
  and after edit replay, so generic insert/update cannot bypass the new operations' constraints.
- Scale fixes measured Hug/Fill dimensions and numeric bindings; scales content, text, strokes/dashes, radii,
  spacing, positions and effects; preserves dimensionless values and colour links. Instances receive overrides,
  including nested instances, structural overrides and slot content. Anchor rotation/mirroring uses the box centre.
- `fixtures/ops/{scale,slots}.vellum` are canonical TEST DATA documents. Twenty-six new core tests include exact
  inverses, atomic refusals and 256 random mixed-operation sequences; one contract integration test covers component
  create/sync/undo/redo. Existing Prompt 11 follow-up changes were present at session start and are preserved.

Choices and limits:
- Definitions live off-page in this format: create leaves an instance with the source id and creates a fresh-id
  definition; non-frame sources become a frame around the original shape. Toggle back detaches the instance and
  removes the definition only if otherwise unused. Sparse instance common properties inherit omitted defaults.
- `plan_with_inputs` / `apply_with_inputs` accept measured boxes, resolved numbers and parent-coordinate paths.
  Missing derived inputs are refused. Later resolver/layout/path-adapter prompts supply them; no vendor entered core.
  Booleans retain editable children, use document stacking order, and subtract the first child from the rest as the
  type documentation specifies. Alignment/distribution currently require absolute, unrotated, unflipped siblings.
- `PathOps::outline_stroke` is new and defaults to an explicit adapter error. `StrokeStyle` cannot represent dash
  patterns or per-side widths, so outlining those refuses rather than losing geometry. The real path adapter is
  still a scaffold; tests verify delegation with a test double, not real curve boolean/stroke fidelity.
- `convert_script` returns `NotImplemented("convert_script")` until Prompt 26. No UI or render goldens were required.
  Validation still clones/read-walks documents; Prompt 11's performance question remains for Prompt 14.

Reference note (no code ported): Graphite's resize helper transforms geometry around a chosen anchor and maps
between viewport and document coordinates. Vellum keeps the anchor calculation in model coordinates and scales
stored length properties, because bound lengths must become fixed and instances must carry overrides. Kurbo's
bounded offset-curve machinery stays behind PathOps rather than entering the pure op layer. Sibling projects were
searched; none provides a reusable Rust canvas component/scale operation layer.

Verified (each exited 0):
- `cargo test -p vellum-core ops::` (62); `cargo test -p vellum-contracts --test component_ops`;
  `cargo test --workspace` (including all eight write-path compile-fail cases).
- `cargo clippy --workspace --all-targets -- -D warnings`; `cargo deny check` (all four checks);
  `pnpm deps:check`; `pnpm lint`; `pnpm type-check`; `pnpm format:check`; `pnpm types:check`;
  `pnpm format-spec:check`; `pnpm clean-room`; `pnpm checksum`; `git diff --check`.
- `cargo run -p vellum-codec-json --example canonicalize -- --check fixtures/ops/scale.vellum fixtures/ops/slots.vellum`.
- Failed: `pnpm test:scripts`: 148 passed, 1 failed, 1 skipped; only the recorded REFERENCE-INDEX failure
  (`missing: R3 taffy: main.rs`). No specification or reference-index file was modified.
- Environment: sandboxed pnpm could not verify its registry signature. The pnpm gates passed outside the sandbox
  with signature verification enabled; no verification was disabled.

Test order: the acceptance tests failed to compile before the API existed. Subsequent regressions first failed
on doubled instance scaling, duplicated style on non-frame components, generic-write bypasses, stale measured
bounds and variant replacement addressing; the implementation was corrected for each. Initial scale assertions
were corrected to compare floating-point JSON numbers rather than integers. Clippy requested collapsed conditionals.

## Prompt 13: Guard, leases, pause set, limits

Built:
- `vellum_core::lease`: the §10.2 machine (`LeaseState::next` over `Claim`, `Activity`, `Release`, `Expire`, `Stop`,
  `PauseAll`), `Leases` (one per frame), `BlockedPairs` and `LEASE_IDLE_MS` (60 s). Time comes in as a `Timestamp`;
  a lease whose expiry has come is free even before `expire` drops it.
- `vellum_core::guard`: pure `guard(write, leases, pause_set, blocked_pairs, limits, now) -> Verdict` with a typed
  `Rejection` (`Leased`, `AllPaused`, `Paused`, `Stopped`, `StoppedHere`, `TooManyOps`, `TooManyNodes`), each with a
  `@vellum/config` key and a plain-language `message(name)`. `PauseSet`, `Limits`, `GuardState` and
  `apply_guarded`, which checks the actor before validating, then plans, guards, commits and extends the agent's
  own leases. `ops::commit` is a new `pub(crate)` helper shared with `ops::apply`.
- `packages/config` exports `guardReasons`. 34 tests: a 22-row lease transition table (free, held, expired x every
  event), an exhaustive 160-row guard table (person, agent, file, system x free, own, other's, other's expired
  lease x running, Pause all, paused, stopped, other paused x blocked or not), each way of writing into a lease,
  limits, messages, config parity and the guarded write path (Invariant 4: Pause all rejects the next agent write
  and the store is untouched).

Choices (in the code until you say otherwise):
- The guide's signature has no document, but "inside a frame" needs the tree, so `guard` takes a `Write` built from
  the document, the transaction and its plan; it reads the planned edits, so every op kind (including Prompt 12's)
  is covered by one rule. A write is inside a frame when an edit sets, inserts under, removes or moves anything in
  its subtree, or removes or moves the frame or an ancestor. Copying out of a frame is a read. Variables are outside
  every frame. A lease on a component definition's node also blocks replacing or removing that definition.
- Only agents are checked. People, file changes and subsystems always pass. Order for an agent: stopped, Pause all,
  paused, op count, inserted nodes, frames it was stopped on, frames others hold.
- Resume all lifts Pause all and individual pauses, not a Stop (a stopped turn is over; the next turn resumes the
  agent). Pause all frees every lease and blocks nobody. Stop on a lease tab frees it and blocks the pair.
- Recording a rejection in History and naming agents are the daemon's job (Prompts 30+); the Guard returns the
  typed reason. No reference row covers the Guard in guide §3.2, so there is no reference note. Sibling projects
  were not a source: none has an agent lease or pause model.

Verified (each exited 0):
- `cargo test -p vellum-core -- guard:: lease::` (34; see "Needs a person" for the guide's form); `cargo test --workspace`;
  `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all --check`.
- `pnpm deps:check`, `pnpm lint`, `pnpm type-check`, `pnpm format:check`, `pnpm types:check`, `pnpm format-spec:check`,
  `pnpm clean-room`, `pnpm checksum`, `pnpm copy-lint`, `pnpm --filter @vellum/config test`.
- Failed: `pnpm test:scripts`: 148 passed, 1 failed, 1 skipped; only the recorded `missing: R3 taffy: main.rs`.

Test order: the new tests failed to compile before the modules existed. After the first implementation passed,
two mutations survived (removing an ancestor of a leased frame; replacing a component definition), so I added a
test for each; both now fail under the mutation and pass on the code. A check against the document after the write
was removed: no test could tell it apart, because the first edit reaching into a frame always names a node that was
already there.

## Prompt 14: DocStore on Loro, per-actor undo, undo this turn

Built:
- `crates/adapters/doc-loro`: `LoroDocStore` (loro 1.16.2). Pages and components are roots of one movable tree with
  fractional indexes; each document node is one tree node whose meta map holds each property as a JSON string; the
  rest of the document is in two maps. The store keeps the document in our types too, so `snapshot()` never reads
  Loro, and reads Loro back whole after an import. Stores opened from the same document write it as the same
  genesis peer (derived from its content), so they sync without duplicating it; updates from another document are
  refused. Commits carry their origin as Loro's commit message. `read_back()` returns the document as Loro holds it.
- `vellum_core::history` (pure): `History` records each commit's inverse and a footprint, and does per-actor undo and
  redo and "Undo this turn". Undo keeps later work by others (properties they changed since, nodes they built on or
  moved). Changes from other stores count as later work. Used by the Loro store and the in-memory fake.
- Port: `DocStore::undo_turn`, `UndoTurn`, `Revert`, `Kept`, `SyncError::DifferentDocument`; `ops::revert`. `vellum-core`
  now depends on `serde_json` (on its allowlist since Prompt 2) to restore node properties one by one.
- Contract suite: undo keeps later work, redo keeps work done since, "Undo this turn" (clean and with later work), and
  `undo_is_yours`, Invariant 3's property test (3 actors, 24 interleaved transactions of renames, opacity and rotation,
  inserts, moves and deletes, then random undos and redos by Alice; 200 seeds on each store). A store that undoes
  whole nodes fails it (`suites_catch_broken_implementations.rs`).
- doc-loro tests: the contract suite and the property test; both again with Loro read back and compared after every
  change; concurrent edits converge (different properties of one node, inserts, a move concurrent with a delete);
  saved updates reopen; another document's updates are refused; a change from another device is kept by undo;
  the fixture documents round-trip and take renames and undo; our ids map one-to-one onto live tree nodes; nothing
  public names a Loro type.
- ADR-004 gains a "Prompt 14" section (layout, genesis, undo outside Loro, rules, numbers, reference notes). The version
  baseline row for loro now names the workspace and doc-loro.

Differs from the guide:
- Loro's `UndoManager` is not used: it is per peer, not per actor, has no turns, and grows with the document
  (ADR-004). Undo runs on our own recorded inverses instead.
- Guide FINAL says `cargo test -p doc-loro`; the package is `vellum-doc-loro` (see "Needs a person").
- An ADR number was not added: Appendix E allots them, so the decision is a section of ADR-004.

Not verified or not done:
- Timing only on the M5 Pro, native, release; not at 50k nodes, not in WASM. Undo at 10k: 14.5 ms (budget 16).
- Imports cost a full read of the document from Loro and a full comparison; Prompt 63 should read Loro's events.
- Concurrent edits that leave an invalid document (an instance whose component was removed elsewhere) are read back
  as they are; nothing reconciles them yet.
- The history is session-scoped and never trimmed.

Verified (each exited 0):
- `cargo test -p vellum-doc-loro` (contract suite, property test, read-back, sync, mapping, public API);
  `cargo test -p vellum-doc-loro --release --test undo_cost -- --ignored --nocapture` (prints the numbers above);
  `cargo test -p vellum-contracts`; `cargo test -p vellum-core history::`; `cargo test --workspace`;
  `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all --check`.
- `pnpm deps:check` (cargo-deny accepts loro's licences), `pnpm lint`, `pnpm type-check`, `pnpm format:check`,
  `pnpm types:check`, `pnpm format-spec:check`, `pnpm clean-room`, `pnpm checksum`, `pnpm copy-lint`.
- Failed: `pnpm test:scripts`: 148 passed, 1 failed; only the recorded `missing: R3 taffy: main.rs`.

Test order: the new contract checks failed on the old fake first ("Alice's undo put back a property Bob changed
after her"). The property test then found that a write setting a value it already has leaves no trace in the
edits; I made that rule explicit (ADR-004, "Needs a person") rather than track op-level writes. The doc-loro tests
were written before the adapter; once it compiled they passed, so I added the read-back wrapper and the component
and variable round trips to make sure Loro's own state, not only the store's copy, was being checked. Two
mutations of the undo rules: keeping a node someone built on is caught; not moving back a node someone moved since
survived, so the suite now has a test for it, which fails under the mutation and passes on the code.

## Prompt 15: diff to ops, three-way merge, merge-driver core

Built:
- `vellum_core::diff::diff(old, new)`: the ops that turn one document into another, by node id and property. A node
  with the same id, kind and tree in both is updated (only the properties that differ), moved or reordered; only
  nodes that are new, gone or changed kind or tree are inserted and deleted, as whole subtrees without what lives on
  inside them. Covers pages, the shell, variables and components; ordered so every step is accepted.
- `vellum_core::merge::merge(base, ours, theirs) -> Result<Merged, MergeError>`: three-way merge by node id and
  property, with `Conflict` (`Properties`, `Removed`, `Moved`, `Order`, `Entry`). Same-property conflicts and
  removed-versus-changed nodes leave a conflict node holding both versions.
- `vellum-types`: the `conflict` node kind (format, JSON Schema, TypeScript regenerated). The op layer: page, shell
  and component ops and edits (ADR-010 "Prompt 15"); the Loro store and the in-memory fake write them; the Guard
  knows them; `ChangeSet` and `Kept` report them. References are checked once per transaction.
- Tests: 18 diff and 15 merge cases by name; 11 for the new ops; the contract check `document_parts` (pages, shell
  and components applied, synced, undone and redone on both stores, Loro read back after every step); property tests
  over random branches of every fixture document (`tests/diff_merge.rs`: diff leads to the new document, also between
  two branches and between every pair of fixtures; merge(b, x, b) == x; merge(b, b, y) == y; every merge valid; a
  clean merge commutes; at least a fifth of random merges clean; every op kind the generator makes is accepted
  often); the cargo-fuzz target `merge` and its `fuzz_smoke` corpus in `cargo test`.

Differs from the guide:
- The guide asks for a conflict marker node; the format had none, so it gained a node kind (ADR-010).
- Diffing whole documents needed ops the guide does not list (pages, shell, components). They are in ADR-010.
- Guide FINAL `cargo test diff:: merge::` does not parse; see "Needs a person".

Found and fixed on the way (Prompt 14 code): redo counted the actor's own earlier redos as later work, so redoing two
changes to one page, component or variable kept the second; making a component or variable counted as changing it;
changing a component's name counted as replacing its tree. Proptest also found that a clean merge was not symmetric
when both sides moved the same nodes into one place; runs now line up on the nodes they share.

Not verified or not done:
- No `vellum merge-driver` CLI, exit codes or `.gitattributes` (Prompt 60), no banner or resolver UI (Prompt 53).
- Diff and merge were not timed on large documents.
- Fuzzing ran locally only (nightly, cargo-fuzz 0.13 on this Mac): about 15,000 runs in 2 minutes and 31,118 in 5 minutes at libFuzzer's default input length (it stayed under 17 bytes), then 25,853 in 4 minutes with inputs up to 512 bytes; no failures. CI does not fuzz.
- The proptest regression file `crates/vellum-core/tests/diff_merge.proptest-regressions` holds two cases (one from a
  deliberate mutation check, one real); both pass and replay on every run.

Verified (each exited 0 unless said):
- `cargo test -p vellum-core -- diff:: merge::` (33 unit, 4 integration); `cargo test -p vellum-core --test diff_merge`
  (also 25 more runs with fresh proptest seeds); `cargo test --workspace`; `cargo clippy --workspace --all-targets
  -- -D warnings`; `cargo test -p vellum-doc-loro --release --test undo_cost -- --ignored --nocapture`.
- `pnpm format:check`, `pnpm types:check`, `pnpm format-spec:check`, `pnpm checksum`, `pnpm copy-lint`,
  `pnpm clean-room`, `pnpm lint`, `pnpm type-check`, `pnpm deps:check`.
- Failed: `pnpm test:scripts`: 148 passed, 1 failed, the recorded `missing: R3 taffy: main.rs`.
- Mutation checks: breaking the id tie-break for inserts at one place fails the commutativity property; skipping the
  diff's reorder step fails the diff property; placing every new page first in Loro fails the read-back suite.

Test order: the type test for the conflict node, then the op tests, then `document_parts`, failed first (missing
variants, then the three redo bugs above on the fake). The diff and merge cases ran against stubs and failed (16 of
17 diff cases; the 18th, an id used twice in the new document, panicked before it returned an error) before the code was written. The generator check failed first too: correlated bytes never chose
some op kinds, so test inputs now come from SplitMix64.

## Prompt 16: resolver I, variables, theme axes, aliases

Built:
- `vellum_core::resolver::variables`: `Theme` (the first value of each axis, a frame's `theme` replacing the axes it names),
  `resolve_variable` (last matching `when` wins; aliases followed under the same theme; typed `VariableError`: `Unknown`,
  `NoValue`, `AliasType`, `Cycle`, `TypeMismatch`), `resolve_value` / `resolve_visible` (a boolean variable drives
  visibility), and `Resolver` with `ResolvedNode`: a tree of nodes with every reference replaced by its value, cached
  per node with a dirty set (`invalidate(doc, &ChangeSet)`), a variable-to-readers index and the inherited theme.
- Bind-time type checks: `vellum_types::{Bindings, BindingsMut, BindingPath}` walk every place a node, style or effect can
  hold a variable with its type (one macro writes the reading and the writing walk). The op layer refuses a wrong type
  with `OpErrorKind::BindingType` at a path such as `patch.fills[0].color`, and a variable that changes type while
  something uses it; `check_document` reports the same. `Node::without_children` and `Node::theme` were added.
- Tests: 51 unit tests (`resolver::variables::tests::{theme,values,bind,cache}`); `tests/resolver_cache.rs` (300 random
  cases and a fixed corpus of 400 seeds: cached resolution equals fresh after random accepted edits, including
  variable values, axes, frame themes and bindings; several change sets can pile up before a resolve).

Differs from the guide:
- None in what it asks. The guide's FINAL command `cargo test resolver::variables` runs as written (53 tests).
- Types are checked once per transaction (like references in Prompt 15), not after each op, because the diff changes a
  variable's type before it rebinds what used it. Found because the Prompt 15 property tests failed on it.

Found and changed on the way:
- `content.vellum` binds a colour variable to a script input, which a stricter first rule refused. The rule changed
  (see "Needs a person"); the fixture did not.
- A replay that names the op at fault refused the first passing mismatch, not the one the final check found. It now
  refuses only the error the final check found.

Not verified or not done:
- Nothing was timed on a large document. Resolution of instances, overrides, slots and libraries is Prompt 17.
- Props overrides are not type-checked (see "Needs a person").

Verified (each exited 0 unless said):
- `cargo test resolver::variables`; `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `pnpm format:check`, `pnpm types:check`, `pnpm format-spec:check`, `pnpm checksum`, `pnpm copy-lint`, `pnpm clean-room`,
  `pnpm lint`, `pnpm type-check`, `pnpm deps:check`.
- Failed: `pnpm test:scripts`: only the recorded `missing: R3 taffy: main.rs`.
- The corpus test with 30,000 seeds in release (3.4 s), once.
- Mutation checks: not evicting a variable's readers, not comparing the inherited theme, not evicting updated nodes, and
  reusing a parent after its children changed each fail the property test; picking the first matching entry fails several
  unit tests; removing cycle detection makes the cycle tests loop until killed.

Test order: the test files were written first and failed to compile (no `Theme`, `Resolver`, `ResolvedValue`,
`OpErrorKind::BindingType`); all 51 passed on the first run of the finished code, which is why the mutation checks above were run.


## Prompt 17: resolver II, components, instances, overrides, slots and libraries

Built:
- `resolver::components` and the complete `resolver::Resolver`: addressed resolved occurrences, typed prop
  bindings/defaults, variants, props/replacement/children overrides, nested instance paths, per-instance slots,
  suggestions, problems and missing-library flags. Own values and unchanged subtrees reuse Arcs; cached/fresh
  equality is checked over 128 generated mixed-edit sequences.
- `resolver::libraries`: an injected `LibrarySource`, relative/transitive imports, per-pass refresh, seeded
  last-known copies with explicit missing/unreadable flags, and Locate through an updated import.
- Paste planning as ordinary ops: Link/Copies, copied component dependencies (including slot suggestions), fresh
  ids and path remapping, variable/alias/theme transfer, per-variable Use existing/Add renamed, style/font definitions,
  rebased imports and explicit dependency-byte handoff. No direct store mutation or fabricated Origin.
- Types: optional prop `bindings` and instance `library` alias. JSON Schema, format reference and 111 TypeScript
  exports regenerated. Canonical TEST DATA parity documents in `fixtures/resolver/`, with a test-to-parity map.
- 27 new tests. Local ops share the instance recipe, apply props on detach, check unselected variants and expanded
  override bindings, and preserve the existing precise binding-error paths. Imported ops without a definition input
  explicitly refuse instead of using a same-id local component.

Decisions and differences:
- The person selected explicit node-path/property prop bindings; the Prompt 8 open decision is removed above.
  Local props-override variable types are now checked; the Prompt 16 open item is removed. Imported overrides are
  typed and resolved with source/owner scope by the resolver. ADR-011 records the representation and behavior.
- Guide FINAL has two cargo filters before `--`; the working form used is
  `cargo test -p vellum-core --lib -- resolver::components resolver::libraries`.
- No reference row is assigned to Prompt 17 in guide §3.2. Sibling projects were searched for reusable Rust patterns;
  none was found. No reference code was ported; the existing typed patch/component machinery is shared.

Found and fixed:
- Tests first exposed missing APIs, then ignored props on detach, override type-check bypass, a local-definition
  requirement for imported links, finite nested slot content mistaken for recursion, missing suggestion dependencies,
  and owner-scope loss in deep library overrides. The full suite exposed broad validation hiding Prompt 16's error
  paths; reference and expanded-binding checks were separated without weakening the assertions.
- Restricted pnpm could not verify its pinned release against the registry. Registry access with the required
  tool permission resolved it; regeneration and repository commands then ran with verified pnpm.

Not verified or not done:
- No large-document timings, GPU or UI screenshot checks. This is an engine prompt, with no user-facing UI.
- Library cache persistence and copying asset/font/script bytes are Prompt 24; live watching is Prompt 28; dialogs
  and starter-system distribution are later prompts. Existing imported detach/scale/replace ops need library-derived
  input and currently return MissingInput. Nested links to other libraries remain linked when copying definitions.

Verified (each exited 0 unless stated):
- `cargo test -p vellum-core --lib -- resolver::components resolver::libraries` (27 tests), `cargo test --workspace`.
- `pnpm types:gen`, `pnpm format-spec:gen`, `pnpm types:check`, `pnpm format-spec:check`, `pnpm format:check`,
  `pnpm lint`, `pnpm type-check`, `pnpm deps:check`, `pnpm clean-room`, `pnpm checksum`.
- `cargo run -p vellum-codec-json --example canonicalize -- fixtures/resolver/components.vellum
  fixtures/resolver/imports.vellum fixtures/resolver/library.lib.vellum`; also the same command with `--check`.
- Failed: `pnpm test:scripts` (148 pass, one skip, the recorded `missing: R3 taffy: main.rs` failure).
- Failed: `pnpm cargo:deny` (three existing Loro-chain unmaintained advisories listed above; bans, licenses and sources pass).


## Prompt 18: production layout on Taffy and browser oracle

Built:
- Core layout-tree conversion from addressed resolver occurrences, typed ADR-002 sizing preparation, fallback
  validation and safe hug-cycle diagnostics. Grid, absolute placement, hidden subtrees, stroke space and text
  inputs are included. Layout result keys are `NodeIdPath`, so component occurrences cannot collide.
- Persistent Taffy 0.14 adapter, injected TextEngine measurement/wrapping, style/content/topology reconciliation,
  dirty measurement invalidation and cached clean subtrees. A topology pre-pass prevents transient ancestry cycles.
- 186 Chrome-generated fixtures (51 existing, 135 new), a reproducible input matrix, shared oracle-to-port test
  helper, production CI lane and spike compatibility wrapper. All boxes match within 0.5 px, without exemptions.
- Tests: LayoutEngine contract; measurement counter; dependent text rewrap; hide/move/remove and reverse ancestry;
  128 generated mixed-edit sequences compared to fresh layout; resolved component addresses, numeric failures,
  empty hug fallbacks and ignored theme warnings. ADR-012 and the oracle runbook record choices and reference study.

Differences and findings:
- Stroke semantics were undefined; ADR-012 records the frame-own inward-space choice and the existing grid model.
  No format representation change; type descriptions and generated format documentation were refreshed.
  `LayoutStyle` gains the missing inputs and `LayoutResult` gains typed problems.
- Chrome snaps border widths to whole pixels; the oracle expresses each fractional remainder as padding, keeping
  exact inward stroke space and absolute coordinates. This fixed a real >0.5 px mismatch; tolerance stayed 0.5.
- Tests failed first on missing APIs, the 51-fixture floor and missing stroke attributes; later on empty hug
  fallbacks and an ignored theme warning incorrectly blocking geometry. Test setup corrections were also needed
  for the existing library fake signature, document envelope and oracle index ids; product assertions were retained.
- Chrome exited in the sandbox; the same generator passed with tool permission. The pnpm shim could not verify
  registry signatures while restricted; permitted registry verification resolved it. The spike's new dependency
  graph could not update the index while restricted; `--offline` resolved cached dependencies and passed.
- Final `pnpm types:check` caught old FrameNode descriptions after concurrent Rust tests and regeneration;
  regeneration after the Rust run finished restored current output. The final bindings check passes.

Limits:
- Preparation/reconciliation still scan the tree; counter tests prove clean text subtrees reuse measurements,
  and dependent nodes remeasure. No large-document timing claim. TextEngine/font changes require dirty text or all.
- No browser font/text parity fixtures (Prompt 19), GPU or UI checks. Geometry fixtures require Chrome only when
  regenerating; ordinary Rust tests use committed expectations. CI configuration was checked locally, not on GitHub.

Verified (each exited 0 unless stated):
- `cargo test -p vellum-layout-taffy` (contract, 186 oracle fixtures, 128 generated edit sequences and regressions),
  `cargo test -p vellum-core --test layout` (6 tests), `cargo test --workspace`.
- `cargo test --offline --manifest-path spikes/layout/Cargo.toml`.
- `node scripts/fixtures-layout.mjs`; `node scripts/gen-layout-oracle.mjs`; `pnpm layout-oracle --check`
  (186 fixtures, Chrome 155.0.8059.41); `pnpm types:gen`, `pnpm format-spec:gen`.
- `pnpm lint`, `pnpm type-check`, `pnpm deps:check`, `pnpm format:check`, `pnpm clean-room`, `pnpm types:check`,
  `pnpm format-spec:check`, `pnpm checksum`.
- `VELLUM_LANES=chrome pnpm test:scripts`: Chrome lane passes; 151 passed, one failed on the existing
  `missing: R3 taffy: main.rs` reference-index issue.
- `pnpm cargo:deny`: existing three Loro-chain unmaintained advisories fail; bans, licenses and sources pass.


## Prompt 19: text on Parley and explicit font sources

Built:
- Parley 0.12 TextEngine: rich runs, bidi, CJK breaks, emoji, spacing, font/default and explicit line heights,
  physical horizontal alignment, justify, font-metric underline/strike, links as data, vertical alignment,
  auto-width/auto-height/fixed boxes and explicit overflow without dropping content.
- FontSource with document → cached Google → system → bundled Inter priority; CSS weight/style matching
  within the first tier containing the family. Tier-private aliases prevent same-family mixing. Byte inputs
  and stable handles are shared with renderers; missing-family substitutions and uncovered clusters report ranges.
  Bundled Inter is a pinned, unmodified Google Fonts variable font, with OFL license and hashes beside it.
- Pure text ports retain logical UTF-8 clusters, vertical glyph offsets, variable axes, synthesis, decoration
  metrics, source-run ownership and font bytes. Resolver-to-layout conversion keeps the new text inputs and sizing.
- Extended shared TextEngine contract on production and fake, plus a broken-implementation check that proves
  dropping links/decorations fails it. Eight ordinary production behavior tests plus the existing smoke test.
- Reproducible TEST DATA oracle from Prompt 5's unchanged spike layout: all 30 fixtures match fonts, glyph ids,
  sizes, line counts and positions within 0.001 px. Separate font-contract, RTL-offset, CJK and emoji-cluster checks.
  Tagged fonts CI lane; ADR-013, font asset notice/provenance and text runbook.

Differences and findings:
- Guide FINAL names `text-parley`; the workspace package is `vellum-text-parley`, as with the other adapters.
- The previous text ports could not represent the required rich styles or complete rendering inputs. They were
  extended without changing document types or the file format; `layout_box` applies box policy while existing
  `layout`/`measure` retain their explicitly constrained content contract.
- Tests failed first on absent APIs and dropped core conversion fields; then on line-box width mistaken for content
  width, overflow hidden by that mistake and Parley's synthetic empty-text placeholder. Those were corrected.
  The first fixture exporter used Rust debug strings with non-JSON Unicode escapes; it now emits valid JSON.
- Production enables `complex-scripts` to include ICU segmentation data; the original spike still emits missing-model
  notices when generating CJK fixtures. Its results match. Spikes/renderer's stale lockfile also gained Prompt 18's
  existing testkit dependencies when the new oracle example was built.
- Sibling projects and the guide's Parley/Fontique and Penpot text paths were read; ADR-013 records the reference
  study. No reference code was ported. The first Inter download's family name was Inter Variable, so it was replaced
  by pinned Google Fonts Inter before tests passed. Restricted DNS/pnpm verification needed tool permission.

Limits:
- Hosts must inject decoded SFNT bytes for document/cache/system fonts; disk persistence and WOFF/WOFF2 decoding
  remain Prompt 24, platform enumeration belongs to hosts. Multi-family collections return a typed error;
  single-family collections retain face indices. Missing Unicode coverage is explicit, not bundled in Inter.
- No GPU, renderer pixel comparison, large-document performance or WASM runtime execution in this prompt.
  WASM compilation passes. Renderers and hosts are later prompts; the new CI job has not run on GitHub.

Verified (each exited 0 unless stated):
- `cargo test --offline -p vellum-text-parley`; `cargo test --offline -p vellum-text-parley --test fonts_lane -- --ignored`;
  `cargo test --offline -p vellum-contracts`; `cargo test --offline -p vellum-contracts --test suites_catch_broken_implementations text_that_drops`;
  `cargo test --offline -p vellum-core --test layout`; `cargo test --offline --workspace`.
- `cargo check --offline -p vellum-text-parley --target wasm32-unknown-unknown`;
  `cargo clippy --offline --workspace --all-targets -- -D warnings`; `cargo fmt --all --check`;
  `cargo fmt --manifest-path spikes/renderer/Cargo.toml --all --check`.
- `node scripts/gen-text-fixtures.mjs`, also `--check` (verifies all five fixture font hashes before generation).
- `pnpm lint`, `pnpm type-check`, `pnpm deps:check`, `pnpm format:check`, `pnpm types:check`,
  `pnpm format-spec:check`, `pnpm clean-room`, `pnpm checksum`; `git diff HEAD --check`.
- Failed: `pnpm test:scripts`: 150 pass, one skip, one existing `missing: R3 taffy: main.rs` failure.
- Failed: `pnpm cargo:deny`: the three existing Loro-chain unmaintained advisories above; bans, licenses and sources pass.


## Prompt 20: scene, spatial index, hit-testing and snapping

Built:
- rstar 0.13.0 adapter: bulk rebuild, incremental envelope replacement/removal and occurrence-addressed
  queries. Extended SpatialIndex contract (production and fake) covers repeated descendants, duplicate writes,
  malformed bounds and removal precedence. No vendor dependency entered core.
- Pure page scene from resolved occurrences and parent-relative layout: accumulated rotation/reflection,
  inherited visibility/locks, nested rectangular/rounded clips, point and marquee selection with drawing order,
  deepest/top-level results, intersection/containment modes and viewport culling. Invalid syncs are atomic.
- Index-only snapping candidates, screen/zoom tolerance, edges/centres, equal gaps between and beyond neighbors,
  optional pixel grid, subtree exclusions and typed guide data. Deterministic `find_space(width,height,near?)`
  includes locked layers and nonselectable session reservations, uses 16 px clearance, and explicitly fails
  after 10,000 probes. No document/store mutations or host UI.
- 15 scene/contract tests plus an opt-in release benchmark; generated tests compare 128 random index edit
  sequences with the naive fake and 64 scene edit sequences with fresh scenes. Component occurrences share
  definition ids without aliasing. ADR-014 and the dependency version baseline record decisions/reference study.

Decisions and limits:
- SpatialIndex keys changed from NodeId to NodeIdPath to match Prompt 18's layout addresses. Selection is
  based on transformed layout boxes, including visible clipped portions; it does not test vector fill/stroke
  or glyph coverage. Rounded clip arcs target 0.01 px sagitta, capped at 128 segments per corner.
- Reconciliation still scans the page; only changed envelopes reach the index. Free-space search is bounded,
  deterministic and conservative about world AABBs; it does not promise the globally nearest placement.
- No reference code was ported. Sibling-project searches found no reusable Rust scene/index implementation.
  The guide's Excalidraw Renderer path moved under `packages/excalidraw/scene/`; that file was read.
- No GPU, UI or browser screenshot checks: this prompt builds engine queries. WASM compilation was verified,
  not WASM runtime execution. The benchmark is local timing evidence, not a cross-machine performance gate.

Test order and findings:
- Tests first failed on missing scene/adapter APIs and NodeId-only index signatures; rounded clipping then
  failed on its absent input field. A test document lacked required `createdWith`; the setup was corrected.
- Final review regressions failed on tiny nonzero clips treated as empty and finite radii overflowing when
  summed. Zero-area detection and normalized radius sums fix both. Clippy's two style findings were fixed.
- Restricted Cargo/pnpm registry verification failed or stalled; permitted registry access resolved the
  pinned dependencies/checks. Context7 had no matching rstar library; official docs.rs and the installed
  crate verified its API (0.13 envelope arguments are by value).

Verified (each exited 0 unless stated):
- `cargo test --offline -p vellum-index-rstar --test scene` (15 pass; benchmark ignored),
  `cargo test --offline scene::`, `cargo test --offline --workspace`.
- `cargo test --offline -p vellum-index-rstar --release --test scene scene::baseline_5000 -- --ignored --nocapture`.
  5,000-node release baseline: initial sync 7.29 ms; p95 point 0.71 µs, marquee 24.88 µs, culling 14.63 µs,
  snapping 72.67 µs, free-space search 42.46 µs, one-move reconciliation 2.80 ms. Other checks ran concurrently.
- `cargo check --offline -p vellum-index-rstar --target wasm32-unknown-unknown`,
  `cargo clippy --offline --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`.
- `pnpm lint`, `pnpm type-check`, `pnpm deps:check`, `pnpm format:check`, `pnpm types:check`,
  `pnpm format-spec:check`, `pnpm clean-room`, `pnpm checksum`, `git diff --check`.
- Failed: `pnpm test:scripts` (150 pass, one skip, the existing `missing: R3 taffy: main.rs` failure).
- Failed: `pnpm cargo:deny` (the three existing Loro-chain unmaintained advisories above;
  bans, licenses and sources pass).


## Prompt 21: layout check and display list

Built:
- Structured occurrence-addressed text overflow, ancestor clipping, auto-layout sibling overlap,
  outside-frame and hug-cycle diagnostics. World-space node snapshots and a problems-only projection
  retain every problem participant. Hidden subtrees are omitted; missing geometry fails explicitly.
- Renderer-neutral DrawCmd and typed shape/paint/stroke/image/effect/glyph resources in vellum-types,
  generated TypeScript bindings, DISPLAY_LIST_VERSION 1 and a deterministic little-endian buffer codec.
  The decoder validates versions, lengths, resource kinds, state balance and nonfinite wire scalars.
- Resolved/layout-to-display-list producer: R-tree viewport queries, ancestor state, clips, subtree
  opacity/blending, ordered appearance, boolean operands, text metadata and shared font bytes. Missing
  font data and unsupported node rendering are explicit problems. No document or store mutations.
- 16 integration tests on the production R-tree and six wire tests, including generated/mutated inputs.
  ADR-015 and the display-list runbook document decisions, wire layout and reference study.

Decisions and limits:
- Transform sets an absolute device matrix; world accumulation and viewport subtraction happen in f64
  before f32 conversion. Shape references carry backend-neutral descriptors; SVG parsing, boolean path
  lowering and actual drawing remain renderer work in Prompts 22–23. No PathOps adapter was introduced.
- Off-frame means no visible frame contains the complete transformed box. Overlap checks flow siblings
  in row/column/grid, with a 1e-9 square-pixel noise tolerance. Clipping uses Prompt 20's rounded polygons.
- Strokes/effects/text overflow conservatively expand spatial queries. Outset queries temporarily disable
  scene clipping and restore it afterward, preserving shadows cast back inside a clip. SVG/shaders disable
  culling when bounds are unknown. Text bounds use line/advance extents, not glyph outlines; stroke/blur
  assumptions and overdraw limits are in ADR-015. No GPU/pixel parity, WASM runtime or performance gate.
- The binary resource codec uses an internal serde_json::Value bridge, never JSON text on the boundary.
  Font bytes remain shared side resources. WASM allocation/host transport and consumers are later prompts.
- Read Excalidraw/Vello references and the existing renderer spike; sibling searches found no reusable
  implementation. No reference code was ported. Context7 verified serde_json's nonfinite-value behavior.

Test order and findings:
- Initial tests failed on missing APIs. Invalid test Document metadata, image-fit spelling and occurrence
  addresses were corrected. Regression tests then failed on f32 pan cancellation and a clipped-away box
  whose shadow re-entered the clip; f64 rebasing and conservative spatial queries fixed them.
- Clippy's default-field-reassignment test finding was fixed. Restricted pnpm verification stalled;
  permitted package-manager verification completed the checks. No dependencies or advisory policy changed.
- Guide FINAL needs `--` before its two test filters; the written command is rejected by Cargo.

Verified (each exited 0 unless stated; pnpm commands used TURBO_TELEMETRY_DISABLED=1 and NEXT_TELEMETRY_DISABLED=1):
- `cargo test --offline -p vellum-index-rstar --test engine_output` (16 pass);
  `cargo test --offline -p vellum-types --test display_list` (six pass);
  `cargo test --offline -- layout_check:: display_list::`; `cargo test --offline --workspace`.
- `cargo check --offline -p vellum-core --target wasm32-unknown-unknown`;
  `cargo clippy --offline --workspace --all-targets -- -D warnings`; `cargo fmt --all`.
- `pnpm types:gen`, `pnpm types:check`, `pnpm format-spec:check`, `pnpm lint`, `pnpm type-check`,
  `pnpm deps:check`, `pnpm format:check`, `pnpm clean-room`, `pnpm checksum`; `git diff --check`.
- Failed: `pnpm test:scripts` (150 pass, one skip, the existing `missing: R3 taffy: main.rs` failure).
- Failed: `pnpm cargo:deny` (the three existing Loro-chain unmaintained advisories above;
  bans, licenses and sources pass).
