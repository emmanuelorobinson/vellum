# Reports

What a person should know after each prompt: what was built, what was decided on the way, what was skipped or
could not be verified, and what only a human can do. AGENTS.md tells every session to read this file first and
to update it at the end of each prompt. Newest prompt last. Keep entries short and factual; say what failed.

## Needs a person

Current open items only. Resolved items, decisions taken for the owner and every earlier prompt entry are in
`addressed-reports.md`. Remove an item here when it is done and add it there.

### Engineering still open (an agent can do these)

- **Docs and icon checks (2026-10-23).** The brand-token check reads Markdown only, so a stray `{{brand}}` in an
  `app/*.tsx` string got through once (fixed by hand); extend the check to TSX. The extension build needs
  `@vellum/renderer-skia`'s `dist/vellum-editor.js` from the wasm pipeline, which this checkout has not built.
  `apps/web` and `apps/account` were type-checked on the new Next pin, not built. Root `pnpm exec biome check .`
  fails on six `packages/editor-core` files that are unchanged from HEAD.
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
- **Pan stalls on effect-heavy pages (2026-10-24).** Reproduced on `fixtures/documents/shapes.vellum`: a fast pan gave
  frame gaps of up to 2.1 s (every step re-rasterised the viewport, ADR-017). `CanvasHost` now moves the drawn pixels
  with a CSS transform while a heavy page (draw over 24 ms) is panned or zoomed, redraws 120 ms after the motion stops
  and at most every 400 ms (or twice the draw time) during it; worst gap fell to 25 ms and the settled frame is
  pixel-identical to the old path. Still open: the one redraw after you stop still blocks for as long as the draw takes
  (about 2 s here); the cure is GPU-native effects. The transform is not applied to the selection overlay, so
  handles briefly lead the content during a fast pan.
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

- **Design guidance and pictures (2026-10-24).** Vellum's agents were told how to call tools but not how to design:
  the built-in skills were API notes only, installed engines (Codex, Claude Code) get a one-line base instruction,
  and no agent can fetch a picture (`set_stock` is called only by tests, and installed engines have no network), so
  boards get hand-drawn vector faces. Added `design-craft.md` (own wording; please review it) and a directive on
  desktop turns to read it. Decide the picture source: a stock key in the keychain, a configured generation model, or
  attach-from-repo through the PermissionBroker; and whether to bundle visual styles beyond the six generic ones.
- **Design differences made on request (2026-10-24).** The engine and model picker is now a chip in the composer
  footer (C01, C08, C26, C27 draw it as a text line under the composer; the `engine line` golden region is removed
  from `screens.json`). Welcome (A01) puts the prompt under the hero, not at the bottom; Home (A02) uses a quiet
  dot-grid tile instead of a "Preview unavailable" message; the desktop document bar is only as wide as its tabs for
  one or two tabs (A03 draws three). Update `vellum.pen` or reject any of these.
- **Docs site and icons (2026-10-23).** Approve the icon as drafted (the brand mark on a white tile; see the last
  entry) or choose another container. Decide the Activity Bar icon (the Lucide frame glyph stays for now), whether the
  editor gets a logo, attribution for `apps/docs/lib/content-cache.mjs` (adapted from the Outpost project, which has no
  licence file), the docs site's hosting, public address and versioning, and whether `pnpm install` may run the five
  ignored build scripts (`@vscode/vsce-sign`, `electron-winstaller`, `esbuild`, `keytar`, `msgpackr-extract`) through
  `pnpm approve-builds` or `allowBuilds`. The installs here used `--config.strict-dep-builds=false`; CI runs
  `pnpm install --frozen-lockfile`, and a fresh CI install was not tested.
- **Release provisioning (Prompt 72, `docs/runbooks/release.md`).** Version, app icon sign-off (drafted 2026-10-23),
  signing and notarization
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

## 2026-10-23 documentation site and brand icons

Built the documentation site in `apps/docs` (Fumadocs, ADR-035): 71 hand-written pages (4,056 lines) and 92 pages
generated at build time from the runbooks, decision records, the MCP tool snapshot, the format spec and the keyboard
table. Search, `/llms.txt`, light and dark appearance and the brand token work. Outpost has 64 content pages; this is
comparable in size, not a page-for-page copy.

