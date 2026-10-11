# Vellum — Canonical Implementation Guide & Complete TDD Build Prompts

> **Product:** Vellum (working codename; rename in `@vellum/config` and `vellum-types::brand` only)
> **Canonical tagline:** *Design where your code lives.*
> **Document status:** Canonical implementation source of truth
> **Version snapshot:** 2026-10-11
> **Build philosophy:** Spine-first, one-write-path, file-is-truth, agents-are-actors, clean-room
> **Implementation mode:** One coding-agent session per prompt
> **Design source:** `docs/design/vellum.pen` (pen.dev), read by agents through the Pencil MCP (§2.1)
> **Reference code:** Penpot, Graphite, Loro, Taffy, Parley, Codex, Goose, OpenCode and others are cloned read-only under `refs/` and are expected to be read (§3)

---

# 1. How To Use This Document

Run the prompts in order. There is no fixed number of prompts: each is sized so one coding-agent
session can finish it with a reviewable diff. If a prompt turns out too big, split it into `NNa`,
`NNb` and record the split in the PR; if a new need appears, add a prompt with the next free
number in the right phase and update §1's list in the same PR.

Each prompt is scoped so a coding agent can:

1. Read the named product, design and architecture sources.
2. For UI prompts, read the named frames from the design file **through the Pencil MCP** (§2.1).
3. Read the named **reference-repo paths** for the hard parts (§3).
4. Add failing tests.
5. Confirm the failures.
6. Implement the smallest coherent change.
7. Re-run focused tests.
8. Run type-check / clippy / lint / dependency rules for affected packages.
9. Produce a reviewable diff and, when a prompt says so, an ADR.

Do **not** skip ahead within a phase. Later phases assume the types, ports, op layer, renderer
decision, daemon protocol and agent runner created earlier.

```text
PHASE 0 — Foundation
Prompt  1 -> Monorepo scaffold, crate/package shells, AGENTS.md, CI skeleton
Prompt  2 -> Reference repos, reference index, version baseline, dependency + clean-room checks, ADR-000
Prompt  3 -> Design source: Pencil MCP access, read-only copy, frame inventory, design snapshots, decisions file

PHASE 1 — Spikes (decide before building)
Prompt  4 -> Spike A: renderer (CanvasKit in TS vs skia-safe in WASM) → ADR-001
Prompt  5 -> Spike B: text parity, Loro at scale, QuickJS in WASM → ADR-004
Prompt  6 -> Spike C: Chrome layout oracle → ADR-002

PHASE 2 — Types and the write path
Prompt  7 -> vellum-types I: nodes, paints, strokes, effects, sizing
Prompt  8 -> vellum-types II: variables, themes, components, libraries, scripts; schema + ts-rs
Prompt  9 -> JSON codec: deterministic bytes, quantization, migrations
Prompt 10 -> Ports, fakes, contract suites
Prompt 11 -> Ops I: structural ops, validation, origins
Prompt 12 -> Ops II: component, slot, scale, boolean, align, flip ops
Prompt 13 -> Guard: leases, pause set, limits
Prompt 14 -> DocStore on Loro: transactions, per-actor undo, undo-turn
Prompt 15 -> Diff → ops, three-way merge, merge-driver core

PHASE 3 — Engine
Prompt 16 -> Resolver I: variables, theme axes, aliases
Prompt 17 -> Resolver II: components, instances, overrides, slots, libraries
Prompt 18 -> Layout on Taffy + the oracle suite
Prompt 19 -> Text on Parley + FontSource
Prompt 20 -> Scene: spatial index, hit-testing, snapping
Prompt 21 -> Layout check + display list
Prompt 22 -> Renderer I: editor + native path, solids, strokes, text, images
Prompt 23 -> Renderer II: gradients, mesh, blend modes, effects, caches, canvas2d, golden suite
Prompt 24 -> Files: assets, custom fonts, libraries on disk, directory codec, atomic writes, format spec
Prompt 25 -> ScriptRuntime: QuickJS sandbox + escape corpus → ADR-005
Prompt 26 -> Script nodes + the `execute` doc API

PHASE 4 — Daemon, tools and agents
Prompt 27 -> Daemon I: local IPC, lifecycle, `vellum mcp` stdio bridge → ADR-003
Prompt 28 -> Daemon II: live/headless targets, file watcher → ops, recovery copies
Prompt 29 -> Tool registry, MCP adapter, read tools
Prompt 30 -> Write tools, `execute` tool, batch errors
Prompt 31 -> Skills and styles content, read_skill/get_style, parity-suite harness
Prompt 32 -> ModelProviders, SecretStore, stub model endpoint
Prompt 33 -> Built-in engine loop, context management
Prompt 34 -> Session store, usage and estimated cost, turn state machine → ADR-006
Prompt 35 -> PermissionBroker
Prompt 36 -> Installed engine: Claude Code → ADR-007
Prompt 37 -> Installed engine: Codex
Prompt 38 -> Plans, placeholders, leases in orchestration
Prompt 39 -> Parallel (Split, Side by side, Explore), spawn_agents, Stop/Pause, Undo this turn

PHASE 5 — Import and export
Prompt 40 -> Tokens, spec, stack profiles
Prompt 41 -> HTML export, raster/PDF/SVG export
Prompt 42 -> Importers (SVG, image, Figma paste), icons, stock images → ADR-009

PHASE 6 — Editor UI (design comes from the .pen file via the Pencil MCP)
Prompt 43 -> Design tokens generated from the .pen variables, contrast and usage lints
Prompt 44 -> editor-core I: tool state machines
Prompt 45 -> editor-core II: commands, keybindings, registries, Host/EngineClient ports, visual harness
Prompt 46 -> UI: floating chrome, canvas interaction, Layers and Assets (C18, S13)
Prompt 47 -> UI: properties, fills/strokes/effects, text, components (C13, C14, C16, C33)
Prompt 48 -> UI: variables and themes, libraries, fonts, scale, vector editing (C15, C17, C34, C36–C38)
Prompt 49 -> UI: Agent panel shell, sessions, turns, Step Timeline (C03, C06, C09)
Prompt 50 -> UI: composer, skills menu, model picker, usage, Permission card, inline prompt (C01, C08, C11, C26, C27)
Prompt 51 -> UI: plans, parallel, alternatives, presence layer, collaboration nodes (C04, C05, C07, C35)
Prompt 52 -> UI: Notes for agents, guidelines, script nodes, slots, image fill (C12, C28–C30)
Prompt 53 -> UI: History, Inspect, token sync, export, file changes (C10, C19–C22)
Prompt 54 -> UI: palette, context menu, settings, edge states (C23–C25, S01–S16)

PHASE 7 — Hosts and CLI
Prompt 55 -> VS Code extension I: custom editor, save/revert/backup, commands
Prompt 56 -> VS Code extension II: Connect agents (C02), sidebar, packaging and publishing
Prompt 57 -> Desktop I: Electron shell, windows, tabs, menus, security (A03)
Prompt 58 -> Desktop II: welcome, home, drafts, recovery, agents, notifications, updater, packaging (A01, A02, A04–A06)
Prompt 59 -> CLI completeness and the interactive shell
Prompt 60 -> `vellum diff`, PR preview Action, merge-driver install

PHASE 8 — Cloud (Pro)
Prompt 61 -> Accounts, billing, entitlements, device login
Prompt 62 -> Share snapshots and the web viewer (W01–W03)

PHASE 9 — v1.1
Prompt 63 -> Sync server
Prompt 64 -> Multiplayer presence, comments, self-host image
Prompt 65 -> BrowserPort, `browser` tool, html importer, embed node (C31)
Prompt 66 -> Figma file import, image ops, image generation
Prompt 67 -> Shaders and the examples gallery → ADR-008
Prompt 68 -> Slides and presenting (C32)

PHASE 10 — Release
Prompt 69 -> Security hardening, threat model, fuzzing
Prompt 70 -> Accessibility, i18n and RTL
Prompt 71 -> Performance and observability
Prompt 72 -> CI/CD, release pipelines, release blockers
```

The project is ready for public beta only when Prompt 72's release-blocker list is all false.
Phase 9 is v1.1; beta does not wait for it, but Prompts 69–72 re-run after each v1.1 prompt.

**Parallel lanes.** Within Phase 3, Prompts 18–19 and 20–21 can run in parallel after 17. Phase 5
can run in parallel with Phase 4 after Prompt 30. In Phase 6, Prompts 46–48 and 49–52 are two
parallel lanes after 45, and 53–54 follow both. Phase 7 can start after Prompt 46. Nothing before
Prompt 43 produces user-facing UI, except the spike pages in Prompts 4–6.

---

# 2. Source Hierarchy

The implementation respects this priority:

```text
1. docs/product/PRODUCT.md                         (the why and the what)
2. docs/design/DESIGN.md                           (the visual and writing law: intent, states, copy)
3. docs/design/vellum.pen                          (the pixels: every screen, light / dark / high contrast,
                                                    components and tokens — read through the Pencil MCP, §2.1)
4. docs/design/snapshots/                          (PNG exports + inventory of vellum.pen; Prompt 3)
5. docs/design/DESIGN-DECISIONS.md                 (which option was chosen where the brief offers options)
6. docs/architecture/ARCHITECTURE.md               (system structure, ports, tools, parity table §7.4)
7. docs/architecture/decisions/ADR-*.md            (evidence-backed amendments to 6)
8. docs/format/                                    (the published .vellum spec, generated from types)
9. this implementation guide
10. tests
11. implementation code
```

Reference repositories under `refs/` are **not** in this hierarchy. They are implementation
reference only. They never decide product behavior, copy, or visual design.

If a frame in `vellum.pen` and `DESIGN.md` disagree, `DESIGN.md` wins on intent, states and
**copy** (Appendix B is checked against `DESIGN.md`, not the design file), and `vellum.pen` wins on
pixels: layout, spacing, sizes, colours and component anatomy. Record every disagreement in
`docs/design/DESIGN-GAPS.md` (one line each: frame, what differs, which source was followed) so a
human can fix the design file. If both are silent, ask; do not improvise brand decisions.

Where this guide records a **verified delta** from `ARCHITECTURE.md` (§6.1), the delta wins and
Prompt 2 records it as an ADR so the architecture document can be updated by a human.

Repository layout for documentation:

```text
docs/
  product/
    PRODUCT.md
  design/
    DESIGN.md
    vellum.pen                     the design file (pen.dev), edited only by a human in pen.dev
    vellum.pen.assets/             images and fonts the design file references (if any)
    snapshots/                     generated by scripts/design-snapshot (Prompt 3); never hand-edited
      inventory.json               frame id, screen id, theme, width, page, bounds, source hash
      C03-agent-building.light.1280.png ...
    DESIGN-DECISIONS.md            (created in Prompt 3 from the template in Appendix G)
    DESIGN-GAPS.md                 (append-only log of design file vs brief mismatches)
  architecture/
    ARCHITECTURE.md
    version-baseline.md
    decisions/ADR-000-deltas.md ...
  format/                          (generated spec + JSON Schema; Prompts 8 and 24)
  reference/
    REFERENCE-INDEX.md             (generated in Prompt 2 from §3)
  runbooks/
    design-mcp.md                  (how to open the design and connect the Pencil MCP; Prompt 3)
```

These files are **read-only specification**. Never modify `docs/product/`, `docs/design/DESIGN.md`
or `docs/design/vellum.pen` during implementation. `docs/design/snapshots/` and
`docs/format/` are only ever regenerated. `docs/architecture/` is modified only by adding ADRs and
the version baseline. `DESIGN-GAPS.md` is append-only.

## 2.1 Reading the design through the Pencil MCP

The visual design lives in `docs/design/vellum.pen`, made in pen.dev (formerly Pencil). Coding
agents read it through **pen.dev's MCP server** (it appears as `pencil` in the client's MCP list),
the same way a designer would hand over a Figma file — except the agent can query it.

**Setup (once per machine; documented in `docs/runbooks/design-mcp.md` by Prompt 3)**

1. Install the pen.dev desktop app (or the pen.dev IDE extension) and sign in.
2. In pen.dev **Settings → MCP**, enable the integration for the coding agent in use (Claude Code
   CLI, Codex CLI, …). pen.dev writes the client's MCP config entry.
3. Run `scripts/design-open` — it copies `docs/design/vellum.pen` (and its assets) to
   `.design-cache/vellum.readonly.pen` and opens **that copy** in pen.dev. Agents always work on the
   copy, so an accidental write can never touch the spec.
4. Restart the coding agent and confirm the `pencil` server is connected.

**Rules for every session that reads the design**

1. **Confirm the document first.** Call `get_app_state` and check the active document is
   `.design-cache/vellum.readonly.pen`. If not, stop and ask.
2. **Learn the current API, don't assume it.** Call `read_skill` once per session (and
   `read_skill` with the `execute` page) before using `execute`. Never hardcode pen.dev's script
   syntax in our code or docs; it may change between pen.dev versions.
3. **Read only.** Use `get_app_state`, `read_skill`, `get_style` and **read-only** `execute`
   scripts (find frames by name, read node trees and properties, read variables, take screenshots,
   export images). Never call an `execute` operation that inserts, updates, moves, deletes or
   replaces anything, never call `spawn_agents`, never save. The copy is discarded after the
   session; the design-file checksum test (Prompt 3) fails the build if the original changed.
4. **Ask for frames by screen ID.** Frames follow the naming convention below. Read the frames the
   prompt names and the components they use — never dump the whole document into context.
5. **Screenshot to compare.** For every screen a prompt builds, take a screenshot of the design
   frame and of the implementation at the same size and theme, and compare. The committed
   snapshots in `docs/design/snapshots/` are what CI compares against; the live MCP screenshot is
   for the agent's own iteration.
6. **Values come from variables and components, not from eyeballing.** Read colours, spacing,
   radii and type from the `.pen` variables and component definitions; those become tokens and
   component specs (Prompt 43). A hardcoded hex in UI code is a lint failure.
7. **pen.dev's HTML export is a reference, never product code.** An agent may export a frame as
   HTML to understand its structure, but must not paste generated markup or class lists into the
   product.
8. **If the MCP is unavailable** (app closed, not signed in, CI), use `docs/design/snapshots/` and
   `inventory.json` instead and say so in the PR. UI prompts must not start without at least the
   snapshots.

**Design file conventions** (the human who maintains `vellum.pen` follows these; Prompt 3's
inventory check enforces them)

```text
Pages        Tokens · Components · Editor Agent · Editor Canvas · Editor Code and Files ·
             Desktop and Web · Edge States
Frames       "<ScreenID> <Title> / <Theme> / <Width>"     e.g. "C03 Agent workspace / Dark / 1280"
             Themes: Light, Dark, High contrast. Widths: 1280 (default), 1512, 960, 760 where DESIGN.md asks.
Components   reusable components named exactly as DESIGN.md's Component library
             ("Agent Panel / Step row", "Composer / Send-Stop", "Actor Marker / Agent / 24") with
             variants for states
Variables    one collection per token group (chrome, canvas, agent hues, type, space, radius, motion),
             theme axis "Mode" with values Light, Dark, High contrast — names match ui-tokens names
             ("chrome/panel", "canvas/selection", "agent/teal", "space/4", "radius/lg")
Copy         final copy from DESIGN.md; placeholders in braces ("{agent}") where DESIGN.md has them
```

**pen.dev is the design tool, not a reference product.** Using it to draw Vellum's own screens is
fine. The clean-room rule (Invariant 14) still applies: Vellum's design must not reproduce
pen.dev's own interface, and nothing from pen.dev's app, extension or MCP internals is copied into
the product.

---

# 3. Reference Repositories (Read Them — Don't Reinvent)

Several open-source projects already solved many of Vellum's hardest problems. Prompt 2 clones
them read-only; every later prompt names the exact paths to study first. All paths below were
verified to exist at the pinned commits on 2026-10-11.

