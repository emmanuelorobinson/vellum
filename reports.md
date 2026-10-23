# Reports

What a person should know after each prompt: what was built, what was decided on the way, what was skipped or
could not be verified, and what only a human can do. AGENTS.md tells every session to read this file first and
to update it at the end of each prompt. Newest prompt last. Keep entries short and factual; say what failed.

## Needs a person

Current open items only. Resolved items, decisions taken for the owner and every earlier prompt entry are in
`addressed-reports.md`. Remove an item here when it is done and add it there.

### Engineering still open (an agent can do these)

- **Editor feature ports in the hosts (Prompts 49–54).** VS Code and the desktop edit through the daemon
  (ADR-033) and the Agent tab shows the daemon's sessions (`vellum/agentFeed`), but composer send and provider
  setup, presence and leases, the History ledger and hand-off panels, the shell's palette/settings hosts and the
  authoring hosts are not served. They stay empty or greyed.
- **Browser engine gaps (ADR-033).** Library documents, image fills, document fonts and script outputs are not
  given to the editor's WASM engine yet; library instances show as missing and image fills draw empty.
- **Visual acceptance.** Screen goldens from Prompts 46–70 (and the Electron `@design` goldens) exceed the
  unchanged 1% gate, mostly text rasterisation. Sheets in `target/screen-goldens/`.
- **Performance budgets (Prompt 71).** Native full-pipeline pan/zoom, drag and 200-op batch and the Canvas2D
  fallback exceed §12; open, idle RSS, Skia draw and both WASM size gates pass. In the browser, a frame with any
  shadow or blur still rasterizes the whole viewport on the CPU (ADR-017): about 150–400 ms per changed frame at
  1280×720 for `shapes.vellum`, more at 2× pixel ratio. GPU-native effects need to pass the goldens first.
- **Localization coverage (Prompt 70).** 127 common messages in 13 catalogs; advanced descriptions, dynamic
  diagnostics, desktop Home and the account/web copy still fall back to English.
- **Electron tests leak daemons.** Eight `vellum` daemons from earlier desktop test runs were still running in
  temporary run directories (some three days old); `afterEach` closes the app but not every daemon it started.
  Stopped by hand on 2026-10-23.
- **Agents reach the document, not yet all of it (ADR-034, phases 1b and 2).** Phase 1 added the `manage_pages`,
  `manage_components`, `instance_nodes` and `arrange_nodes` tools. Still open: the daemon applies agents' changes
  with no measured boxes, path geometry or script outputs, so boolean, outline stroke, convert script and
  hug-sized align, distribute or scale fail through agents; `execute` has no methods for the new ops, no print channel, no
  name-to-id map and no layout warnings; `browser` cannot read a selector's DOM or styles back.
- **`cargo clippy -p vellum --all-targets -- -D warnings` fails at HEAD.** `crates/vellum/src/daemon/documents.rs:185`
  (`scenes`) trips `type_complexity` under the installed toolchain (1.99). Not from ADR-034; the new files are clean.
- **pnpm advisories without a patched release:** `braces` (via ovsx/secretlint) and `sprintf-js` (via
  electron-builder), both packaging-time tooling.

### Needs a person

- **Release provisioning (Prompt 72, `docs/runbooks/release.md`).** Version, app icon, signing and notarization
  identities, Windows publisher, registry/namespace ownership, protected environments, six signing runners,
  HTTPS update feed and Ed25519 key, scoped update hosting; upload the pinned design snapshot bundle.
- **Cloud provisioning (Prompts 61–64, 71).** HTTPS account origin, SES sender, Polar product and webhook,
  Postgres/Redis, R2, WSS sync, room grants, and reviewed PostHog/Sentry delivery and retention.
- **Platforms and CI.** Windows/Linux lanes for every host and the daemon, M1 and mid-range Windows
  measurements, GPU and installed-engine runners, and the first remote runs of every workflow (layout oracle,
  types/format-spec checks, native IPC, renderer, release gates).