The logo from the docs header is now the app icon. `scripts/brand-icons.mjs` (macOS: `sips`, `iconutil`) reads the
colours from `packages/ui-tokens`, writes `packages/config/assets/{mark,app-icon}.svg`, and writes: favicons, `icon.svg`
and apple icons for `apps/docs`, `apps/web` and `apps/account`; `apps/desktop/build/icon.{icns,ico,png}`; and
`apps/extension/media/icon.png` with `"icon"` in the extension manifest. The desktop development run sets the window and
macOS dock icon. Electron-builder reads `build/icon.*` by default; a packaged build was not run. The icon is the mark on
a white tile with a light border (dark mode uses the dark panel), so it reads on a dark dock and a dark tab bar. The
docs site had a 404 for `favicon.ico`; it is served now.

Pins decided on the way: Next.js 16.4.0 → 16.4.2 (three advisories fixed in 16.4.1; `pnpm audit` reports none now; the
account and web apps share the pin). Fumadocs 16.16.2 for core and UI; `fumadocs-mdx` 15.4.6, because 15.4.7 and later
need `fumadocs-core` 16.17; `@types/mdx` 2.0.14; Tailwind 4.3.3 in `apps/docs` only. The root Biome configuration
enables Tailwind directives. `version-baseline.md` records each pin.

Fixed on the way: the home page showed the raw `{{brand}}` token in one card title, because the brand check covers
Markdown only. The title now reads `brand.name`. Earlier browser state had moved the test session between pages; a
fresh session stays on the home page.

Not changed on purpose: the Activity Bar icon. Its code says it is the frame glyph, not a brand logo. The mark's two
shapes overlap, so one colour merges them; a knockout gap is the likely next step. The editor's top bar has no logo.

Passed (exit 0): `pnpm --filter @vellum/docs build`, `… test` (16 node tests and vitest), `… type-check`; `pnpm exec
biome check apps/docs`; `pnpm peers check`; `pnpm audit` (no Next.js advisory); `pnpm --filter ./apps/desktop
type-check` and `… test` (30 vitest tests and the packaging test); `pnpm --filter ./apps/extension type-check` and
`… test` (41 vitest tests and the packaging test); `pnpm --filter @vellum/web type-check`; `pnpm --filter
@vellum/account type-check`; `node scripts/brand-icons.mjs`; `pnpm checksum`; `pnpm clean-room`; `pnpm copy-lint`;
`pnpm tokens:lint`; `node --test scripts/__tests__/*.test.mjs`; `node --test scripts/__tests__/version-baseline.test.mjs
scripts/__tests__/architecture-records.test.mjs`.

Browser (Playwright, production build on 127.0.0.1:3002): the home page and a docs page in light and dark appearance;
search for "keyboard" returns 30 results with breadcrumbs; no horizontal overflow at 390 px; no console errors on the home
page. Not checked: Safari and Firefox, the mobile contents bar, the account and web apps in a browser.

Not verified: `pnpm --filter ./apps/extension build` fails here because `@vellum/renderer-skia` has no
`dist/vellum-editor.js` (the wasm pipeline is not built). The icon copy line in `apps/extension/build.mjs` is therefore
not run end to end. The Electron dev run and `pnpm package:desktop` were not run. Root `cargo clippy` was not run (its
failure at HEAD is recorded above).

## Prompt 72 follow-up: runners dating-app comparison (2026-10-24)

Used computer UI control to submit an original four-screen runners dating-app brief in a new pen.dev draft;
generation started with its existing model connection. No repository design file was opened or modified.
Vellum could not be opened by app name: no installed app, desktop bundle or renderer WASM build was found.
Confirmed `apps/desktop/src/renderer/Home.tsx` disables the design composer and Start designing control;
the requested comparison is blocked by the existing host integration gap above. No implementation, builds or
tests were run. No build output was created or cleared; `target/` was absent and disk availability was 372 GiB.

## Prompt 72 follow-up: build and try the desktop (2026-10-24)