```text
refs/penpot        https://github.com/penpot/penpot                      10955f17e27943429a10e90360d0404e3f56e27c  MPL-2.0      READ ONLY, never port
refs/graphite      https://github.com/GraphiteEditor/Graphite            7477cd2fab315494e4c2613bc446c707733e6e7b  Apache-2.0/MIT
refs/loro          https://github.com/loro-dev/loro                      4f60d921d820de3522624a436de77f994a6622b8  MIT
refs/taffy         https://github.com/DioxusLabs/taffy                   7d33901cc383489d9945ae796a1ceed0a935ef66  MIT
refs/parley        https://github.com/linebender/parley                  46fcad316d4ff05dc0d5647fda2d15def7be896e  Apache-2.0/MIT
refs/vello         https://github.com/linebender/vello                   235739c4f08eb48d900e1512459d453a9ab77f96  Apache-2.0/MIT
refs/kurbo         https://github.com/linebender/kurbo                   9366cda80ebb56f41e5d09509a89dbfe81bc4f5b  Apache-2.0/MIT
refs/codex         https://github.com/openai/codex                       8c0f7011674145da7e10627769fb221c74fbf7a0  Apache-2.0
refs/goose         https://github.com/block/goose                        3bd852002903e016ff30947e973f76e2fcfcf90f  Apache-2.0
refs/opencode      https://github.com/sst/opencode                       d21b0790a6c146815f28fea604e41e922f009590  MIT
refs/rquickjs      https://github.com/DelSkayn/rquickjs                  a4e45effc347d63c58c2efc3020566e6095cc94f  MIT
refs/rmcp          https://github.com/modelcontextprotocol/rust-sdk      c1a8eea312613519b93f53b89a1dc74dc993f5b0  Apache-2.0 (MIT legacy parts)
refs/vscode-samples https://github.com/microsoft/vscode-extension-samples 65c1c44800eeac6fb065a54823ee5ae62c79540a MIT
refs/excalidraw    https://github.com/excalidraw/excalidraw              e65eca999e071503efb893d3379a29fb96ff37c4  MIT
refs/tldraw        https://github.com/tldraw/tldraw                      7937be360bcb8239d89e420850843f81313fc07b  tldraw license  READ DOCS ONLY, never port
```

At scaffold time, re-pin each to the **newest release tag** at or after these commits (or keep the
commit where a project has no tags) and record the SHAs in `docs/architecture/version-baseline.md`.

## 3.1 Rules for using reference code

1. `refs/` is `.gitignore`d, cloned by `scripts/refs.sh` (blob-less, shallow), and never modified.
2. **Read, then decide.** Before implementing any row in §3.2, read the named paths and write a
   3–5 line note in the PR description: what the reference does, what Vellum keeps, what it
   changes and why.
3. **Porting is allowed with attribution only from MIT or Apache-2.0 projects.** Ported code goes
   in a file whose header names the source repo, path and commit, and the project is added to
   `THIRD_PARTY_NOTICES.md`.
4. **Penpot is MPL-2.0: read it, never copy from it.** Its design decisions are fair to learn from;
   its code stays out of this repo so licensing stays simple.
5. **tldraw is under a commercial licence: read its public docs and architecture for ideas only.**
   Never copy code, never add it as a dependency.
6. **pen.dev is our design tool and the product we match — never a code reference.** Agents use
   it only through its MCP to read `vellum.pen` (§2.1). Never decompile, inspect or copy its
   binaries, extension bundles, MCP internals, prompts, skills text, API wording or UI assets.
   Capability parity comes from its public docs only (ARCHITECTURE §7.4).
7. **Reference docs beat reference code for intent.** Read the project's `AGENTS.md`, `docs/` or
   README first; code second.
8. **Large repos:** use `rg` with the paths below; never load whole directories into context.

## 3.2 Hard problem → where it is already solved

| # | Vellum hard problem | Study | Used in |
|---|---|---|---|
| R1 | A Rust renderer compiled to WASM and driven from a web UI | penpot `render-wasm/docs/rendering_architecture.md`, `tile_rendering.md`, `serialization.md`, `render-wasm/src/render.rs`, `src/render/{surfaces,shape_renderer,fills,strokes,shadows,filters,text}.rs`, `src/tiles.rs`; graphite `frontend/wrapper/src/{editor_wrapper,editor_commands,wasm_value}.rs` | P4, P22, P45 |
| R2 | Text layout, rich text, editing and font fallback | parley `parley/src/layout/`, `parley/src/editing/{editor,cursor,selection}.rs`, `fontique/src/{fallback,matching,source_cache}.rs`, `examples/editor/src/main.rs`; penpot `render-wasm/docs/texts.md`, `docs/text_editor.md` | P5, P19, P47 |
| R3 | Flexbox/grid that matches the browser | taffy `src/compute/flexbox.rs`, `src/compute/grid/`, `docs/style-properties.md`; **Chrome-generated fixtures**: `scripts/gentest/` (`src/main.rs`, `test_helper.js`, `test_base_style.css`), `test_fixtures/` | P6, P18 |
| R4 | Tree CRDT with moves, undo per peer, compact sync | loro `context/tree-events.md`, `context/loro-js-tree-diff.md`, `context/tree-checkout-window.md`, `crates/loro-internal/src/undo.rs`, `src/handler/tree.rs`, `src/state/tree_state.rs`, `docs/encoding.md`, `docs/shallow-snapshot-concurrency.md` | P5, P14, P63 |
| R5 | Editor tool state machines, input mapping, snapping, transform cages | graphite `editor/src/messages/tool/tool_messages/{select,pen,path,shape,text,gradient}_tool.rs`, `tool/common_functionality/{snapping,transformation_cage,resize,shape_editor}.rs`, `input_mapper/input_mappings.rs`, `website/content/volunteer/guide/codebase-overview/editor-structure.md` | P44, P46 |
| R6 | Document history and diffs | graphite `editor/src/messages/portfolio/document/{document_history,document_diff}.rs`; excalidraw `packages/excalidraw/history.ts`, `packages/element/src/{store,delta}.ts` | P14, P15, P53 |
| R7 | Hit-testing, bounds, static vs interactive canvas layers | excalidraw `packages/element/src/{collision,bounds}.ts`, `packages/excalidraw/renderer/{staticScene,interactiveScene,renderSnaps}.ts`, `scene/Renderer.ts`; tldraw (docs only) editor managers concept | P20, P46 |
| R8 | An MCP server that lets agents run code against a design | penpot `mcp/packages/server/src/tools/{ExecuteCodeTool,HighLevelOverviewTool,ExportShapeTool,PenpotApiInfoTool}.ts`, `mcp/packages/server/data/{base_instructions,initial_instructions}.md`, `docs/mcp/{good-prompting-practices-design,prompting-token-aware,design-file-structure-best-practices}.md` (read for ideas; write ours) | P26, P29, P31 |
| R9 | "Code mode": agents writing programs instead of many tool calls | codex `codex-rs/code-mode/src/lib.rs`, `codex-rs/code-mode-runtime/src/cell_actor/`; opencode `packages/codemode/{README.md,src/codemode.ts,src/tool-runtime.ts}` | P25, P26, P30 |
| R10 | Sandboxed JS with limits, in WASM too | rquickjs `core/src/runtime/{base,raw}.rs` (memory limit, interrupt handler), `core/src/context/builder.rs`, `wasm-test/` | P5, P25 |
| R11 | Hosting an MCP server in Rust | rmcp `crates/rmcp/README.md`, `examples/servers/src/{counter_stdio,structured_output,progress_demo,elicitation_stdio}.rs` | P29 |
| R12 | stdio MCP bridged to a local socket daemon | codex `codex-rs/stdio-to-uds/{README.md,src/lib.rs}`, `codex-rs/uds/src/lib.rs` | P27 |
| R13 | An agent loop over many providers, context management | goose `crates/goose/src/agents/{agent,reply_parts,prompt_manager,large_response_handler}.rs`, `context_mgmt/mod.rs`, `providers/{base,anthropic_def,openai_def,openrouter_def,bedrock,gcpvertexai}.rs`; opencode `packages/ai/src/{provider,providers}.ts` | P33 |
| R14 | Driving installed agents (Claude Code, Codex, Cursor, Gemini CLI) headlessly | goose `crates/goose/src/providers/{claude_code,codex,codex_acp,cursor_agent,gemini_cli}.rs`; codex `codex-rs/app-server/README.md`, `codex-rs/app-server-protocol/src/protocol/` | P36, P37 |
| R15 | Permission prompts for files and commands | goose `crates/goose/src/permission/{permission_inspector,permission_judge,permission_store}.rs`; opencode `packages/core/src/permission.ts`, `permission/saved.ts`; codex `docs/sandbox.md`, `codex-rs/execpolicy/README.md` | P35 |
| R16 | Sessions on disk, resumable, searchable | codex `codex-rs/rollout/src/lib.rs`, `codex-rs/thread-store/README.md`; goose `crates/goose/src/session/{session_manager,session_naming}.rs` | P34 |
| R17 | Skills: SKILL.md discovery and loading | codex `docs/skills.md`, `codex-rs/skills/src/lib.rs` | P31 |
| R18 | Secrets in the OS keychain | codex `codex-rs/keyring-store/src/lib.rs` | P32 |
| R19 | VS Code custom editor for a binary-ish document with undo/save | vscode-samples `custom-editor-sample/src/{pawDrawEditor,catScratchEditor,extension}.ts`, `media/pawDraw.js`; `mcp-extension-sample/src/extension.ts` (registering an MCP server from an extension) | P55, P56 |
| R20 | Renderer research and CPU fallback | vello `ARCHITECTURE.md`, `vello_cpu/`, `vello_common/src/{encode,clip,filter_effects}.rs` | P23 |
| R21 | Curve and path geometry | kurbo `kurbo/src/` (bezpath, offset, simplify) | P20, P22 |
| R22 | Visual regression testing for a canvas renderer | penpot `render-wasm/docs/visual_regression_tests.md`, `docs/test_playground.md` | P23, P71 |

## 3.3 Other repositories and docs worth reading

Read the named part only.

| Source | Read for |
|---|---|
| `rust-skia/rust-skia` (skia-safe) | Building with the `textlayout` and `svg` features; WASM (emscripten) target notes |
| Skia CanvasKit docs | Only if ADR-001 picks the CanvasKit-in-TS path |
| `linebender/fontique` (in parley) | System font enumeration per OS |
| `notify-rs/notify` | Debounced file watching, rename/atomic-save handling |
| `kotauskas/interprocess` | Unix domain sockets and Windows named pipes with tokio |
| `electron/electron` docs | `contextIsolation`, `sandbox`, `safeStorage`, `autoUpdater`, `utilityProcess` |
| `electron-userland/electron-builder` | Signing, notarization, AppImage/deb, ARM64 |
| VS Code API docs | `CustomEditorProvider`, `WebviewPanel`, `workspace.fs`, `lm.registerMcpServerDefinitionProvider` |
| W3C Design Tokens (DTCG) spec | `dtcg-json` token format |
| Claude Code docs (headless mode, MCP config) | `claude-code` engine; verify flags against the installed version, never from memory |
| Codex docs `docs/exec.md`, `docs/config.md` | `codex` engine fallback when the app-server protocol is unavailable |

---

# 4. Product Invariants

These are not suggestions. Each is enforced by a test, a type, or a lint — not by hope.

## Invariant 1 — One Write Path

Every change to a document — by a person, an agent, a script, an importer, an external file
change or a merge — is a validated **Op transaction** applied by `vellum-core`. Nothing else can
mutate `DocStore`.

```rust
pub struct Transaction { pub origin: Origin, pub ops: Vec<Op> }   // the only input to DocStore::apply
```

Test: `DocStore::apply` is `pub(crate)` to `vellum-core::ops`; a trybuild compile-fail test proves
adapters and tools can't call it; a property test applies arbitrary op batches and checks the
document stays schema-valid after every transaction or the whole batch is rejected.

## Invariant 2 — Every Change Is Attributed

```rust
pub struct Origin { pub actor: Actor, pub session: Option<SessionId>, pub turn: Option<TurnId> }
pub enum Actor { Person { device: DeviceId }, Agent { agent: AgentId }, File { reason: FileReason }, System { subsystem: Subsystem } }
```

No `Transaction` can be built without an `Origin` (type-level). History entries, presence and
undo all read this field and nothing else. UI renders agents as rounded squares and people as
circles from `Actor` only.

## Invariant 3 — Undo Is Yours

⌘Z undoes only the pressing person's own most recent transaction. **Undo this turn** reverts
only transactions whose origin carries that turn. Reverting never silently drops another actor's
later change; when one depends on it, the result is a new attributed Revert transaction.
Property test: interleave transactions from 3 actors; any undo sequence by actor A leaves every
property last written by B or C unchanged.

## Invariant 4 — Your Edits Win, Paused Means Paused

Agent transactions pass the **Guard**: a write by agent X inside a frame leased by agent Y, or by
any paused or stopped agent, is rejected with a typed reason, recorded in History and returned to
the agent. Human transactions are never blocked by leases. Test: Pause all → the next agent
transaction from every engine is rejected, measured < 100 ms after the click in the editor
(Guard state is local); the engine itself is interrupted within 3 s (Invariant 12).

## Invariant 5 — The File Is The Truth, And It Is Deterministic

`.vellum` is our documented JSON. `load → save` is byte-identical. The same document always
serializes to the same bytes regardless of edit order. No library's native format (Loro binary,
Skia picture) is ever the system of record on disk. Every older format version stays readable
forever; a newer one opens read-only. Tests: round-trip on every fixture; a property test that
applies the same logical edits in different orders and compares bytes; one fixture per migration.

## Invariant 6 — What Agents See Is What People See

A `screenshot` an agent receives is rendered by the same renderer code path (or a golden-matched
one) as the editor canvas. Test: the golden suite renders every fixture in the editor path and the
headless path; pixel difference above the per-fixture tolerance fails the build.

## Invariant 7 — Layout Matches The Browser

Auto-layout (row, column, grid, hug, fill, gap, padding, justify, align, absolute children) matches
Chrome's CSS flexbox/grid on the fixture suite, so designs become code without surprises. Test:
layout fixtures generated by rendering their CSS equivalent in headless Chrome (the Taffy `gentest`
approach, R3); differences over 0.5 px fail.

## Invariant 8 — Scripts Can't Escape

`execute` scripts and script nodes run in `ScriptRuntime` with no network, file system, timers,
async or host objects beyond the `doc` API; with memory, time and op limits; and with seeded
randomness so the same script yields the same ops. Test: an escape corpus (fetch, import, eval of
host objects, prototype pollution, infinite loops, memory bombs) — every case returns a typed
`ScriptError`; determinism test runs each fixture script twice in native and WASM and compares ops.

## Invariant 9 — Local-Only By Default

The daemon listens only on a user-only Unix socket or named pipe; no TCP listener exists unless
`--listen` is passed. No network request leaves the machine except to a model provider or image
source the person configured, or to our cloud for features they signed in to. Telemetry is off
until opted in. No account is needed for any local feature (editor, CLI, MCP, exports, built-in
agent). Tests: a socket-inventory test on daemon start; an egress test in CI that runs the full
local E2E suite with all network blocked except the stub model endpoint.

## Invariant 10 — Keys Are Keychain-Only

API keys and tokens live in `SecretStore` (OS keychain). They never appear in a `.vellum` file,
the session store, logs, crash reports, MCP results, History, telemetry or the CLI's `--json`
output. Enforced by a **canary test**: a unique canary key is stored, a full scripted session
runs (built-in engine, permissions, export, share), then every file under the repo, `~/.vellum/`,
logs and recorded frames is scanned for the canary bytes. Zero hits or the build fails.

## Invariant 11 — Outside The Canvas Needs Permission

Any agent action outside the canvas — reading or writing a file, running a command, network
access by the built-in agent beyond the model call — goes through `PermissionBroker`. Canvas
edits never prompt. There is no code path that performs one of those actions without an
`Allowed` decision record (from the person, a policy rule, or an engine's auto mode the person
turned on). Property test over generated tool calls; the decision record is in the session store.

## Invariant 12 — Always Interruptible