- **Human review.** Language review of the 13 catalogs, VoiceOver/NVDA and text-scaling passes, a live Figma
  paste through the importer, Pixabay's terms for personal keys, `crates/vellum/data/pricing.json`, and the
  built-in prompt pack text.
- **Design source.** Run `pen login` then `pnpm design:snapshot` once; decide whether to rewrite history to drop
  the 86 MB of committed snapshot PNGs; supply high-contrast and the 303 missing component appearance references;
  settle the six Component-library entries without a reusable component.
- **Share authority (Prompt 69).** Decide whether `vellum/share` needs its own host capability or approval;
  today any initialized local client shares that interface.
- **How far "agents manipulate anything" goes (ADR-034).** Host controls (selection, viewport, undo, version
  history, sharing), raw browser control and file writes from scripts act on the live editor or outside the canvas.
  Decide, for each one, whether an agent may take it and whether it needs a `PermissionBroker` prompt. Nothing of
  that kind is exposed to agents yet.
- **Clean-room record (Invariant 14).** While comparing Vellum's MCP with pen.dev's, an agent read the text of
  pen.dev's MCP skill (`read_skill`) before the rule that parity comes from public docs only was applied. Nothing
  was copied, and the comparison uses ARCHITECTURE §7.4 and the tool schemas. Decide whether that read needs to be
  recorded or re-checked.

## Prompt index

Full entries for every prompt below are in `addressed-reports.md` (archive).

## Prompt 1: monorepo scaffold

## Prompt 2: dependencies, reference repositories, copy checks

## Prompt 3: design source

## Prompt 4: Spike A, renderer (ADR-001)

## Prompt 5: Spike B, text parity, Loro at scale, QuickJS in WASM (ADR-004)

## Prompt 6: Spike C, Chrome layout oracle (ADR-002)

## Prompt 7: vellum-types I, nodes, paints, strokes, effects, sizing

## Prompt 8: vellum-types II, the document, variables, components, schema and TypeScript

## Prompt 9: JSON codec, deterministic bytes, quantization, migrations

## Prompt 10: Ports, fakes, contract suites

## Prompt 11: Ops I, structural ops, validation, origins

## Prompt 11: acceptance follow-up

## Prompt 12: component, slot and geometric ops

## Prompt 13: Guard, leases, pause set, limits

## Prompt 14: DocStore on Loro, per-actor undo, undo this turn

## Prompt 15: diff to ops, three-way merge, merge-driver core

## Prompt 16: resolver I, variables, theme axes, aliases

## Prompt 17: resolver II, components, instances, overrides, slots and libraries

## Prompt 18: production layout on Taffy and browser oracle

## Prompt 19: text on Parley and explicit font sources

## Prompt 20: scene, spatial index, hit-testing and snapping

## Prompt 21: layout check and display list

## Prompt 22: shared native/editor Skia renderer

## Prompt 23: renderer appearance, caches and Basic fallback

## Prompt 24: files, assets, custom fonts, libraries on disk, directory format, spec

## Prompt 25: QuickJS sandbox, limits and escape corpus

## Prompt 26: script nodes and document API

## Prompt 27: local IPC, lifecycle and stdio bridge

## Prompt 28: live/headless targets, file watcher, recovery copies

## Prompt 29: tool registry, MCP adapter, read tools

## Prompt 30: write tools, `execute`, batch errors, steps

## Prompt 31: skills, styles and parity inventory

## Prompt 32: model providers, secret stores and deterministic endpoint

## Prompt 33: built-in engine loop and context management

## Prompt 34: session store, usage and estimated cost, turn state machine

## Prompt 35: PermissionBroker

## Prompt 36: installed Claude Code engine

## Prompt 37: installed Codex engine

## Prompt 38: plans, placeholders, leases in orchestration

## Prompt 39: parallel (Split, Side by side, Explore), spawn_agents, Stop/Pause, Undo this turn

## Prompt 40: tokens, spec, stack profiles