Built the native daemon, editor WASM and unsigned macOS ARM64 app. Computer-use smoke test found a packaging
bug: `engine/` was omitted from both staging and the archive, so a new draft could not start its engine.
Added a failing runtime-asset regression test, then included the engine, Inter font and licence in packaging.
After rebuilding and restarting, the same draft opens the canvas. Home's composer/Start designing remain
disabled; Agents says provider/engine controls are unavailable; the draft shows an unavailable agent feed,
read-only/daemon-offline state, and New session returns unavailable. No runners-app prompt could be submitted
in Vellum and no ChatGPT connection was made. The existing host integration item remains open.

Passed: `node scripts/skia-build.mjs build -p vellum`;
`node scripts/skia-build.mjs build --release -p vellum-wasm --bin vellum-editor --target wasm32-unknown-emscripten`;
`node apps/desktop/packaging.mjs --platform darwin --arch arm64 --daemon target/debug/vellum --development-preview --dir`;
`node --test apps/desktop/packaging.test.mjs` (5 tests);
`node_modules/.bin/biome check apps/desktop/packaging.mjs apps/desktop/packaging.test.mjs`;
`apps/desktop/node_modules/.bin/tsc --noEmit -p apps/desktop/tsconfig.json`;
`bash scripts/check-deps.sh ts`; `pnpm checksum`; `node scripts/spec-checksums.mjs check`; `git diff --check`.
Rust logic was unchanged; clippy was not rerun. Native linking warned about the unwind table size.
Initial WASM linking and packaging failed on sandbox cache/network access; retries with access passed.
Sandboxed pnpm could not verify its registry signature; the network-enabled checksum invocation passed.

Reviewed cleanup with `node scripts/clean.mjs --dry-run`. Staged unused Rust and desktop/WASM intermediates
under `/tmp/vellum-build-clean-TRtvPl`, then ran
`pnpm run clean -- --root /tmp/vellum-build-clean-TRtvPl` (3 output directories removed).
Retained only `target/desktop-packages/` (611 MiB) because the built app is running there. Disk availability
rose from 364 GiB before cleanup to 370 GiB after; source, dependencies, references and design cache retained.

## Follow-up 2026-10-24: desktop chrome, presence, picker, craft skill, pan stalls

Built: compact document bar (`apps/desktop` `style.css`, `Tabs.tsx`); composer footer model chip and a compact
Engine and model menu with each installed engine's models and reasoning effort (`EnginePicker`, `composer.css`;
models come from `~/.codex/models_cache.json` and Claude Code's documented aliases, `agent-runs.ts`);
`createLivePresence` (editor-core) derives agents, leases and a stepping cursor from the running turn and the nodes it
changes, wired into the desktop editor; cursor glide, working pill, lease glow and a working row in the Agent tab;
Home and Welcome restyled; `design-craft.md` registered in `crates/vellum/src/content/skills.rs` (tests count 8);
`CanvasHost` adaptive pan.

Differs from the design: see "Design differences made on request" above.

Failed or not verified: Electron `@design` goldens A01–A06 and A03 tabs fail at HEAD and after (A01 0.026 to 0.032,
A02 0.041 to 0.047, A04 0.027 unchanged; gate 0.01); A03 reads 908 px on macOS because the 88 px traffic-light padding
is added to a content-box width. `@electron account device flow` did not open a window in two runs and was not
diagnosed. `design.spec.ts` "Shader tab says it arrives in v1.1" is stale (the string no longer exists). The cursor
and presence were checked on scripted scenarios and a unit test, not against a live Codex or Claude Code turn. Model
choice was exercised in the UI, not through a real send. `cargo fmt --check` differs in `mcp/cli.rs` (not touched).

Passed: `vitest` in editor-core (2405), editor-react (1362), apps/desktop (33); `tsc --noEmit` in the three;
`pnpm tokens:lint`, `pnpm copy-lint`; `biome check --diagnostic-level=error`; Playwright composer, agent,
collaboration, accessibility, editor, engine, design, shell specs (179 of 180; the one failure is the stale test above);
`cargo test -p vellum --test skills_styles --test cli` (23).

Cleared: this session's probe profiles in the system temp folder. Retained `target/` (running app in
`target/desktop-packages/`); `pnpm clean` was not run for that reason.