Stop on a working session yields `turn.state = stopped` and no further transaction from that
session within **1 s** (Guard), and the engine process or request is interrupted within **3 s**
(p95). Stopping keeps everything already built and offers Undo this turn. Tested against the
built-in engine with a scripted slow model and against each installed engine.

## Invariant 13 — Honest About The Machine

Daemon offline, basic renderer, missing fonts, newer file format, unreadable file, write
rejected, key or engine failed, usage limit, agent stuck, missing library, recovered changes
and share upload failed are explicit states with the exact copy from `DESIGN.md` (S01–S16,
A06, W02). No view shows stale data as live. Snapshot tests cover each state in light and dark.

## Invariant 14 — Clean Room

pen.dev is used as the design tool for Vellum's own screens (§2.1); that is fine. But no code,
text, prompt, skill, API wording or asset is copied from pen.dev, no generated HTML from its
export is pasted into the product, and Vellum's design must not reproduce pen.dev's interface; nothing ported from
Penpot (MPL) or tldraw (commercial). A CI check scans for a deny-list of distinctive strings and
for missing attribution headers on files that declare `ported-from`. Matching capabilities from
pen.dev's public documentation is fine and expected (ARCHITECTURE §7.4).

---

# 5. Current Technical Baseline — October 11, 2026

Verified against crates.io and npm on 2026-10-11. At scaffold time, re-check the latest stable
patch of each and record exact resolved versions. No canary/beta/rc/alpha unless this table says so.

```text
Rust                       latest stable toolchain (rust-toolchain.toml pins it); wasm32 + emscripten targets per ADR-001
tokio                      1.53.x
axum                       0.8.x        (daemon --listen mode, sync server)
interprocess               2.4.x        (Unix sockets / named pipes)
loro                       1.16.x
taffy                      0.14.x
parley / fontique          0.12.x
skia-safe                  0.153.x      (features: textlayout, svg, pdf; gl or vulkan off for headless)
vello                      0.11.x       (later, behind a flag)
kurbo                      0.13.x
i_overlay                  9.0.x        (boolean ops)
rstar                      0.13.x
rquickjs                   0.14.x
rmcp                       3.5.x
schemars                   1.2.x
ts-rs                      12.x
wasm-bindgen               0.2.129+     (if ADR-001 picks wasm-bindgen over emscripten exports)
notify                     8.2.x
rusqlite                   0.40.x       (session store; bundled SQLite)
keyring                    4.2.x
clap                       4.6.x
chromiumoxide              0.9.x        (v1.1, BrowserPort)
naga                       30.x         (v1.1, only if ADR-008 translates shaders through it)
cargo-deny                 0.20.x

Node                       24 LTS line for JS tooling
pnpm                       12.11.x
Turborepo                  2.11.x
TypeScript                 7.0.x native line; if any tool (vsce, Electron types, Vite plugins) conflicts, newest 5.x — record decision
Vite                       8.3.x
React                      19.3.x
@base-ui/react             1.9.x
shadcn/ui                  CLI version current at scaffold (generated source, not a dependency)
Tailwind CSS               4.3.x
Zustand                    5.0.x        (vanilla stores in editor-core)
Zod                        4.6.x
i18next                    26.x
Electron                   44.x         (+ electron-builder 26.15.x, electron-updater 6.8.x)
@vscode/vsce               4.0.x  ·  ovsx 1.2.x
canvaskit-wasm             0.42.x       (only if ADR-001 picks the CanvasKit-in-TS path)
@modelcontextprotocol/sdk  1.32.x       (TS side only for the extension's MCP registration helper)
Playwright                 1.64.x  ·  Vitest 5.0.x  ·  Storybook 10.6.x  ·  Biome 2.5.x
dependency-cruiser         18.5.x
Next.js                    16.4.x       (account app, web viewer, marketing)
better-auth                1.7.x  ·  drizzle-orm 0.45.x  ·  Hono 4.13.x
Installed engines          Claude Code 2.1.x, Codex CLI 0.162.x at the time of writing — tested versions pinned per release (ADR-007)
pen.dev (design tool)      desktop app current release + @pen.dev/cli 0.3.x (Node ≥ 22.19) for scripts/design-snapshot;
                           a design-tool dependency only — never shipped, never imported by product code
```

## Version policy

1. Stable lines only; exact versions in `Cargo.lock` and `pnpm-lock.yaml`.
2. No `"latest"` or unbounded ranges in manifests.
3. Record every resolved version and every compatibility decision in
   `docs/architecture/version-baseline.md`.
4. The renderer stack (skia-safe or canvaskit-wasm) is upgraded only in its own prompt with the
   golden suite re-run; never mid-prompt.
5. Installed-engine versions (Claude Code, Codex) are tested ranges, not dependencies; a new
   major gets its own prompt with the engine fixture suite re-recorded (Prompts 36–37).