## Prompt 41: HTML, raster, PDF and SVG exports

## Prompt 42: importers, icons, stock images (ADR-009)

## Prompt 43

## Prompt 44

## Prompt 45

## Prompt 46

## Prompt 47

## Prompt 48

## Prompt 49

## Prompt 50

## Prompt 51

## Prompt 52

## Prompt 53

## Prompt 54

## Prompt 55

## Prompt 56

## Prompt 57

## Prompt 58

## Prompt 59

## Prompt 60

## Prompt 61

## Prompt 61 follow-up — native account integration

## Prompt 61 follow-up — account protocol acceptance

## Prompt 62

## Prompt 63

## Prompt 64

## Prompt 65

## Prompt 66

## Prompt 67

## Prompt 66 follow-up — .pen import

## Prompt 68

## Prompt 69

## Prompt 70 — Accessibility, local languages and RTL (implementation; acceptance open)

## Prompt 71 — Performance and observability (implementation; budget acceptance open)

## Prompt 72 — CI/CD, release pipelines and release blockers (implementation; release blocked)


## 2026-10-22 follow-up (owner: "complete what is open, decide for me")

Built ADR-033 (live editor replicas, editor/viewer WASM modules), per-document daemon scene services,
annotation-free screenshots, pixel grid, Codex paragraph fix, exporter contrast fix and token re-export,
advisory fixes, script/Biome/clean-room fixes. Details and decisions: `addressed-reports.md`, ADR-033,
version-baseline C27/C28.

Passed: `cargo test -p vellum` (full; one expectation updated where notes became drawable), `-p vellum-wasm`,
`-p vellum-doc-loro`, `-p vellum-contracts`, `-p vellum-image-ops`, `-p vellum-engine-codex`, index-rstar scene;
workspace clippy `-D warnings`; `cargo fmt --check`; `cargo deny check`; `bash scripts/check-deps.sh`;
`pnpm type-check` (34); editor-core 2,396 and editor-react 316 unit tests; extension 45; desktop 29 + packaging;
host-api 32; `engine.spec.ts` 4 and `agent.spec.ts` 8 in Chrome; VS Code Insiders smoke with live editing;
Electron functional tests (10 of 10 non-design after fixes); script suite 237/0; token lint/check; copy-lint;
clean-room.

Not verified / not done: Electron `@design` goldens remain red against the 1% gate (pre-existing); both WASM modules were rebuilt on 2026-10-23 (editor 6.27 MB, viewer 1.90 MB gzipped) and `engine.spec.ts`
passes on them; feature ports of
Prompts 49–54 are still unserved by hosts; Windows/Linux and remote CI not run. Committed by the owner as
9c6b0c4 and f65eb81 (agent feed).

## 2026-10-23 follow-up

Fixed the cause of the flaky desktop folder test: canvas frames with shadows blocked the renderer for 1–5 s,
because filtered layers had no bounds and their filters ran over the whole viewport on the CPU. Filtered layers
are now bounded by their content (`render-skia` `content_bounds`); pixels are unchanged. Also fixed two
pre-existing test faults (desktop share test lacked the fixture's shader file; a Figma import locator matched
hidden text). Details: `addressed-reports.md`.

Passed: `cargo test -p vellum-render-skia`, `-p vellum-wasm`; clippy `-D warnings` on both; native renderer
goldens (`renderer-fixtures`, `renderer-goldens`, no recording); `node scripts/renderer-goldens.mjs` and
`--software` (no failures); `pnpm --filter @vellum/renderer-skia test`; editor-react `engine`, `resources` and
the Figma progress spec; Electron non-design suite 10 of 10 (folder test 5 of 5 in a row).

Not done / still red (pre-existing, unchanged without this fix): Electron `@design` A01–A06 goldens; editor-react
C30/C31 and C33 design comparisons. WASM rebuilt: editor module 6.32 MB gzipped (gate 6.5 MiB). `pnpm clean`
afterwards (target was 54 GB; free space 355 → 407 GiB). Not committed.