7. A pen.dev version change that alters snapshots is reviewed like a design change: regenerate
   snapshots in their own PR (Prompt 3's script) and re-run the UI goldens.
6. Electron major, Loro major and the `.vellum` format version each get their own reviewed prompt.

---

# 6. Stack Decisions

## 6.1 Verified deltas from ARCHITECTURE.md

The reference research on 2026-10-11 changed or sharpened six decisions. Prompt 2 writes each as
an ADR with status "Proposed"; the prompt named in brackets confirms or rejects it.

**D1 — Consider one Rust renderer everywhere: skia-safe compiled to WASM.** [Prompt 4 → ADR-001]
ARCHITECTURE §3.2 has the editor drawing with CanvasKit from TypeScript and the daemon drawing
with skia-safe natively, matched by goldens. Penpot's `render-wasm` already ships Rust + Skia
compiled to WASM (emscripten) with tile rendering, text and fills in Rust (R1). If Vellum does the
same, the editor and the headless renderer are **the same code**, which removes the text-parity
open question and makes Invariant 6 nearly free. Costs: emscripten toolchain, a larger WASM
bundle, and GPU context handling in Rust. The `Renderer` port stays either way; the spike decides
the default.

**D2 — Layout goldens come from Chrome, not from hand-written expectations.** [Prompt 6 → ADR-002]
Taffy generates its test fixtures by laying out HTML in headless Chrome (`scripts/gentest`, R3).
Vellum adopts the same oracle for its own auto-layout semantics (Invariant 7), including hug/fill
mapping and absolute children.

**D3 — `vellum mcp` is a stdio-to-socket bridge.** [Prompt 27 → ADR-003]
Codex ships exactly this shape (`stdio-to-uds`, R12): MCP clients launch a tiny stdio process that
forwards to the user-only daemon socket. Vellum's `vellum mcp` does the same, so every client gets
the live document when the editor has it open and headless mode otherwise, with no TCP port.

**D4 — Installed engines are ported from Goose adapters, and Codex is driven through its
app-server protocol.** [Prompts 36–37 → ADR-007]
Goose (Apache-2.0) already implements Claude Code, Codex, Cursor Agent and Gemini CLI as providers
(R14). Port with attribution rather than reverse-engineering CLIs. For Codex, prefer its JSON-RPC
app-server protocol (`codex-rs/app-server-protocol`) over parsing `codex exec` output; fall back to
`exec` only when the installed version lacks it.

**D5 — `execute` follows the "code mode" pattern.** [Prompts 25–26 → ADR-005]
Codex and OpenCode both ship code-mode runtimes where a model writes a program against a typed API
instead of many tool calls (R9); Penpot's MCP server exposes code execution against its plugin API
(R8). This confirms ARCHITECTURE §7.3's two-layer design. Vellum's runtime stays QuickJS
(Invariant 8, deterministic, embeddable in WASM), and the `doc` API ships a generated `.d.ts` so
models get types, as those projects do.

**D6 — Session storage follows rollout-style append-only logs.** [Prompt 34 → ADR-006]
Codex's rollout/thread-store and Goose's session manager (R16) confirm the append-only event log
per session in ARCHITECTURE §7.6. Vellum keeps SQLite for the index and events, with thumbnails
in a content-addressed cache.

## 6.2 In

| Layer | Choice | Reason |
|---|---|---|
| Monorepo | Cargo workspace + pnpm workspaces + Turborepo | One Rust core for WASM, daemon, CLI, workers |
| Document engine | `vellum-core` (Rust) over ports; Loro behind `DocStore` | Invariants 1–5 |
| Layout / text | Taffy; Parley + fontique | Browser-faithful layout (Invariant 7), real text shaping and bidi |
| Rendering | skia-safe (native) + WASM path per ADR-001; canvas2d fallback; Vello later | Invariant 6 |
| Scripts | rquickjs behind `ScriptRuntime` | Invariant 8, runs in WASM and native |
| Daemon | `vellum` — Rust, tokio, interprocess, rmcp, notify, rusqlite, clap | One static binary: MCP, CLI, agent runner, headless render |
| Agent engines | built-in (our loop over `ModelProvider`s), Claude Code, Codex; later Gemini CLI, Copilot, Cursor, OpenCode | Bring your own model or subscription |
| Editor | Vite + React + shadcn/Base UI over `editor-core` (framework-agnostic, Zustand vanilla) | UI replaceable without touching behavior |
| IDE host | VS Code extension (`CustomEditorProvider`), published to Marketplace + Open VSX | Cursor, Windsurf, VS Code |
| Desktop host | Electron + electron-builder + electron-updater | Same Chromium as VS Code |
| Account & cloud | Next.js + tRPC + BetterAuth + Drizzle + Postgres; Hono + Bun public API; Polar.sh | Your standard stack; cloud only for Pro features |
| Storage (ours) | Cloudflare R2 behind `ObjectStore` | Share snapshots, assets, exports |
| Hosting (ours) | Netcup RS 1000 G12 + Dokploy; Infisical; GitHub Actions | Your standard ops |
| Errors / analytics | Sentry + PostHog (cloud) behind `Telemetry`; **off by default** in editor, CLI and daemon | Invariant 9 |

## 6.3 Out of the v1 critical path

- **Live multiplayer, comments, sync server.** v1.1 (Prompts 63–64).
- **Browser port, html importer, embed node, Figma file import, image generation and image ops,
  shader fills and effects, slides and presenting.** v1.1 (Prompts 65–68).
- **Vello renderer.** Behind a flag after ADR-001's revisit condition.
- **Browser-based editor for cloud files.** Web viewer only in v1.
- **Plugins API.** Registries exist; public API later.
- **Mobile app.** Read-only viewer later.
- **Hosted models.** BYO keys and installed engines only.
- **Reading other tools' proprietary formats** (`.fig` files, `.pen`). Legal review first.
- **Outbound telemetry without opt-in.** Never.

---

# 7. Architecture (as built)

```text
 VS Code / Cursor / Windsurf                Desktop (Electron)                  Browser (W01 viewer)
   extension host ─ postMessage ─┐            main ─ IPC ─┐                       Next.js page
                                 ▼                        ▼                             │
               ┌──────────────── EDITOR bundle (same everywhere) ──────────────────┐   │ read-only
               │ editor-react (UI) → editor-core (tools, commands, registries)      │   │
               │ engine-client → vellum-wasm (core + renderer per ADR-001)          │◄──┘
               └───────────────────────────────┬────────────────────────────────────┘
                                               │ daemon protocol v1 (relayed by host; never a socket in the webview)
 ┌─────────────────────────────── `vellum` daemon (native Rust) ───────────────────────────────┐
 │ IPC listener (~/.vellum/run/vellum.sock or \\.\pipe\vellum-<user>)                         │
 │ Tool registry ─► MCP adapter · CLI adapter · agent runner                                  │
 │ DocumentTarget: Live (forward to editor) | Headless (native core + render-skia + file)     │
 │ AgentEngines: builtin(ModelProvider) · claude-code · codex  │ SessionStore (SQLite)        │
 │ PermissionBroker · SecretStore (keychain) · SkillSource · StyleLibrary · ScriptRuntime     │
 │ File watcher → diff → ops (Actor::File) · recovery copies · share uploads                  │
 └───────────────▲─────────────────────────────────────────────▲──────────────────────────────┘
                 │ stdio (`vellum mcp` bridge, D3)              │ child processes (installed engines)
   Claude Code · Cursor · Codex · Gemini CLI · Copilot …     claude / codex (user's own sign-in)
```

## Ownership

- `vellum-core` owns document semantics: ops, validation, resolution, layout, scene, display list.
- The **file** owns document truth (Invariant 5). The daemon owns sessions, permissions, keys,
  leases, presence and recovery copies. Nothing in the daemon's stores is needed to open a file.
- The **editor** owns a live in-memory engine when a file is open; the daemon forwards agent ops to
  it (Live target). With no editor, the daemon's native engine owns it (Headless target).
- Exactly one writer per file at a time: if two editors open the same file, the second attaches to
  the first's engine through the daemon (or opens read-only if the daemon is offline).
- A model provider never sees a key it doesn't need; an installed engine sees none of ours.

## Two history layers

1. **Document history:** transactions with origins in `DocStore` (undo, History tab, Undo this turn).
   Session-scoped; Git is the permanent history.
2. **Session history:** turns and steps in `SessionStore` (Agent panel). Each step that changed
   the document references the transaction IDs it produced, so History ↔ Agent panel links both ways.

---

# 8. Monorepo

```text
vellum/
  Cargo.toml                 Rust workspace
  rust-toolchain.toml
  pnpm-workspace.yaml
  turbo.json
  crates/
    vellum-types/            data types, display list, protocols, brand constants (serde, schemars, ts-rs)
    vellum-core/             ops, guard, resolver, layout, scene, display list, diff/merge, export + PORTS
    contracts/               contract suites for every Rust port + fakes
    adapters/
      doc-loro/  layout-taffy/  text-parley/  index-rstar/  pathops-kurbo/
      codec-json/  codec-json-dir/  assets-fs/  assets-s3/
      render-skia/  render-vello/ (later)  shader-sksl/ (v1.1)
      script-quickjs/  sessions-sqlite/  secrets-keyring/  browser-cdp/ (v1.1)
      llm-openai-compat/  llm-openai-responses/  llm-anthropic/  llm-google/
      engine-builtin/  engine-claude-code/  engine-codex/
      import-svg/  import-image/  import-figma-clipboard/  import-figma-api/ (v1.1)  import-html/ (v1.1)
      icons/  images-stock/  image-ops/ (v1.1)
      export-tokens/  export-html/  export-raster/
    vellum-wasm/             composition root: browser engine
    vellum/                  composition root: daemon + CLI + MCP bridge
    sync-server/             composition root: sync (v1.1)
    vellum-testkit/          stub model endpoint, fake engines, fixture docs, Chrome layout oracle runner, clocks
  packages/
    config/                  brand, copy constants (exact strings, Appendix B)
    types-ts/                GENERATED by ts-rs — never hand-edited
    renderer-api/  renderer-canvas2d/  (renderer-canvaskit/ only if ADR-001 says so)
    host-api/  host-vscode/  host-electron/  host-browser/  host-fake/
    engine-client/           EngineClient over vellum-wasm (+ fake)
    editor-core/             tools, commands, keybindings, registries, selection, viewport (no React)
    editor-react/            UI: shadcn / Base UI panels
    ui-tokens/               tokens (light / dark / high-contrast; chrome and canvas groups)
    contracts/               TS contract suites (Renderer, Host, EngineClient)
    auth/ billing/ storage/ mail/ jobs/ telemetry/   cloud wrapper packages
    design-systems/          starter libraries (*.lib.vellum) + guidelines.md
    skills/  styles/  prompt-packs/
    test-utils/
  apps/
    extension/  desktop/  account/  api/  web/  docs/
  fixtures/                  golden documents, reference images, layout oracle HTML, engine transcripts
  scripts/
    refs.sh                  clones refs/ at pinned SHAs
    design-open              copies docs/design/vellum.pen to .design-cache/ and opens the copy in pen.dev
    design-snapshot          exports every inventoried frame to docs/design/snapshots/ via the pen CLI
    design-check             inventory + checksum + naming-convention checks (CI)
    gen-layout-oracle.sh     renders layout fixtures in headless Chrome
    record-engine-fixtures.sh
  refs/                      (gitignored)
  .design-cache/             (gitignored) read-only working copy of the design file
  docs/                      see §2
  .github/workflows/
```

---

# 9. Data And Numeric Model

## Identifiers

- In the file: `NodeId`, `PageId`, `VariableId`, `ComponentId` — 10-character nanoid-style strings
  from `IdGen`, unique per document, never reused, no `/` (paths use `/` as separator).
- Outside the file: `SessionId`, `TurnId`, `StepId`, `AgentId`, `TransactionId` — ULIDs.
- All typed newtypes in `vellum-types`. Never raw strings across crate boundaries.

## Geometry

- Model coordinates are `f64` CSS pixels; display list is `f32` in device pixels after transform.
- **Quantization for determinism:** positions, sizes, radii, gaps and paddings serialize rounded to
  1/100 px; ratios, opacities and gradient stops to 1/1000; angles to 1/100 degree. Rounding happens
  in the codec only, never in live state, and is idempotent (property test).
- Rotation in degrees, clockwise; transforms as 2×3 affine in the file only for image crops and
  gradients.

## Colour

- File: sRGB hex `#RRGGBB` or `#RRGGBBAA`, or a variable reference.
- Renderer: linear-light blending per Skia defaults; the golden suite pins the colour pipeline.
- Contrast checks (design tokens, lint) use WCAG 2.x relative luminance.

## Time

UTC milliseconds (`i64`) everywhere. Rendered in local time. No function in `vellum-core`,
`editor-core` or the daemon's logic reads the wall clock directly; tests inject `Clock`.

## Usage and money

- Tokens as `u64` per kind (input, output, cache read, cache write).
- Estimated cost as `u64` micro-USD from a local `pricing.json` (versioned data file, not code),
  always rendered with "Estimated" (Appendix B). No other money in the editor or daemon.
  Plan prices come only from billing metadata in the account app.

---

# 10. State Machines

All pure, table-driven, exhaustively tested over (state × input).

## 10.1 Turn

```text
queued ──start──▶ planning ──plan needed──▶ awaiting_plan ──Build──▶ running
   │                 │ no plan needed                      │ Edit request → planning
   │                 └─────────────────────────────────────▶ running
   │                                                          │ permission requested
   │                                                          ▼
   │                                                   awaiting_permission ──decided──▶ running
   │                                                          │
   │                       running ──done──▶ done   running ──error──▶ failed
   └──Stop (any non-terminal state)──▶ stopped        (stopped, failed and done are terminal; Retry creates a new turn)
```

Rules:
1. `stopped` is only entered by a person (Stop, Stop all, Pause all) — never by the system.
2. A step running longer than 120 s emits a `stuck` flag (S10) without changing state.
3. Every transition appends one session event with the actor.
4. Mapping to the Agent panel's session dot (working, needs you, done, failed) is a total function.

## 10.2 Lease

```text
free ──claim(agent)──▶ held(agent, expires_at) ──activity──▶ held (expiry extended 60 s)
                          │ release | expiry | Stop on lease tab (person) | Pause all
                          ▼
                        free
```

A held lease never blocks a person. Stop on a lease tab also marks `(agent, frame)` as blocked until
resumed, so a re-claim by that agent is rejected.

## 10.3 Document save state

```text
clean ──edit──▶ dirty ──save──▶ saving ──ok──▶ clean
                                   └─fail──▶ save_failed ──retry──▶ saving
any ──newer format on open──▶ read_only        any ──external change──▶ (apply as ops, stay in current state)
```

## 10.4 Agent (presence)

`idle · working · paused · blocked_by_permission · failed · disconnected`, derived from its active
turn, the Guard's pause set and its connection; never stored independently.

---

# 11. Global TDD Contract

Every prompt:

1. Read `AGENTS.md` (this repo).
2. Read this guide.
3. Read the named product/design/architecture files.
4. Read the named reference-repo paths (§3) and summarize them in the PR description.
5. Inspect existing implementation.
6. Add failing tests first.
7. Run focused tests and confirm the expected failure.
8. Implement the smallest coherent production change.
9. Re-run focused tests.
10. Run `cargo clippy -D warnings`, `cargo deny check`, the dependency-rule script, `pnpm type-check`,
    lint and dependency-cruiser for affected packages.
11. Preserve unrelated changes.
12. Never modify `docs/product/`, `docs/design/`, `docs/format/` (except by regeneration) or `refs/`.
13. Never weaken a test to pass.
14. Never hardcode example data (Sam, Juniper, Teal, Saffron, juniper.vellum) outside fixtures and
    stories; fixtures carry a `TEST DATA` marker.
15. Inject clocks and ID generators; no wall-clock or randomness in logic.
16. Mock network, file system, keychain and model-provider boundaries in unit tests; use the
    **stub model endpoint** and **recorded engine transcripts** (Prompts 32, 36–37) for agent tests,
    and real installed engines only in the tagged `engines` lane.
17. Make missing, stale, offline, truncated and read-only data explicit — never fabricate completeness.
18. Behavior contracts over snapshots; golden images only where pixels are the behavior.
19. Any test that needs a GPU, headless Chrome, VS Code, Electron, an installed engine or the
    network is tagged and documented with what it needs; CI runs it in the matching lane.
20. Secrets never appear in test output (the canary scanner runs on test logs too).
21. No vendor crate or package outside an adapter or wrapper package (dependency rules, ARCHITECTURE §2.1).
22. UI prompts read the design through the Pencil MCP following §2.1 (confirm document, read_skill,
    read-only, frames by screen ID, screenshot compare); never modify `docs/design/vellum.pen`.
23. Record any design-file vs DESIGN.md mismatch in `docs/design/DESIGN-GAPS.md`; never silently
    pick one.

---

# PHASE 0 — Foundation

---

# PROMPT 1 — Monorepo Scaffold, Crate/Package Shells, AGENTS.md, CI Skeleton

```text
Initialize the Vellum monorepo. No business logic. No UI.

Read first:
- docs/product/PRODUCT.md
- docs/architecture/ARCHITECTURE.md §1, §2, §13
- docs/design/DESIGN.md (Visual system and Content sections only)
- this guide §1, §2, §4, §8, §11

## TDD FIRST

Failing smoke tests:
- @vellum/config exports brand.name === "Vellum", tagline === "Design where your code lives."
- vellum-types exports FORMAT_VERSION == 1 and a placeholder Document type
- `vellum --version` exits 0; `vellum mcp --help` exits 0
- vellum-wasm builds for the browser target with a placeholder export
- packages/types-ts is generated (a stub type exists) and marked generated
- checksums of docs/product, docs/design/DESIGN.md and docs/design/vellum.pen unchanged by build/test

Confirm failures, then implement placeholders.

## ROOT

Cargo.toml (workspace), rust-toolchain.toml (with wasm32 targets), pnpm-workspace.yaml,
turbo.json (cargo tasks wrapped), tsconfig.base.json, biome.json, .editorconfig, .gitignore
(refs/, target/, .design-cache/, .env*, *.vellum.recovery), .env.example, AGENTS.md, README.md,
THIRD_PARTY_NOTICES.md (empty table), docker-compose.dev.yml (Postgres + Redis for the cloud apps),
.github/workflows/ci.yml (fmt, clippy, test, type-check, lint, checksum).

Scripts: dev · build · lint · type-check · test · test:watch · format · format:check · refs ·
deps:check · clean-room · types:gen · layout-oracle · design:open · design:snapshot · design:check ·
fixtures:record · clean

## CRATES & APPS

Every crate and package in guide §8 as a shell with a smoke test, including apps/extension
(VS Code skeleton), apps/desktop (Electron skeleton), apps/account and apps/web (Next.js).

## AGENTS.md MUST STATE

- docs/product, docs/design/DESIGN.md, docs/design/vellum.pen, docs/format (except regeneration)
  and refs/ are READ-ONLY; docs/design/DESIGN-GAPS.md is append-only
- the design is read through the Pencil MCP per guide §2.1: open the read-only copy with
  scripts/design-open, confirm the document with get_app_state, call read_skill first, read only,
  ask for frames by screen ID, compare screenshots, values come from .pen variables/components
- build only the options recorded in docs/design/DESIGN-DECISIONS.md
- every document change is an Op transaction with an Origin; nothing else touches DocStore
- vendor crates and packages live only in adapters/ or wrapper packages
- read the guide §3.2 reference paths before implementing a listed hard problem; cite them
- port only from MIT/Apache projects, with a source header and a THIRD_PARTY_NOTICES entry;
  never port from penpot or tldraw; never copy anything from pen.dev (its HTML export included)
- blue = selection, violet = component, agent hues = agents, amber = needs you, red = errors;
  chrome stays neutral
- agents are rounded squares, people are circles; state = symbol + label, never colour alone
- agents are named by hue (built-in) or client (external); no personas, no emoji, no exclamation marks
- "on your computer", "in your repo"; "cloud" only for Pro features
- keys live in the OS keychain only; the canary scanner is law
- no network listener, telemetry or account requirement for local features

## FINAL

cargo build --workspace · cargo test --workspace · cargo clippy -D warnings · pnpm install ·
pnpm test · pnpm type-check · pnpm lint
```

---

# PROMPT 2 — Reference Repos, Version Baseline, Dependency And Clean-Room Checks, ADR-000

```text
Read: guide §3, §5, §6; ARCHITECTURE §2.1, §2.2.

## VERSION VALIDATION FIRST
Resolve latest stable patches for every line in guide §5. Write exact versions and every
compatibility decision to docs/architecture/version-baseline.md. Reject canary/beta/rc/alpha.

## REFERENCE REPOS
- scripts/refs.sh clones every repo in guide §3 into refs/ (blob-less, shallow) at the pinned SHAs
  or the newest release tag at/after them — record which.
- Generate docs/reference/REFERENCE-INDEX.md from §3.2 and §3.3 with a path check that fails if a
  listed path is missing; penpot marked READ ONLY (MPL-2.0), tldraw DOCS ONLY.

## GUARDRAILS
- scripts/check-deps.sh over `cargo tree` with an allowlist for vellum-types and vellum-core;
  cargo-deny bans; dependency-cruiser rules for packages/ (editor-core can't import React or a
  renderer; apps can't import vendor SDKs, only wrapper packages)
- scripts/check-clean-room.sh: deny-list strings (scripts/clean-room-denylist.txt, human-maintained)
  and `ported-from:` headers must have THIRD_PARTY_NOTICES entries
- copy-lint skeleton: @vellum/config exports Appendix B strings; UI packages may not duplicate them

## ADR
docs/architecture/decisions/ADR-000-deltas.md: D1–D6 from §6.1, status "Proposed", with the
confirming prompt for each.

## TDD FIRST
- refs/ untracked; refs.sh idempotent; reference index path check passes
- check-deps.sh fails when a fixture crate adds `loro` to vellum-core (negative test)
- clean-room check fails on a fixture file with a denied string and on a ported file without notice
- no prerelease versions in lockfiles

## FINAL
refs.sh && index check · deps:check · clean-room · cargo deny check · pnpm lint
```

---

# PROMPT 3 — Design Source: Pencil MCP Access, Read-Only Copy, Inventory, Snapshots, Decisions

```text
Make the design file usable by every later UI prompt, and by CI without pen.dev.

Read:
- guide §2 and §2.1 (whole)
- docs/design/DESIGN.md "Screen requirements", "Component library", "Deliverables"
- pen.dev public docs: AI Integration (MCP), CLI (interactive + export) — capability reference only

## RUNBOOK
docs/runbooks/design-mcp.md: install pen.dev, sign in, enable the MCP integration for the agent
in Settings → MCP, run scripts/design-open, confirm `pencil` in the client's MCP list, the
session rules from §2.1, and what to do when the MCP is unavailable.

## SCRIPTS
- scripts/design-open: copy docs/design/vellum.pen (+ assets) to .design-cache/vellum.readonly.pen
  and open it in pen.dev (desktop app or the IDE extension); prints the path to confirm with
  get_app_state.
- scripts/design-snapshot: uses the pen CLI in headless interactive mode on the read-only copy.
  It first calls read_skill to get the current execute syntax (never hardcoded), then for every
  frame matching the naming convention exports PNG at 1× and 2× to docs/design/snapshots/ and
  writes inventory.json {frame_id, screen_id, title, theme, width, page, bounds, components used,
  source_sha256}. Requires `pen login` or PEN_CLI_KEY; run by a human or an agent locally.
- scripts/design-check (CI, no pen.dev needed):
  * docs/design/vellum.pen checksum equals inventory.source_sha256 (snapshots are current)
  * every screen ID × theme × width required by DESIGN.md "Deliverables" exists in the inventory;
    missing frames are listed, and UI prompts that need them must stop
  * frame names follow the convention; component names match DESIGN.md's Component library
  * variables collections and names follow the convention (feeds Prompt 43)

## DESIGN FILES
- docs/design/DESIGN-DECISIONS.md from Appendix G, all UNDECIDED (only a human edits choices)
- docs/design/DESIGN-GAPS.md with a header and the line format

## TDD FIRST
- design-check unit tests over a fixture inventory: missing screen, wrong theme name, renamed
  component, stale checksum → each fails with a precise message
- a checksum test proves no test or script modifies docs/design/vellum.pen

## MCP SMOKE (tagged design-mcp, run locally)
In a session with the Pencil MCP connected: get_app_state shows the read-only copy; read_skill
succeeds; a read-only execute lists frames on the "Editor Agent" page; a screenshot of
"C03 Agent workspace / Light / 1280" is saved to .design-cache/. Record the transcript summary in
the PR (no design content beyond frame names).

## FINAL
design-check passes on the committed inventory · runbook reviewed · decisions file committed
```

---

# PHASE 1 — Spikes

---

# PROMPT 4 — Spike A: Renderer → ADR-001

```text
Decide D1 with numbers. Spike quality; rewritten in Prompts 22–23.

Read (required):
- refs/penpot/render-wasm/docs/{rendering_architecture,tile_rendering,serialization}.md
- refs/penpot/render-wasm/src/{render.rs,tiles.rs}, src/render/{surfaces,fills}.rs (skim)
- refs/graphite/frontend/wrapper/src/{editor_wrapper,wasm_value}.rs (Rust ⇄ JS boundary)

## BUILD
- generated documents of 1,000 / 5,000 / 10,000 nodes (frames, text, rects, images) as plain data
- Renderer A: Rust builds a display list; TS draws it with canvaskit-wasm
- Renderer B: skia-safe compiled to WASM (emscripten), drawing into WebGL2 from Rust; the same
  crate renders PNG natively
- a spike page with pan/zoom, FPS meter and a screenshot button for each

## MEASURE (ADR-001)
fps per size on an M1 and a mid-range Windows laptop (or the GPU CI lane) · drag input → frame
latency · WASM size (gzip) and cold load · native vs WASM pixel difference on 20 non-text fixtures

## TDD FIRST
testkit pixel-diff helper (tolerance, diff image artifact) · gpu-tagged test rendering fixture #1
in both paths

## FINAL
ADR-001 (choice, numbers, Vello revisit condition) · artifacts in the PR
```

---

# PROMPT 5 — Spike B: Text Parity, Loro At Scale, QuickJS In WASM → ADR-004

```text
Read:
- refs/parley/examples/{tiny_skia_render,vello_cpu_render}/src/main.rs; refs/penpot/render-wasm/docs/texts.md
- refs/loro/context/{tree-events,tree-checkout-window}.md, docs/encoding.md
- refs/rquickjs/core/src/runtime/{base,raw}.rs, wasm-test/

## BUILD AND MEASURE
- 30 text fixtures (Latin, Arabic, CJK, emoji, mixed bidi, wrapping) through both renderer paths
  from Prompt 4: max and p95 pixel difference; record which path ADR-001 should prefer for text
- Loro: load/save ms and MB for the 10k document as a movable tree; 200-node batch apply ms
- rquickjs for wasm32 inside vellum-wasm: added KB; a 500-op script's run time

## ADR
ADR-004-loro: confirm Loro behind DocStore or name the alternative; the directory-codec threshold.
Append the text results to ADR-001.

## FINAL
numbers tables in ADRs · spike tests tagged and green
```

---

# PROMPT 6 — Spike C: Chrome Layout Oracle → ADR-002

```text
Read: refs/taffy/scripts/gentest/{src/main.rs,test_helper.js,test_base_style.css},
refs/taffy/test_fixtures/ (format), refs/taffy/docs/style-properties.md.

## BUILD
- scripts/gen-layout-oracle.sh: renders fixtures written as HTML (our sizing semantics mapped to
  CSS: hug → fit-content, fill → flex-grow/stretch, absolute children, gap, padding, justify incl.
  space-between/around, align, grid) in headless Chrome and writes expected boxes as JSON
- 40 starter fixtures; Taffy computes the same fixtures in a spike test

## ADR
ADR-002-layout-oracle: accept D2; the CSS mapping for every sizing/alignment option; every
semantic mismatch found and how Vellum resolves it.

## TDD FIRST
testkit oracle loader round-trips expected boxes; tolerance 0.5 px.

## FINAL
oracle script reproducible in CI's chrome lane · ADR-002
```

---

# PHASE 2 — Types And The Write Path

---

# PROMPT 7 — vellum-types I: Nodes, Paints, Strokes, Effects, Sizing

```text
Read: ARCHITECTURE §4.1 (node union, paint, stroke, effects, blend modes, common properties);
guide §9.

## TDD FIRST
- Node tagged union: frame, group, rect, ellipse (inner radius, start/sweep), polygon, star, line,
  vector (path, viewBox, fillRule), boolean, text (rich runs, align, vertical align, resize),
  image (fit, crop), icon, instance (placeholder fields until Prompt 8), script, note, context,
  prompt, embed (typed now, gated v1.1)
- common fields: id, name, visible (literal or var), locked, opacity, rotation, flipX/Y, position
  (auto | absolute), width/height (Fixed | Var | Hug{fallback?} | Fill{fallback?}), fills[],
  stroke?, effects[], blendMode, agentNotes?, prototype?, metadata
- Paint: solid, gradient (linear/radial/angular, stops, transform), mesh, image, shader
- Stroke: paints, width (uniform or per side), align, cap, join, dash; Effects: drop/inner shadow,
  layer blur, background blur, shader; BlendMode (18 values)
- serde deny_unknown_fields except `metadata`; Value<T> = Literal | VarRef on bindable fields
- IDs typed; no "/" in IDs; NodeIdPath parse/print round-trip

## FINAL
cargo test -p vellum-types · property tests for serde round-trip
```

---

# PROMPT 8 — vellum-types II: Variables, Themes, Components, Libraries, Scripts; Schema + TS

```text
Read: ARCHITECTURE §4.1 (themes, variables, aliases, overrides, slots, scripts, slides), §4.4.

## TDD FIRST
- Document { meta {formatVersion, name, createdWith, kind: document | library}, imports, themes
  (axis → ordered values), variables (type, values [{value | alias, when}]), styles, fonts,
  components, pages (kind canvas | slides) }
- Instance { componentId, variant, props, overrides {NodeIdPath → props | replacement | children},
  slotContent }; frame.slot {suggested}; frame.theme
- script node { src, inputs (literal | var), clip }
- schemars JSON Schema → docs/format/vellum.schema.json; generated spec page in docs/format/
- ts-rs → packages/types-ts/src/generated/; CI regenerates and fails on diff

## FINAL
types:gen + schema diff check · pnpm type-check
```

---

# PROMPT 9 — JSON Codec: Deterministic Bytes, Quantization, Migrations

```text
Read: ARCHITECTURE §4.3; guide §9 (quantization), Invariant 5.

## TDD FIRST
- codec-json: fixed key order per type, 2-space indent, LF, trailing newline, quantization in the
  codec only (positions/sizes 1/100 px, ratios 1/1000, angles 1/100°)
- load → save byte-identical on every fixture; property test over generated documents
- same logical edits in different orders → identical bytes
- unknown fields rejected with path, line and column (S05)
- FORMAT_VERSION = 1; Migration trait + registry; newer version → typed NewerFormat (S04);
  each migration ships a before/after fixture
- quantization idempotent

## FINAL
cargo test -p codec-json · fuzz target skeleton for the decoder
```

---

# PROMPT 10 — Ports, Fakes, Contract Suites

```text
Read: ARCHITECTURE §2 (whole), §3.1, §3.4 (trait sketches).

## TDD FIRST
- traits in vellum-core::ports: DocStore, LayoutEngine, TextEngine, FontSource, SpatialIndex,
  PathOps, Codec, AssetStore, Clock, IdGen, ScriptRuntime, ShaderCompiler
- crates/contracts: a contract suite per port and a fake per port (in-memory DocStore, fixed-metric
  TextEngine at 0.6 em per glyph, naive SpatialIndex, identity PathOps, deterministic Clock/IdGen,
  no-op ScriptRuntime, passthrough ShaderCompiler)
- every fake passes its suite; suites are generic so adapters reuse them
- check-deps enforced for vellum-core

## FINAL
cargo test -p vellum-core -p contracts · deps:check
```

---

# PROMPT 11 — Ops I: Structural Ops, Validation, Origins

```text
Read: ARCHITECTURE §5 (Ops); guide Invariants 1–2;
refs/graphite/editor/src/messages/portfolio/document/document_history.rs;
refs/excalidraw/packages/element/src/{store,delta}.ts.

## TDD FIRST
- Transaction { origin, ops }; Origin { actor, session?, turn? } mandatory (type-level)
- ops: insert, update, delete, move, copy, replace, set_variables, reorder, wrap_in_frame,
  group, ungroup
- validation: types, parent kinds, cycles, page boundaries; atomic batch rejection with
  path-qualified errors and op index
- each op returns its inverse (for undo)
- DocStore::apply reachable only from ops (trybuild compile-fail test)
- Invariant 1 property test: schema-valid after every accepted batch; rejected batches leave no trace

## FINAL
cargo test -p vellum-core ops:: · trybuild
```

---

# PROMPT 12 — Ops II: Component, Slot, Scale, Boolean, Align, Flip Ops

```text
Read: ARCHITECTURE §4.1 notes, §5; DESIGN.md C16, C29, C38.

## TDD FIRST
- create_component, toggle_component (element ⇄ component), detach (nested instances and variable
  links stay), replace_in_instance, make_slot (only empty frames inside a component; suggestions),
  instance structure restriction (inherited layers can't be reparented)
- scale: content, text, strokes, radii, spacing and effects together; anchor; bound sizes become
  literals; instances get overrides; Fill dimensions become fixed
- boolean (union/subtract/intersect/exclude), outline_stroke via PathOps
- align, distribute, flip (H/V), convert_script (placeholder until Prompt 26)
- every op: valid case, each invalid case, inverse

## FINAL
cargo test ops:: · fixtures for scale and slots
```

---

# PROMPT 13 — Guard: Leases, Pause Set, Limits

```text
Read: guide Invariant 4, §10.2; ARCHITECTURE §5 (Guard), §7.5.

## TDD FIRST
- pure guard(transaction, leases, pause_set, blocked_pairs, limits) → Allow | Reject(reason)
- people bypass leases; agents can't write inside another agent's lease; paused or stopped agents
  rejected; Stop-on-lease blocks (agent, frame) until resumed
- limits: max ops and max inserted nodes per agent transaction
- reasons are plain-language strings from @vellum/config ("Frame leased by {agent}")
- lease state machine (§10.2) as a table; expiry with injected Clock
- exhaustive table tests over actor kinds × lease states × pause states

## FINAL
cargo test guard:: lease::
```

---

# PROMPT 14 — DocStore On Loro: Transactions, Per-Actor Undo, Undo-Turn

```text
Read: ADR-004; refs/loro/crates/loro-internal/src/{undo.rs,handler/tree.rs,state/tree_state.rs};
refs/loro/context/{tree-events,loro-js-tree-diff}.md; ARCHITECTURE §4.2; guide Invariant 3.

## TDD FIRST
- adapters/doc-loro passes the DocStore contract suite
- our NodeIds map to Loro tree nodes; no Loro type escapes the adapter
- snapshot() equals what the codec would decode
- undo(actor) reverts only that actor's latest transaction; redo symmetric
- undo_turn(turn): reverts that turn's transactions; when later work by others depends on them,
  returns a Revert transaction instead (typed result)
- export/import updates round-trip between two stores (for Prompt 63)
- Invariant 3 property test: 3 actors interleaved, random undos

## FINAL
cargo test -p doc-loro · contract suite
```

---

# PROMPT 15 — Diff → Ops, Three-Way Merge, Merge-Driver Core

```text
Read: refs/graphite/editor/src/messages/portfolio/document/document_diff.rs; ARCHITECTURE §4.5;
DESIGN.md C22.

## TDD FIRST
- diff(old, new) → ops by node ID and property; apply(diff) to old == new (property test)
- moves detected as moves (stable IDs)
- merge(base, ours, theirs): clean when edits don't overlap; same-property conflicts produce a
  conflict marker node holding both versions
- merge(b, x, b) == x; merge(b, b, y) == y; commutative for non-conflicting edits
- fuzz target for merge inputs

## FINAL
cargo test diff:: merge:: · fuzz smoke
```

---

# PHASE 3 — Engine

---

# PROMPT 16 — Resolver I: Variables, Theme Axes, Aliases

```text
Read: ARCHITECTURE §4.1 (themes, aliases); DESIGN.md C17, C34.

## TDD FIRST
- theme resolution per axis: node theme overrides inherited; first axis value is default
- last matching value entry wins; alias chains resolve; cycles → typed error
- boolean variables drive visibility; type mismatches rejected at bind time
- cache by subtree with dirty-set invalidation; property test: cached == fresh

## FINAL
cargo test resolver::variables
```

---

# PROMPT 17 — Resolver II: Components, Instances, Overrides, Slots, Libraries

```text
Read: ARCHITECTURE §4.1 (overrides, slots), §4.4; DESIGN.md C16, C29, C36, S14, S15.

## TDD FIRST
- instance = component tree + overrides by NodeIdPath (props, replacement node, children)
- overrides win over component changes; nested instances
- slots: per-instance content; suggestions exposed to UI and agents
- libraries: imports by relative path; Missing resolution from cache with a flag (S14)
- paste between documents: link vs copies; variable clash → Use existing | Add renamed (S15)
- fixtures for every §7.4 parity row about components, slots and libraries

## FINAL
cargo test resolver::components resolver::libraries
```

---

# PROMPT 18 — Layout On Taffy + The Oracle Suite

```text
Read: ADR-002; refs/taffy/src/compute/{flexbox.rs,grid/}, docs/style-properties.md.

## TDD FIRST
- adapters/layout-taffy maps frames/sizing onto Taffy (row, column, grid, gap, padding, justify,
  align, hug/fill with fallbacks, absolute, layoutIncludesStroke); measure function from TextEngine
- incremental: only dirty subtrees re-layout (counter test)
- Invariant 7: oracle suite grown to ≥150 fixtures, all within 0.5 px
- hug-cycle detection (a frame can't hug children that depend on it)

## FINAL
layout oracle suite (chrome lane) · LayoutEngine contract
```

---

# PROMPT 19 — Text On Parley + FontSource

```text
Read: refs/parley/parley/src/layout/, fontique/src/{fallback,matching,source_cache}.rs;
refs/penpot/render-wasm/docs/texts.md; DESIGN.md C14, C37, S03.

## TDD FIRST
- adapters/text-parley: rich runs, bidi, CJK breaking, emoji, letter spacing, line height,
  underline/strikethrough, links as data, vertical alignment, justify; auto-width/auto-height/fixed
- FontSource order: document custom fonts → Google Fonts cache → system → bundled Inter;
  missing fonts reported with the substitute
- TextEngine contract suite; text fixtures from Prompt 5

## FINAL
cargo test -p text-parley · text fixtures
```

---

# PROMPT 20 — Scene: Spatial Index, Hit-Testing, Snapping

```text
Read: refs/excalidraw/packages/element/src/{collision,bounds}.ts;
refs/graphite/editor/src/messages/tool/common_functionality/{snapping,transformation_cage}.rs;
refs/kurbo/kurbo/src/.

## TDD FIRST
- adapters/index-rstar passes the SpatialIndex contract; incremental updates
- hit-testing: point and rect; deepest vs top-level; clip, lock and visibility respected
- snapping: edges, centres, equal spacing, pixel grid; candidates from the R-tree only
- find_space(width, height, near?) for agents and placeholders

## FINAL
cargo test scene:: · benchmark baseline (5k nodes)
```

---

# PROMPT 21 — Layout Check + Display List

```text
Read: ARCHITECTURE §3.2 (DrawCmd), §5 (Layout check, Display list).

## TDD FIRST
- layout check: text overflow, clipped children, overlapping siblings in auto-layout, nodes
  outside any frame, hug cycles — as structured problems (feeds snapshot_layout(problems_only))
- display list: resolved + laid-out tree → DrawCmd buffer for a viewport; culling from the R-tree;
  DISPLAY_LIST_VERSION; deterministic bytes for the same input

## FINAL
cargo test layout_check:: display_list::
```

---

# PROMPT 22 — Renderer I: Editor + Native Path, Solids, Strokes, Text, Images

```text
Read: ADR-001; refs/penpot/render-wasm/src/render/{surfaces,shape_renderer,strokes,text,images}.rs.

## TDD FIRST
- render-skia (native): PNG, multi-page PDF, SVG via the core's SVG writer
- editor renderer per ADR-001 behind the Renderer port
- solid fills, strokes (per-side widths, align, caps, joins, dashes), text glyph runs, images
  (fill/fit/stretch, crop), clips, opacity, transforms
- Renderer contract suite (TS) and render-skia tests

## FINAL
render tests (gpu lane) · Renderer contract
```

---

# PROMPT 23 — Renderer II: Gradients, Mesh, Blend Modes, Effects, Caches, Canvas2D, Golden Suite

```text
Read: refs/penpot/render-wasm/src/render/{fills,shadows,filters}.rs, docs/visual_regression_tests.md;
refs/vello/ARCHITECTURE.md (revisit condition only).

## TDD FIRST
- linear/radial/angular gradients, mesh gradients, 18 blend modes, drop/inner shadows, layer and
  background blur (shader effects stay behind the v1.1 flag)
- tile cache when zoomed out; picture cache per unchanged frame
- renderer-canvas2d: reference fake and S02 fallback (no effects, documented)
- golden suite (Invariant 6): fixtures × renderers; editor path vs headless path compared directly;
  per-fixture tolerance file; diff artifacts

## FINAL
golden suite · ARCHITECTURE §12 budgets measured and recorded
```

---

# PROMPT 24 — Files: Assets, Custom Fonts, Libraries On Disk, Directory Codec, Format Spec

```text
Read: ARCHITECTURE §4.3, §4.4; DESIGN.md S04–S06; ADR-004 threshold.

## TDD FIRST
- AssetStore (fs): content-hash files in <name>.assets/ (images, fonts, shaders); GC only on
  explicit save
- custom fonts (TTF, OTF, WOFF, WOFF2) imported into the document and served by FontSource
- *.lib.vellum kind; Turn into library (one-way); cache copies in ~/.vellum/cache/
- codec-json-dir: one file per page + index; switching codecs is lossless
- atomic writes (temp + fsync + rename); crash simulation never leaves a half file
- docs/format/ regenerated (schema, field reference, examples, migration notes)
- S06 threshold from ADR-004

## FINAL
codec + assets + library tests · spec diff check
```

---

# PROMPT 25 — ScriptRuntime: QuickJS Sandbox + Escape Corpus → ADR-005

```text
Read: refs/rquickjs/core/src/runtime/{base,raw}.rs, context/builder.rs;
refs/codex/codex-rs/code-mode/src/lib.rs; refs/opencode/packages/codemode/README.md;
guide Invariant 8, §6.1 D5.

## TDD FIRST
- adapters/script-quickjs passes the ScriptRuntime contract
- no network, fs, timers, async, host objects beyond the injected API; Math.random seeded per run
- limits: time (interrupt handler), memory (runtime limit), op count, node count → typed ScriptError
- same script → same ops in native and WASM
- escape corpus in testkit (fetch, import, prototype pollution, infinite loops, memory bombs)

## ADR
ADR-005-execute: QuickJS code mode, limits, determinism, the generated .d.ts approach.

## FINAL
script tests native + wasm · escape corpus
```

---

# PROMPT 26 — Script Nodes + The `execute` Doc API

```text
Read: ARCHITECTURE §4.1 (script nodes), §7.3 (`execute`); DESIGN.md C28;
refs/penpot/mcp/packages/server/src/tools/ExecuteCodeTool.ts (ideas only; MPL).

## TDD FIRST
`doc` API over Ops:
- page, get, query, insert, update, copy, replace, move, delete, setVariables, createComponent,
  makeSlot, instance, findSpace, layoutProblems, screenshot, export, fillImage, icon, claim, release
- one script = one transaction; failure → nothing applied, error with line + column
- validate_only dry run; doc.apiVersion; generated vellum-doc.d.ts
Script nodes:
- header parser (@schema, @input with number/string/boolean/color/enum/ref, args, defaults);
  inputs bindable to variables; output limits (1,000 nodes, 2 s); output derived (not saved, not
  in undo); Convert to layers = one transaction; errors carry an excerpt for the node

## FINAL
cargo test script_nodes:: doc_api:: · header parser fuzz target
```

---

# PHASE 4 — Daemon, Tools And Agents

---

# PROMPT 27 — Daemon I: Local IPC, Lifecycle, `vellum mcp` Stdio Bridge → ADR-003

```text
Read: refs/codex/codex-rs/stdio-to-uds/{README.md,src/lib.rs}, codex-rs/uds/src/lib.rs;
ARCHITECTURE §7.2; guide Invariant 9, §6.1 D3.

## TDD FIRST
- listener on ~/.vellum/run/vellum.sock (0700 dir, 0600 socket) or \\.\pipe\vellum-<user> with a
  user-only DACL; token in ~/.vellum/run/daemon.json; protocol version handshake
- socket inventory: no TCP listener unless --listen (token + Origin/Host checks required)
- single instance; graceful shutdown; crash restart by hosts
- `vellum mcp`: stdio ⇄ socket bridge; starts the daemon if needed

## ADR
ADR-003-daemon-lifecycle.

## FINAL
ipc lane (Unix + Windows) · clippy
```

---

# PROMPT 28 — Daemon II: Live/Headless Targets, File Watcher → Ops, Recovery Copies

```text
Read: ARCHITECTURE §7.1, §4.5, §6 Host behaviours; guide §7 Ownership.

## TDD FIRST
- DocumentTarget port: LiveTarget forwards transactions to the editor engine holding the file;
  HeadlessTarget loads, applies, renders, saves; one writer per file; second editor attaches or
  opens read-only
- file watcher: external change → diff → ops with Actor::File{reason}; self-writes ignored by hash;
  atomic-save renames handled
- recovery copies in ~/.vellum/recovery/ every few seconds; removed on save; listed on open (A06)

## FINAL
daemon integration tests · watcher tests with git checkout and merge fixtures
```

---

# PROMPT 29 — Tool Registry, MCP Adapter, Read Tools

```text
Read: refs/rmcp/crates/rmcp/README.md, examples/servers/src/{counter_stdio,structured_output,progress_demo}.rs;
ARCHITECTURE §3.4 (Tool), §7.3; refs/penpot/mcp/packages/server/src/tools/HighLevelOverviewTool.ts (ideas only).

## TDD FIRST
- Tool { name, version, input_schema, run }; MCP and CLI adapters generated from the registry
- read tools: get_app_state (open documents, active document, selection, viewport, leases, paused
  agents, Notes for agents and context nodes in scope), open_document, get_nodes, query_nodes,
  snapshot_layout, find_properties, get_variables, list_components, list_fonts, find_space,
  screenshot
- MCP tool schema snapshot test (ARCHITECTURE §2.5)

## FINAL
registry tests · MCP integration over stdio (headless and live with a fake editor)
```

---

# PROMPT 30 — Write Tools, `execute` Tool, Batch Errors

```text
Read: ARCHITECTURE §7.3; DESIGN.md "Step Timeline" (what steps show).

## TDD FIRST
- execute (doc API, validate_only), insert_nodes (JSON and compact JSX-like spec; parsers in a
  registry), update_nodes, copy_nodes, replace_nodes, move_nodes, delete_nodes,
  replace_properties, set_variables, create_component, create_instance, make_slot, claim_frame,
  release_frame, report_progress
- every mutating tool: one transaction, one undo step, created IDs returned, Guard applied
- batch errors: op index, path, plain reason; agents can fix and retry
- step descriptors (verb, target, counts, thumbnail request) emitted for the Agent panel

## FINAL
tool tests · scripted-agent E2E on the stub endpoint (after Prompt 32 exists, tag pending)
```

---

# PROMPT 31 — Skills And Styles, read_skill/get_style, Parity-Suite Harness

```text
Read: refs/codex/docs/skills.md, codex-rs/skills/src/lib.rs;
refs/penpot/docs/mcp/{good-prompting-practices-design,prompting-token-aware}.md (ideas only);
ARCHITECTURE §3.4 (SkillSource, StyleLibrary), §7.4.

## TDD FIRST
- SkillSource stacking: builtin → design-system guidelines → user SKILL.md folders
- written from scratch (Invariant 14): format.md, execute.md (+ vellum-doc.d.ts), workflows
  (design a screen, build a design system, recreate from code, make a chart with a script)
- StyleLibrary: 6 built-in styles (palette, type, radius, density, description)
- tools: list_skills, read_skill, get_style
- parity-suite harness: every ARCHITECTURE §7.4 row marked v1 maps to a named test; the harness
  fails if a row has none (rows are filled by later prompts)

## FINAL
skills/styles tests · parity harness reports coverage
```

---

# PROMPT 32 — ModelProviders, SecretStore, Stub Model Endpoint

```text
Read: refs/goose/crates/goose/src/providers/{base,anthropic_def,openai_def,openrouter_def,bedrock,gcpvertexai}.rs;
refs/opencode/packages/ai/src/{provider,providers}.ts; refs/codex/codex-rs/keyring-store/src/lib.rs;
ARCHITECTURE §3.4; guide Invariant 10.

## TDD FIRST
- adapters: openai-compat (OpenRouter default deepseek/deepseek-v4.1-flash; DeepSeek, xAI,
  Moonshot/Kimi, Together, Fireworks, Z.AI, OpenCode, Ollama, LM Studio, custom), openai-responses,
  anthropic (API, Bedrock, Vertex, Foundry, custom Messages), google
- streaming, tool calls, vision where supported, reasoning effort; provider status values
- SecretStore (keyring) + env fallback for CI (VELLUM_*_API_KEY)
- testkit stub endpoint replays scripted completions deterministically
- canary test skeleton (Invariant 10)

## FINAL
provider tests against the stub · keychain tests (tagged per OS)
```

---

# PROMPT 33 — Built-In Engine Loop, Context Management

```text
Read: refs/goose/crates/goose/src/agents/{agent,reply_parts,prompt_manager,large_response_handler}.rs,
context_mgmt/mod.rs; refs/opencode/packages/core/src/session/compaction.ts; ARCHITECTURE §8.

## TDD FIRST
- engine-builtin: PromptPack + skills → tool calls through ToolBridge → screenshot self-checks
- large tool results truncated/summarized with an explicit marker, never silently dropped
- interrupt within 3 s (Invariant 12)
- AgentEngine contract suite; fake engine for UI work

## FINAL
engine E2E on the stub (single-frame requests)
```

---

# PROMPT 34 — Session Store, Usage And Cost, Turn State Machine → ADR-006

```text
Read: refs/codex/codex-rs/{rollout/src/lib.rs,thread-store/README.md};
refs/goose/crates/goose/src/session/{session_manager,session_naming}.rs; guide §10.1; ARCHITECTURE §7.6.

## TDD FIRST
- sessions-sqlite: append-only events per session (messages, plans, steps with previews and
  thumbnail refs, usage, origin IDs); keyed by repo root + repo-relative path; delete cascades
- thumbnails in ~/.vellum/cache/ by content hash
- usage per kind; estimated cost from pricing.json (micro-USD, labelled estimate)
- turn state machine (§10.1) as a pure table; stuck flag after 120 s
- external MCP sessions recorded (tool calls only)

## ADR
ADR-006-sessions.

## FINAL
cargo test sessions:: turns::
```

---

# PROMPT 35 — PermissionBroker

```text
Read: refs/goose/crates/goose/src/permission/{permission_inspector,permission_judge,permission_store}.rs;
refs/opencode/packages/core/src/{permission.ts,permission/saved.ts}; refs/codex/docs/sandbox.md;
ARCHITECTURE §7.7; guide Invariant 11.

## TDD FIRST
- requests: read file in repo, write file, run command, read outside repo, network for built-in
- decisions: Allow once, Allow for this session, Deny; policy rules; engine auto mode; skip-all
  (off by default, persistent warning flag)
- Invariant 11 property test over generated agent actions; decisions stored in the session store
- canvas edits never prompt (test)

## FINAL
cargo test permissions::
```

---

# PROMPT 36 — Installed Engine: Claude Code → ADR-007

```text
Read: refs/goose/crates/goose/src/providers/claude_code.rs; Claude Code docs for headless mode and
MCP config (verify against the installed version, never from memory); guide §6.1 D4.

## TDD FIRST
- engine-claude-code (ported with attribution where taken from Goose): detect install and sign-in,
  run headless with `vellum mcp` attached, stream structured events into steps, interrupt, usage
- statuses: Installed · signed in | not signed in (S16) | usage limit (S09)
- scripts/record-engine-fixtures.sh records transcripts per installed version (engines lane);
  unit tests replay them

## ADR
ADR-007-installed-engines (Claude Code section).

## FINAL
transcript replay tests · engines lane E2E (nightly)
```

---

# PROMPT 37 — Installed Engine: Codex

```text
Read: refs/goose/crates/goose/src/providers/{codex,codex_acp}.rs; refs/codex/codex-rs/app-server/README.md,
codex-rs/app-server-protocol/src/protocol/; refs/codex/docs/{exec,config}.md.

## TDD FIRST
- engine-codex: app-server protocol first, `exec` fallback when unavailable; same event mapping,
  interrupt, usage, statuses (incl. ChatGPT sign-in)
- transcripts recorded and replayed as in Prompt 36

## ADR
ADR-007 Codex section.

## FINAL
transcript replay tests · engines lane E2E (nightly)
```

---

# PROMPT 38 — Plans, Placeholders, Leases In Orchestration

```text
Read: ARCHITECTURE §7.5; DESIGN.md "Plan card", "Agent Presence" (placeholders); guide §10.

## TDD FIRST
- requests over the plan threshold produce a plan event (frames, pages, intents); Edit request
- Build creates placeholders: lease records with planned bounds from find_space, visible to all
  agents via get_app_state; abandoned plans leave nothing in the file
- report_progress ticks plan items; plan state in the session store

## FINAL
orchestration tests on the fake engine
```

---

# PROMPT 39 — Parallel (Split, Side By Side, Explore), spawn_agents, Stop/Pause, Undo This Turn

```text
Read: ARCHITECTURE §7.5; guide Invariants 3, 4, 12.

## TDD FIRST
- Split: plan items divided into lanes; each lane its own engine run with its own leases and hue
- Side by side: same brief, separate areas, optional model per lane
- Explore: N successive variants varying only Layout or only Style
- spawn_agents for built-in engines; external clients gated by setting
- Stop (session), Stop this agent, Stop all, Pause all → Guard state + engine interrupts within
  Invariant 4 / 12 budgets
- Undo this turn via origin turn ID (Invariant 3)
- parity-suite rows for parallel, explore, spawn_agents filled

## FINAL
orchestration tests · engines lane parallel E2E (nightly)
```

---

# PHASE 5 — Import And Export

---

# PROMPT 40 — Tokens, Spec, Stack Profiles

```text
Read: ARCHITECTURE §3.5; DESIGN.md C19, C20; W3C DTCG spec.

## TDD FIRST
- TokenFormat: css-vars, tailwind-v4 (@theme), dtcg-json; theme axes → selectors (:root, .dark,
  [data-theme], [data-device]) with a person-editable mapping; import diff (added, changed, removed)
- token round-trip: export → import → identical variables
- SpecFormat layout-spec-v1 for get_spec; StackProfiles (shadcn, react-native, swiftui) as data
- tools: get_spec, import_tokens, export_tokens

## FINAL
cargo test tokens:: spec::
```

---

# PROMPT 41 — HTML Export, Raster/PDF/SVG Export

```text
Read: ARCHITECTURE §3.5; DESIGN.md C19, C21.

## TDD FIRST
- CodeExporter html-tailwind and html-css; scaffold on/off; assets inline or files; output rendered
  in headless Chrome and compared to the golden (chrome lane)
- ImageExporter png, jpeg, webp (1–3×), pdf (selection order), svg; multi-select raster → zip
- export tool; copy as PNG / HTML data for the editor

## FINAL
export goldens
```

---

# PROMPT 42 — Importers (SVG, Image, Figma Paste), Icons, Stock Images → ADR-009

```text
Read: ARCHITECTURE §3.6; DESIGN.md C30, C31 (Figma paste note).

## TDD FIRST
- Importer svg (editable vectors), image (png/jpeg/webp), figma-clipboard (layers and text; no
  images, with the note)
- IconLibrary: lucide, material-symbols (outlined/rounded/sharp), phosphor, feather, project folder;
  search index; weight; icon nodes; tools search_icons, insert_icon
- ImageSource stock with attribution in node metadata; fill_image tool; `import` tool

## ADR
ADR-009-stock-images (provider and licence permitting shipped products).

## FINAL
import fixtures · parity rows for icons and images filled
```
---

# PHASE 6 — Editor UI

Every prompt in this phase starts with the same **design read** (guide §2.1). It is written out
in full once here and referenced as `DESIGN READ` below.

```text
DESIGN READ
1. scripts/design-check passes (snapshots current; every frame this prompt names exists).
   If a named frame is missing from the inventory, stop and list what is missing.
2. scripts/design-open; in the session, get_app_state confirms .design-cache/vellum.readonly.pen.
3. read_skill (and its execute page) — learn the current read syntax; never hardcode it.
4. For each named frame (Light, Dark, High contrast where it exists): read the node tree, the
   components it uses (and their variants), and the variables bound in it; take a screenshot.
5. Note any mismatch with DESIGN.md (copy, states, missing elements) in docs/design/DESIGN-GAPS.md.
6. Build from components and tokens, not pixels: if a value isn't a variable or component
   property in the design, ask before inventing a token.
7. Before finishing: screenshot the implementation at the same size and theme as each frame and
   compare with the snapshot in docs/design/snapshots/; goldens use those snapshots with the
   documented tolerance.
Read-only throughout: no insert/update/move/delete/replace operations, no spawn_agents, no save.
```

---

# PROMPT 43 — Design Tokens Generated From The .pen Variables, Contrast And Usage Lints

```text
Stop if docs/design/DESIGN-DECISIONS.md has UNDECIDED entries this phase needs.

DESIGN READ: page "Tokens" (all frames), the variables collections and the "Mode" theme axis;
page "Components" overview frame.

## TOKENS PIPELINE
- scripts/design-tokens (read-only MCP or headless pen CLI on the read-only copy) exports every
  variable (name, type, value per Mode) to packages/ui-tokens/source/tokens.json with the design
  file's sha256; generated outputs: CSS variables (chrome and canvas groups, light/dark/HC) and a
  TS theme object
- CI check: tokens.json source hash equals inventory.source_sha256; a stale tokens.json fails
- any token DESIGN.md defines but the design file lacks (or vice versa) → DESIGN-GAPS.md + failing
  test until a human resolves it

## LINTS
- contrast: every text/background pair ≥ 4.5:1 including 11–12 px text; icons and focus ≥ 3:1
- usage: selection blue and component violet never on buttons; agent hues only on agent markers,
  cursors, leases, placeholders and session dots; no hardcoded colour literals in UI packages

## FINAL
token generation reproducible · contrast + usage lints · Storybook token page vs "Tokens" snapshots
```

---

# PROMPT 44 — editor-core I: Tool State Machines

```text
Read: refs/graphite/editor/src/messages/tool/tool_messages/{select,pen,path,shape,text,gradient}_tool.rs,
tool/common_functionality/{snapping,transformation_cage,resize,shape_editor}.rs;
DESIGN.md "Keyboard" (Tools, Path editing, Select).

## TDD FIRST (no React; editor-core only)
- tools as state machines: move/select (click, ⌘-click deepest, ⌘-drag marquee, ⇧-click, Enter,
  ⇧Enter, Esc), scale (K, anchor, factor), frame, rect/ellipse/line/polygon/star, pen + path edit
  (select points, cut, connect, smooth/corner, symmetric handles, bend, straighten, add point),
  text, hand (and Space), zoom (hold Z), note
- every tool emits Op transactions with Origin Person; transforms preview locally then commit once
- snapping integration via Scene; overlay model (handles, guides, measurements)

## FINAL
editor-core tool tests (every transition) · no React import (dependency-cruiser)
```

---

# PROMPT 45 — editor-core II: Commands, Keybindings, Registries, Host/EngineClient Ports, Harness

```text
Read: refs/graphite/editor/src/messages/input_mapper/input_mappings.rs; ARCHITECTURE §3.3;
DESIGN.md "Keyboard" (every table and the host-conflict rules).

DESIGN READ: page "Components" — every component frame and its variants (to scaffold the harness).

## TDD FIRST
- command registry + keybinding table with every row from DESIGN.md "Keyboard"; IDE vs desktop
  variants; host-conflict rules; rebinding and conflict detection
- registries: registerTool, registerCommand, registerPanel
- ports: Host (vscode, electron, browser, fake), EngineClient (wasm, fake), DaemonClient (relayed),
  SyncClient (noop), Telemetry (noop) — each with a TS contract suite
- visual harness: Storybook with one story per component × variant × theme, fake-engine mode;
  goldens against docs/design/snapshots/ component frames

## FINAL
every-shortcut test · port contracts · component goldens (initially pending until built)
```

---

# PROMPT 46 — UI: Floating Chrome, Canvas Interaction, Layers And Assets

```text
DESIGN READ: "C03 Agent workspace" (chrome only), "C18 Layers and Assets", "S13 Narrow editor"
(960 and 760); components "Floating chrome / *", "Layers row", "Actor Marker / *".
Reference: refs/excalidraw/packages/excalidraw/renderer/{staticScene,interactiveScene}.ts.

Build (editor-react on editor-core):
- document bar, tool rail, top-right bar, bottom strip, zoom pill; responsive rules (960 / 1040 /
  1200 px); Tab and ⌘\ hide panels
- canvas host: content renderer + overlay canvas (selection, handles, guides, measurements)
- Layers tab (tree, rename, reorder, visibility, lock, recent-agent marker), Assets tab
  (components, icons, styles, libraries, design systems)

TDD: component tests per state; Playwright (browser host, fake daemon): draw, select, reorder,
hide panels; per-person undo. Goldens vs snapshots at 1280, 960, 760; light/dark/HC.
```

---

# PROMPT 47 — UI: Properties, Fills/Strokes/Effects, Text, Components

```text
DESIGN READ: "C13 Frame with auto-layout selected", "C14 Text editing", "C16 Components and
variants", "C33 Fill and effects"; components "Property controls / *", "Paint editor / *",
"Variable chip", "Component property row".

Build:
- right panel Design tab: Notes for agents (collapsed when empty), Position, Layout, Appearance,
  Fill, Stroke, Effects, Text, Component/instance (Go to main, Replace inside instance, Detach),
  Export section
- paint editor: solid, gradients incl. mesh (on-canvas points), image (fit, crop handles), blend
  modes, visibility; effects rows; stroke per-side widths, align, caps, joins, dashes
- value fields: scrub, arrows, expressions, mixed, bound-to-variable chip

TDD: every control state; Playwright: auto-layout edits match the oracle; components and
overrides; mesh gradient editing. Goldens vs snapshots.
```

---

# PROMPT 48 — UI: Variables And Themes, Libraries, Fonts, Scale, Vector Editing

```text
DESIGN READ: "C15 Vector editing", "C17 Variables", "C34 Themes", "C36 Libraries",
"C37 Custom fonts", "C38 Scale", "S14 Missing library", "S15 Paste conflicts".

Build: Variables dialog with collections, theme axes as grouped columns, aliases; Theme section on
frames; Libraries (import, missing with Locate, Turn into library confirm); paste dialogs (link or
copies; Use existing / Add renamed); font picker with custom fonts and Manage fonts; Scale panel;
path editing UI.

TDD: Playwright: two-axis themes render correctly per frame; library missing → locate → saved;
scale converts bound values and creates overrides. Goldens vs snapshots.
```

---

# PROMPT 49 — UI: Agent Panel Shell, Sessions, Turns, Step Timeline

```text
DESIGN READ: "C03 Agent workspace (building)", "C06 Sessions and external agents",
"C09 Turn details and undo"; components "Agent Panel / *", "Step row / *", "Turn footer".

Build: Agent tab shell; session bar + switcher (Running / Needs you / Recent), New, delete;
turns (your message with chips, agent header single/stacked, prose, Step Timeline with designed
thumbnails → zoom-and-select, screenshot checks, layout checks, ran-script reveal, grouping, Retry
step), turn footer (usage, Review changes, Undo this turn with confirm, Retry); external session
view; Latest pill.

TDD: streaming never jumps scroll when scrolled up; undo-turn removes only that turn; external
session shows the composer replacement line. Playwright with fake engines replaying Prompt 36–37
transcripts. Goldens vs snapshots.
```

---

# PROMPT 50 — UI: Composer, Skills Menu, Model Picker, Usage, Permission Card, Inline Prompt

```text
DESIGN READ: "C01 First open", "C08 Steering with selection", "C11 Engine and model",
"C26 Skills menu", "C27 Permission card"; components "Composer / *", "Permission card / *".

Build: composer (chips, @ mentions for files/frames/components/variables, / skills menu with Add
skill…, attach, Parallel with mode, Explore, engine/model/effort picker with provider setup and
status rows, usage meter + popover with "Estimated cost at API rates", Send ↔ Stop with Stop this
agent / Stop all, queue-or-stop prompt, prompt history ↑/↓, draft per session); first-open prompt
with suggestion chips; inline prompt (⌘K desktop, ⌥K IDE) and selection toolbar Ask; Permission
card (Allow once / for session / Deny) and the skip-all warning line.

TDD: permission card blocks the turn until decided; Stop visible whenever any agent works; keys
never rendered after entry. Goldens vs snapshots.
```

---

# PROMPT 51 — UI: Plans, Parallel, Alternatives, Presence Layer, Collaboration Nodes

```text
DESIGN READ: "C04 Plan card", "C05 Parallel build", "C07 Alternatives", "C35 Notes, context and
prompts on the canvas"; components "Plan card", "Agent Presence / *".

Build: Plan card (editable items, Parallel stepper with mode, Build, Edit request); placeholders
on canvas; parallel turn lanes with progress; alternatives (Side by side, Explore) with Keep;
presence stack + menu (Follow, Open session, Stop, Pause all), cursors and name tags, leases with
Stop and rejected counts, paused banner, follow badge, insert highlight; canvas note, context and
prompt nodes (Run).

TDD: presence never relies on colour alone (labels present); your edits inside a lease are accepted
and the tab updates. Goldens vs snapshots.
```

---

# PROMPT 52 — UI: Notes For Agents, Guidelines, Script Nodes, Slots, Image Fill

```text
DESIGN READ: "C12 Notes for agents and guidelines", "C28 Script node", "C29 Slots", "C30 Images".

Build: Notes for agents section; Guidelines editor (design-system guidelines.md); script node
(rendering, error with excerpt, empty), Script section with generated input controls and Convert
to layers; slot marker and suggested components (one-click insert); image fill tabs (Upload,
Stock with attribution; Generate shown as v1.1-disabled).

TDD: script errors render on the node and in the panel; slot suggestions insert instances. Goldens.
```

---

# PROMPT 53 — UI: History, Inspect, Token Sync, Export, File Changes

```text
DESIGN READ: "C10 History", "C19 Inspect", "C20 Token sync", "C21 Export", "C22 File changed on disk".

Build: History tab (entries, expand to ops with before → after, Select / Zoom / Undo / Revert /
Copy JSON, filters by actor/session/page, footer); Inspect with stack selector, Copy prompt for your
coding agent, Implement with…, HTML export with settings; Token sync dialog with diff; Export dialog;
Copy as PNG / HTML; external-change banner; conflict node + resolver with Ask agent to merge.

TDD: revert creates a new attributed entry; token import shows the exact diff. Goldens vs snapshots.
```

---

# PROMPT 54 — UI: Palette, Context Menu, Settings, Edge States

```text
DESIGN READ: "C23 Command palette", "C24 Context menu", "C25 Editor settings", and every frame on
page "Edge States" (S01–S16).

Build: command palette with the "Ask the agent: …" row; context menu with Ask about this… and
Redesign this…; settings (appearance, agents, permissions, skills, assets, canvas preferences,
renderer, language, telemetry with exact disclosure, shortcuts with conflicts); every edge state
with exact copy (Appendix B).

TDD: global state matrix — every panel × {loading, empty, offline daemon, read-only, basic
renderer, missing fonts, missing library} with exact copy; goldens light/dark.
```

---

# PHASE 7 — Hosts And CLI

---

# PROMPT 55 — VS Code Extension I: Custom Editor, Save/Revert/Backup, Commands

```text
Read: refs/vscode-samples/custom-editor-sample/src/{pawDrawEditor,extension}.ts, media/pawDraw.js;
ARCHITECTURE §6 Hosts and Host behaviours; DESIGN.md "Keyboard" host conflicts.

Build apps/extension:
- CustomEditorProvider for *.vellum and *.lib.vellum; webview with the editor bundle and a strict
  CSP; host-vscode; save, revert, backup/hot exit; dirty state
- relays the daemon protocol from the extension host; starts the bundled daemon if missing
- commands: New Design File, Toggle Design / Text View, Open Welcome File, Pause All Agents
- theme kind mapping + Match editor colours

TDD: @vscode/test-electron (vscode lane): open, edit, save, revert, hot exit, external git checkout
applied as ops, two editors on one file attach, keybinding conflicts.
```

---

# PROMPT 56 — VS Code Extension II: Connect Agents, Sidebar, Packaging And Publishing

```text
DESIGN READ: "C02 Connect agents".
Read: refs/vscode-samples/mcp-extension-sample/src/extension.ts.

Build: Connect agents dialog — detect and write/remove MCP entries for Claude Code, Codex (also
ChatGPT desktop and the Codex IDE extension), Cursor, Windsurf, VS Code Copilot, Gemini CLI,
Antigravity, OpenCode, Kiro, Claude Desktop; show exactly what was written and where; Disconnect
removes only Vellum's entry; Activity Bar sidebar (recent design files, sessions); packaging with
vsce + ovsx, platform-specific VSIX bundling the daemon.

TDD: config writers are idempotent and never touch other entries (fixture config files per client).
```

---

# PROMPT 57 — Desktop I: Electron Shell, Windows, Tabs, Menus, Security

```text
DESIGN READ: "A03 Editor window with tabs".
Read: Electron security docs (contextIsolation, sandbox, utilityProcess).

Build apps/desktop: windows per folder, document tabs, native menus (File/Edit/View/Object/
Arrange/Agents/Help), daemon as a sidecar, host-electron via contextBridge (contextIsolation on,
sandbox on, nodeIntegration off).

TDD: Playwright-Electron: open folder, tabs, menus; security test — renderer can't reach Node or
the socket directly.
```

---

# PROMPT 58 — Desktop II: Welcome, Home, Drafts, Recovery, Agents, Notifications, Updater, Packaging

```text
DESIGN READ: "A01 Welcome", "A02 Home", "A04 Agents", "A05 Sign in", "A06 Recovered changes".

Build: Welcome; Home (Recents / Drafts / Design systems, list/grid, sort, multi-select, drop to
open); drafts with auto-save and Save As; Recovered changes dialog; Agents settings page; OS
notifications for turn end / failure / permission; electron-builder for macOS (arm64, x64; signed,
notarized), Windows (x64, arm64; signed), Linux (AppImage, deb, tarball; x64, arm64);
electron-updater with signature verification and Check for Updates.

TDD: recover after kill -9; update rollback on bad signature. Goldens vs snapshots.
```

---

# PROMPT 59 — CLI Completeness And The Interactive Shell

```text
Read: ARCHITECTURE §7.8; DESIGN.md "Terminal output (CLI)".

Build: prompt (engine, model, effort, attach, repo, parallel + mode, usage json, preview each
change, max failed calls, export + scale, verbose), --tasks (runs every task), models, version,
status, new, export (all formats), tokens import/export; shell (live attached or headless with
save()); --json everywhere; NO_COLOR; non-zero exit on any failure; CI usage docs (env keys, no
account).

TDD: CLI golden outputs (text and --json); exit codes.
```

---

# PROMPT 60 — `vellum diff`, PR Preview Action, Merge-Driver Install

```text
Read: ARCHITECTURE §4.5, §9 (PR previews).

Build: `vellum diff main..HEAD` → before/after PNGs per changed frame; `vellum git setup` writes
.gitattributes and git config for the merge driver; a GitHub Action that renders changed frames
and comments on the PR (images as workflow artifacts; hosted previews only with Pro).

TDD: merge driver in a real git repo fixture (clean merge, conflict node); diff output goldens.
```

---

# PHASE 8 — Cloud (Pro)

---

# PROMPT 61 — Accounts, Billing, Entitlements, Device Login

```text
Read: ARCHITECTURE §3.7, §9; PRODUCT §6.

Build: apps/account (BetterAuth incl. device-code for `vellum login`), @vellum/billing on Polar.sh,
webhooks → entitlements (cloud_sync, share_links, pr_previews, premium_systems); apps/api (Hono +
Bun) with /v1/ endpoints for CLI and extensions; JobQueue (BullMQ) workers running the `vellum`
binary for exports, thumbnails and hosted PR previews; Mailer on SES.

TDD: entitlement matrix — every local feature works signed out; Pro gates only cloud features.
```

---

# PROMPT 62 — Share Snapshots And The Web Viewer

```text
DESIGN READ: "W01 Share link viewer", "W02 Share dialog" (every state), "W03 Account and plan".

Build: share flow in the daemon (dependency check with missing-files warning, chunked resumable
upload that continues if the dialog closes, Update snapshot, Delete share link); ObjectStore on
R2; web viewer (read-only canvas via vellum-wasm + host-browser, Inspect, Download ZIP); account
and plan pages.

TDD: snapshot round-trip (upload → view → download → open → identical document); revoke works;
incomplete snapshots are marked. Goldens vs snapshots.
```

---

# PHASE 9 — v1.1

---

# PROMPT 63 — Sync Server

```text
Read: ARCHITECTURE §9, §3.7 (UpdateLog, SnapshotStore, Presence); refs/loro/docs/{encoding,shallow-snapshot-concurrency}.md.

Build crates/sync-server: rooms per document; Loro updates merged, persisted, broadcast; .vellum
snapshots on compaction; permissions enforced per update; Postgres/Redis/R2 adapters and
SQLite/disk/in-memory adapters.

TDD: convergence property tests (n clients, random ops, partitions); viewer writes rejected; a room
rebuilt from .vellum snapshots alone.
```

---

# PROMPT 64 — Multiplayer Presence, Comments, Self-Host Image

```text
DESIGN READ: any multiplayer and comment frames added to the design file for v1.1 (stop if none).

Build: SyncClient in the editor; presence for people (circles) and agents (rounded squares);
comments (Team); share dialog roles (Viewer, Commenter, Editor); self-host Docker image.

TDD: self-host image and cloud pass the same E2E; two people + two agents in one document.
```

---

# PROMPT 65 — BrowserPort, `browser` Tool, HTML Importer, Embed Node

```text
DESIGN READ: "C31 Import".

Build: BrowserPort (Electron's Chromium on desktop; chromiumoxide headless in the daemon); `browser`
tool (screenshot, import, import_element, localhost); html importer (DOM + computed styles →
frames, text, images); embed node.

TDD: import fidelity vs Chrome screenshots on fixture pages; embed renders and respects device size.
```

---

# PROMPT 66 — Figma File Import, Image Ops, Image Generation

```text
DESIGN READ: "C30 Images" (Generate tab, image ops state), "C31 Import" (Figma file state).

Build: figma-api importer (REST API with the user's token in SecretStore; summary of what didn't
convert); ImageOps (background removal via a local model, tracing via vtracer); image generation
through an image-model adapter with the provider in settings.

TDD: import fixtures; image ops are undoable transactions; generated images marked in metadata.
```

---

# PROMPT 67 — Shaders And The Examples Gallery → ADR-008

```text
DESIGN READ: "C33 Fill and effects" (Shader parts).

Build: ShaderCompiler (our dialect → SkSL; GLSL import decision in ADR-008), shader fills and
effects with uniforms bound to variables, errors shown on the node, examples gallery (shaders,
scripts, design systems).

TDD: compile errors are typed and shown; runaway shaders are bounded; goldens for each example.
```

---

# PROMPT 68 — Slides And Presenting

```text
DESIGN READ: "C32 Slides".

Build: slides page kind, slide strip with reorder, Present (⌘Enter) with arrows/Space/Enter/Esc,
PDF export of all or selected slides in slide order.

TDD: PDF page order and page sizes match frames; presenting never mutates the document.
```

---

# PHASE 10 — Release

---

# PROMPT 69 — Security Hardening, Threat Model, Fuzzing

```text
- docs/architecture/threat-model.md: malicious .vellum file, malicious script node, prompt
  injection via documents/skills/context nodes, malicious library, compromised MCP client, local
  process attacking the daemon, webview escape, update tampering, share-link leakage
- checklist and tests: socket permissions, token handling, webview CSP, Electron hardening, script
  escape corpus, prompt-injection corpus, cargo-fuzz (codecs, merge driver, op application, script
  header, daemon frames), canary scan, supply chain (cargo-deny, pnpm audit, SBOMs, signing)
```

---

# PROMPT 70 — Accessibility, i18n And RTL

```text
- WCAG AA in light, dark, high contrast; Layers as the canvas's accessible tree; keyboard canvas
  mode; polite announcements for turn and step completion; reduced motion
- 13 languages; chrome and agent conversation mirror in Arabic; canvas, numbers, code and paths
  do not; goldens for Arabic at 1280
```

---

# PROMPT 71 — Performance And Observability

```text
- ARCHITECTURE §12 budgets in CI against every adapter; WASM size budget; daemon idle RSS budget;
  open-2 MB and 5k-node pan/zoom benchmarks
- telemetry off by default in editor, CLI and daemon; opt-in shows exactly what is sent; cloud on
  Sentry/PostHog with scrubbing (no document content, prompts or keys)
- penpot render-wasm/docs/visual_regression_tests.md reviewed for the golden suite's flake controls
```

---

# PROMPT 72 — CI/CD, Release Pipelines, Release Blockers

```text
## CI/CD
- ci.yml: fmt · clippy · cargo test · deny · deps rules · clean-room · types:gen diff · schema diff ·
  design-check (snapshots and tokens current) · pnpm lint/type-check/test · layout oracle · golden
  suite (gpu lane) · parity suite · canary · engines lane (nightly) · vscode lane · electron lane
- release: signed CLI/daemon binaries (macOS, Windows, Linux; x64 + arm64), signed Electron builds,
  VSIX per platform to Marketplace + Open VSX, npm wrapper @vellum/cli, Homebrew tap

## RELEASE BLOCKERS (all must be false)
- any document mutation not through an Op transaction, or a transaction without an origin
- ⌘Z ever undoing another actor's change
- an agent write accepted inside another agent's lease or while paused
- load → save not byte-identical on any fixture
- editor vs headless render over tolerance on any golden
- layout oracle failures
- any script escape or nondeterminism
- any TCP listener without --listen, any network call not configured by the person, telemetry on
  by default, or a local feature needing an account
- canary key found anywhere
- an agent action outside the canvas without a permission decision
- Stop p95 over 3 s
- any §7.4 v1 parity row without a passing test
- any screen missing its explicit edge states, or any UI golden failing against the design snapshots
- stale design snapshots or tokens (design-check), or an open DESIGN-GAPS.md entry marked blocking
- any clean-room check failure
```

---

# Appendix A — pen.dev Parity → Vellum Mapping (test oracle for the parity suite)

ARCHITECTURE §7.4 is authoritative; this table adds where each row is implemented and tested.

| pen.dev capability (public docs) | Vellum surface | Prompts | Test |
|---|---|---|---|
| `execute` script tool | `execute` + `doc` API | 25, 26, 30 | parity suite, escape corpus |
| `get_app_state` | `get_app_state` | 29 | parity suite |
| `read_skill` | `read_skill`, `list_skills` | 31 | parity suite |
| `get_style` | `get_style` | 31 | parity suite |
| `spawn_agents` | `spawn_agents` | 39 | orchestration tests |
| `browser` | `browser` (v1.1) | 65 | html import fidelity |
| Earlier batch ops (insert, copy, update, replace, move, delete, image) | typed write tools, `fill_image` | 30, 42 | parity suite |
| Layout snapshot incl. problems only | `snapshot_layout` | 21, 29 | layout-check fixtures |
| Find empty space | `find_space` | 20, 29 | scene tests |
| Search / replace properties | `find_properties`, `replace_properties` | 29, 30 | parity suite |
| Integrated agent, sessions, effort, Stop | Agent panel, engines | 33, 34, 49, 50 | E2E |
| Claude Code / Codex as agents | installed engines | 36, 37 | engines lane |
| Skills via `/`, custom SKILL.md | SkillSource, `/` menu | 31, 50 | E2E |
| Permission requests, auto mode, skip all | PermissionBroker, Permission card | 35, 50 | property test, E2E |
| Context usage, estimated cost | usage meter | 34, 50 | E2E |
| Parallel agents (Split, Side by side, per-agent model) | Parallel modes | 39, 51 | orchestration tests |
| Let it cook (layout/style variants) | Explore | 39, 51 | orchestration tests |
| Script nodes | `script` node | 26, 52 | script tests |
| Slots with suggestions | `slot` | 12, 17, 52 | resolver tests |
| Theme axes, frame themes, aliases, boolean vars | variables + themes | 16, 48 | resolver tests |
| Libraries, paste link/copies, variable conflicts, Locate | libraries | 17, 24, 48 | library tests |
| Mesh gradients, image crop, 18 blend modes, effects | paints/effects | 23, 47 | goldens |
| Shader fills/effects + gallery | ShaderCompiler | 67 | shader tests |
| Text links, underline, strike, vertical align | text node | 19, 47 | text fixtures |
| Notes, prompt nodes, context nodes | collaboration nodes | 7, 51 | E2E |
| Browser node | `embed` | 65 | E2E |
| Custom fonts with the file | fonts + assets | 19, 24, 48 | font tests |
| Scale tool | `scale` op + K | 12, 44, 48 | op tests, E2E |
| Icons (Material Symbols, Lucide, Feather, Phosphor) | IconLibrary | 42 | icon tests |
| HTML + Tailwind / CSS export, scaffold | CodeExporter | 41, 53 | Chrome lane goldens |
| PNG/JPEG/WEBP/PDF export, copy as PNG/HTML | ImageExporter | 41, 53 | export goldens |
| Figma paste / file | figma-clipboard / figma-api | 42 / 66 | import fixtures |
| Remove background, vectorize, AI image edit | ImageOps | 66 | fixtures |
| Slides, present, PDF | slides | 68 | slides tests |
| Share snapshots, update, revoke, ZIP | share | 62 | round-trip test |
| Dashboard, drafts, recovery | desktop | 28, 58 | Electron E2E |
| Settings (grid, snapping, nudge, zoom, animations) | local prefs | 45, 54 | E2E |
| Full keyboard set | keybinding table | 44, 45 | every-shortcut test |
| One-click MCP setup for many clients | Connect agents | 56 | config-writer fixtures |
| Many providers + custom endpoints, status labels | ModelProviders | 32 | provider tests |
| CLI (agent, shell, app/headless, export, models, verbose) | `vellum` CLI | 59 | CLI goldens |

# Appendix B — Exact-Copy Assertions (CI-enforced)

From `DESIGN.md` (not from the design file; mismatches in `vellum.pen` go to DESIGN-GAPS.md).
Copy-lint fails if any string drifts. `{…}` are placeholders.

```text
Tagline:                 Design where your code lives.
First open heading:      What are we designing?
Home composer:           What do you want to design?
Draw by hand hint:       Or draw by hand: press F for a frame
Composer placeholder:    Describe what to design or change…
Selection placeholder:   Change the selected layers…
External session line:   Chat with {client} in its own window · Stop
Key storage:             Stored in your system keychain. Never written to your files or sent to us.
Notes for agents line:   Agents read these before changing this frame
Editing inside lease:    You're editing here too
Paused banner:           Agents paused · their changes are being rejected
History footer:          History since this file was opened · Full history is in Git.
History empty:           Changes from you and your agents appear here
Layers empty:            Ask the agent or press F to draw a frame
Home empty:              Open a folder to see its design files.
Daemon offline:          Daemon offline
Composer when offline:   Agents need the Vellum daemon · Restart
Basic renderer:          Basic renderer
Newer format:            Made with a newer version of Vellum (format {n}) · Update to edit
Write rejected:          Frame leased by {agent}
Key failed:              {provider} rejected the key · Update key
Engine not signed in:    {engine} isn't signed in · Open terminal to sign in
Usage limit:             {engine} reached its usage limit · resets at {time} · Switch engine
Agent stuck:             Still working on this step
Custom fonts:            Stored with this file, so it travels with it
Scale note:              Bound sizes become fixed values; instances get overrides.
Undo-turn confirm:       Removes {n} changes from {agent}'s turn. Your edits since then are kept.
Cost label:              Estimated cost at API rates
Sync scope (A05):        Copies of files you choose to share, never your whole repo
Script node empty:       Choose a script file or ask the agent to write one
Review reminder:         Review agent changes in History.
```

Banned in agent and system copy: emoji, exclamation marks, personas, claims of taste ("stunning",
"beautiful"), "magic", apologies for normal behaviour, fake typing indicators, "cloud" for the
person's own computer.

# Appendix C — Testing Strategy Summary

```text
Unit          types · codec · ops · guard · resolver · layout mapping · scene · display list ·
              script header parser · tools · providers (stub) · permissions · state machines
Property      one write path · attribution · per-actor undo · deterministic bytes · diff/merge ·
              quantization · resolver cache · convergence (v1.1)
Contract      every port × every adapter (Rust and TS)
Oracle        layout vs headless Chrome (Taffy-style gentest) · HTML export vs Chrome
Design        design-check (inventory, naming, checksum) · tokens generated from vellum.pen ·
              UI goldens vs docs/design/snapshots/ · DESIGN-GAPS review
Golden        renderers × fixtures (editor path vs headless path) · components × themes × states ·
              screens at 1280×800 (+1512×982, 960×640, 760×640 where listed)
Fuzz          codecs · merge driver · op application · script header · daemon protocol frames
Corpus        script escapes · prompt-injection in documents, skills and context nodes
Transcript    recorded installed-engine sessions replayed in unit tests (pinned engine versions)
Integration   daemon IPC (Unix + Windows) · MCP stdio bridge · VS Code (@vscode/test-electron) ·
              Electron (Playwright) · engines lane (real Claude Code / Codex, nightly) ·
              design-mcp lane (local only: Pencil MCP smoke)
E2E           spec → plan → parallel build → undo turn · design → code → tokens back ·
              share round-trip · recovery after crash
Manual-doc    notarization · Windows signing · ARM64 Linux · installed-engine sign-in flows
```

# Appendix D — Release Gate Metrics (public beta)

```text
Activation      % installs that create a document and get an agent to edit it within 24 h
Retention       weekly active documents; documents with commits in 2+ different weeks
Agent quality   eval suite (ARCHITECTURE §11): layout problems per frame trending down; token use per
                screen trending down
Reliability     Stop p95 < 3 s · pan/zoom 60 fps on 5,000 nodes · open 2 MB < 500 ms · zero data loss
                in recovery soak
Fidelity        layout oracle 100 % within 0.5 px · editor vs headless goldens 100 % within tolerance ·
                UI vs design snapshots 100 % within tolerance
Security        zero release blockers; canary clean; clean-room clean
```

# Appendix E — ADR List Expected By The End

```text
ADR-000  Deltas D1–D6 from ARCHITECTURE.md (P2; closed by P4, P6, P25–27, P34, P36–37)
ADR-001  Renderer: skia-wasm everywhere vs CanvasKit in TS (P4, text results appended in P5)
ADR-002  Layout oracle and sizing semantics (P6)
ADR-003  Daemon lifecycle and the stdio bridge (P27)
ADR-004  Loro behind DocStore; directory-codec threshold (P5)
ADR-005  execute: QuickJS code mode and the doc API (P25)
ADR-006  Session storage (P34)
ADR-007  Installed engines: Goose-derived adapters, Codex app-server (P36, P37)
ADR-008  Shader dialect and GLSL import (P67)
ADR-009  Stock image provider and licence (P42)
```

# Appendix F — Final Engineering Principle

Vellum contains two products:

```text
1. The visible one: a canvas in your repo where you describe screens, watch agents build them in
   parallel, steer them, refine by hand, and ship the code.
2. The invisible one: the guarantees that make that safe to rely on — one write path, every change
   attributed, undo that belongs to you, a file format that is deterministic and yours, renders
   that match what agents saw, layout that matches the browser, scripts that can't escape, keys
   that never leave the keychain, and nothing on the network you didn't ask for.
```

The second is more important. A beautiful agent panel over a file format that drifts, an undo that
eats your work, or a daemon anyone on the machine can talk to is a failed product with a nice UI.

**Describe → plan → watch → steer → refine → ship. Build exactly that, borrow what's already
solved, and refuse anything that makes the file, the history or the machine less yours.**

# Appendix G — DESIGN-DECISIONS.md Template

```markdown
# Design decisions (edited by a human only)

Choices the brief leaves open. If vellum.pen already shows one option, record it here so code
and design agree; if it shows both, pick one.

| Decision | Options | Choice | Date | By |
|---|---|---|---|---|
| Agent turn layout | A Prose and steps interleaved in order · B Prose first, steps collapsed below | UNDECIDED | | |
| Plan card placement | A Inline in the turn · B Sheet over the panel | UNDECIDED | | |
| Default plan threshold | 2 frames · 3 frames · 20 changes | UNDECIDED | | |
| Lease tab position | A Above the frame's top-left · B Inside the frame's top edge | UNDECIDED | | |
| Placeholder style | A Dashed outline + shimmer · B Tinted fill + label | UNDECIDED | | |
| Alternatives layout on canvas | A Row with A/B/C labels · B Grid for 4+ | UNDECIDED | | |
| Inline prompt anchor | A Beside the selection · B Below the selection toolbar | UNDECIDED | | |
| Slot marker | A Dotted inner outline · B Hatched fill | UNDECIDED | | |
| Selection toolbar contents | A Ask only · B Ask + quick actions | UNDECIDED | | |
| UI golden tolerance vs design snapshots | 0.5 % · 1 % · per-screen | UNDECIDED | | |
```
