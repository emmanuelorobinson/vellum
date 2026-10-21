# Version baseline

- **Resolved:** 2026-10-11, against the crates.io sparse index, the npm registry, the Rust channel manifest
  (`static.rust-lang.org`, channel 2026-10-01), the Node.js release index, and the GitHub releases and tags of each
  reference repository.
- **Covers:** every line of guide §5, the pins the manifests carry (`package.json`, `pnpm-workspace.yaml`,
  `Cargo.toml`, `rust-toolchain.toml`, `.nvmrc`), and the reference repositories of guide §3.
- **Checked by:** `scripts/__tests__/version-baseline.test.mjs` (every §5 line has an exact version, the manifests agree
  with this file, the reference SHAs are recorded, no lockfile holds a prerelease) and `scripts/__tests__/architecture-records.test.mjs`.

## Rules

1. The latest stable patch of each line in guide §5. Prereleases (canary, beta, rc, alpha) are rejected: for example
   `axum 0.8.0-rc.1` and `serde 1.0.172-alpha.0` exist and are not used.
2. Exact versions are recorded here. Manifests pin exact versions for npm and the catalog. Cargo requirements are
   caret-style (`"1.0.229"`) and `Cargo.lock` holds the exact resolved version.
3. Lockfiles hold no prerelease. The test above fails if one appears.
4. A line not yet installed is still recorded, with the prompt that installs it. Nothing is installed ahead of the
   prompt that needs it.

## Rust toolchain and crates

| Component | Resolved | Guide line | Pinned in | Decision |
|---|---|---|---|---|
| Rust toolchain | 1.99.0 | latest stable toolchain | `rust-toolchain.toml` channel; `Cargo.toml` rust-version 1.99 | C1 |
| tokio | 1.53.2 | 1.53.x | ipc-local (Prompt 27), model-http (32), engine-claude-code (36), engine-codex (37) | latest stable patch of the line |
| axum | 0.8.9 | 0.8.x | not yet used | daemon `--listen` and sync server (later prompts) |
| interprocess | 2.4.5 | 2.4.x | not yet used | Unix sockets and named pipes (Prompt 27) |
| loro | 1.16.2 | 1.16.x | workspace `Cargo.toml`, used by `crates/adapters/doc-loro` only (Prompt 14); also `spikes/engine` (Prompt 5) | behind DocStore (Prompt 14, ADR-004) |
| taffy | 0.14.0 | 0.14.x | spike only: `spikes/layout` (Prompt 6, default features) | layout adapter (Prompt 18); ADR-002, C17 |
| parley | 0.12.0 | 0.12.x | workspace `vellum-text-parley` (Prompt 19; no default features, `std`, `complex-scripts`); renderer spike (`std`) | ADR-001 and ADR-013 |
| fontique | 0.12.0 | 0.12.x | text adapter through Parley's re-export (Prompt 19), no system feature | explicit font bytes/tier matching; ADR-013 |
| skia-safe | 0.153.3 | 0.153.x | spike only: `spikes/renderer` (features gl; textlayout for a size build and the native SkParagraph comparison) | the renderer everywhere, native and emscripten (ADR-001, C15, C16) |
| vello | 0.11.0 | 0.11.x | not yet used | later, behind a flag |
| kurbo | 0.13.1 | 0.13.x | not yet used | path geometry adapter |
| i_overlay | 9.0.1 | 9.0.x | not yet used | boolean operations (Prompt 12) |
| rstar | 0.13.0 | 0.13.x | =0.13.0 | spatial index (Prompt 20) |
| libm | 0.2.16 | portable math (not in §5) | `crates/adapters/script-quickjs`; no defaults, `force-soft-floats` | identical Math in native/WASM (Prompt 25, ADR-005) |
| rquickjs | 0.14.0 | 0.14.x | `crates/adapters/script-quickjs` only; `std`, emscripten-only `bindgen`; also the Prompt 5 spikes | ScriptRuntime (Prompt 25, ADR-005); explicit bounded allocator, no loader/futures |
| rmcp | 3.5.1 | 3.5.x | not used: Prompt 29 wrote the MCP front end itself (ADR-021, C22) | read for the wire shapes only |
| schemars | 1.2.2 | 1.2.x | `Cargo.toml` workspace.dependencies | JSON Schema from vellum-types |
| ts-rs | 12.0.1 | 12.x | `Cargo.toml` workspace.dependencies | TypeScript bindings from vellum-types |
| wasm-bindgen | 0.2.129 | 0.2.129+ | not yet used | ADR-001 uses emscripten C exports for the renderer; not needed there |
| notify | 8.2.0 | 8.2.x | `crates/adapters/watch-notify` (pinned `=8.2.0`, default features off, `macos_fsevent`) | FileWatcher (Prompt 28, ADR-020) |
| rusqlite | 0.40.2 | 0.40.x | `crates/adapters/sessions-sqlite` (pinned `=0.40.2`, `bundled`) | SessionStore (Prompt 34, ADR-006) |
| keyring | 4.2.0 | 4.2.x | `crates/adapters/secrets-keyring` (`v1` native stores) | SecretStore (Prompt 32, ADR-023) |
| clap | 4.6.7 | 4.6.x | `Cargo.toml` workspace.dependencies (`derive`, `string`) | CLI; `vellum tool` is generated from the registry (Prompt 29, C22) |
| chromiumoxide | 0.9.1 | 0.9.x | not yet used | BrowserPort, v1.1 (Prompt 65) |
| naga | 30.0.1 | 30.x | not yet used | shader translation, v1.1, only if ADR-008 uses it |
| cargo-deny | 0.20.2 | 0.20.x | `scripts/check-deps.sh` and `pnpm cargo:deny` (installed, not a dependency) | C5 |
| emsdk (emscripten) | 4.0.6 | wasm32 + emscripten targets per ADR-001 | `spikes/renderer/scripts/emscripten-env.sh` (installed, not a dependency) | C15 |
| png | 0.18.1 | dev tooling (not in §5) | `crates/vellum-testkit` | pixel-diff PNG read and write (Prompt 4) |
| serde | 1.0.229 | workspace dependency (not in §5) | `Cargo.toml` workspace.dependencies | Prompt 1 pin, still latest |
| serde_json | 1.0.154 | workspace dependency (not in §5) | `Cargo.toml` workspace.dependencies | Prompt 1 pin, still latest; `float_roundtrip` on since Prompt 7 (C18) |
| proptest | 1.12.0 | dev dependency (not in §5) | `Cargo.toml` workspace.dependencies; `vellum-types` and `vellum-core` dev-dependencies | property tests (guide §11 and Prompts 7, 9, 10, 14); `scripts/rust-deps-allowlist.txt` (C18, C19) |
| trybuild | 1.0.122 | dev dependency (not in §5) | `Cargo.toml` workspace.dependencies; `vellum-core` dev-dependencies | compile-fail test for Invariant 1 (Prompt 11); C19 |
| sha2 | 0.11.1 | 0.11.x | `crates/adapters/assets-fs` | SHA-256 content hash of an asset, its file name in `<name>.assets/` (Prompt 24, ADR-018, C20) |
| wuff | 0.2.9 | 0.2.x | `crates/adapters/text-parley` | WOFF and WOFF2 to sfnt, pure Rust, MIT; byte-identical to Google's woff2 on google/fonts per its README (Prompt 24, ADR-018, C20) |

## Node and JavaScript tooling

| Component | Resolved | Guide line | Pinned in | Decision |
|---|---|---|---|---|
| Node | 24.21.0 | Node 24 LTS line | `.nvmrc` (24); `package.json` engines `>=24 <25` | C4 |
| pnpm | 12.11.2 | 12.11.x | `package.json` packageManager | latest stable patch |
| Turborepo | 2.11.7 | 2.11.x | root devDependencies (`turbo`) | latest stable patch |
| TypeScript | 5.9.3 | 7.0.x native line, newest 5.x if a tool conflicts | `pnpm-workspace.yaml` catalog | C3 |
| @types/node | 24.19.2 | Node runtime line (not in §5) | `pnpm-workspace.yaml` catalog | C2 |
| Vite | 8.3.4 | 8.3.x | `pnpm-workspace.yaml` catalog | Storybook builder in `@vellum/ui-tokens` (Prompt 43, C24) |
| React | 19.3.0 | 19.3.x | `pnpm-workspace.yaml` catalog | latest stable patch |
| @types/react | 19.3.0 | 19.3.x (not in §5) | `pnpm-workspace.yaml` catalog | matches React |
| @types/react-dom | 19.3.0 | 19.3.x (not in §5) | `pnpm-workspace.yaml` catalog | matches React |
| @base-ui/react | 1.9.0 | 1.9.x | `pnpm-workspace.yaml` catalog; `@vellum/editor-react` | menus and tooltips of the editor chrome (Prompt 46, C26) |
| shadcn | 4.21.4 | CLI current at scaffold (generated source, not a dependency) | not installed | C11 |
| Tailwind CSS | 4.3.3 | 4.3.x | not yet used | not adopted in Prompt 46: the chrome is plain CSS over the token variables (C26) |
| Zustand | 5.0.15 | 5.0.x | not yet used | not adopted in Prompt 46: the workspace and session are small subscribable objects in editor-core (C26) |
| Zod | 4.6.6 | 4.6.x | not yet used | schemas at the edges (Prompt 10 onward) |
| i18next | 26.4.3 | 26.x | not yet used | copy and locale work (Prompt 70) |
| Biome | 2.5.15 | 2.5.x | root devDependencies (`@biomejs/biome`) | latest stable patch |
| dependency-cruiser | 18.5.0 | 18.5.x | root devDependencies | C10 and C3 |
| Vitest | 5.0.3 | 5.0.x | `pnpm-workspace.yaml` catalog | latest stable patch |
| Playwright | 1.64.1 | 1.64.x | `pnpm-workspace.yaml` catalog (`playwright`) | Storybook story tests (Prompt 43, C24); `@playwright/test` in `@vellum/editor-react` for the browser tests (Prompt 46, C26) |
| Storybook | 10.6.2 | 10.6.x | `pnpm-workspace.yaml` catalog | `@vellum/ui-tokens` stories (Prompt 43, C24) |

## Frontend apps, desktop, extension and cloud

| Component | Resolved | Guide line | Pinned in | Decision |
|---|---|---|---|---|
| Next.js | 16.4.0 | 16.4.x | `pnpm-workspace.yaml` catalog | account app and web viewer |
| Electron | 44.7.0 | 44.x | `pnpm-workspace.yaml` catalog | desktop host (Prompt 57) |
| electron-builder | 26.15.7 | 26.15.x | not yet used | packaging (Prompt 58) |
| electron-updater | 6.8.10 | 6.8.x | not yet used | updater (Prompt 58) |
| @vscode/vsce | 4.0.0 | 4.0.x | not yet used | VS Code packaging (Prompt 56) |
| ovsx | 1.2.0 | 1.2.x | not yet used | Open VSX publishing (Prompt 56) |
| @types/vscode | 1.140.0 | VS Code API types (not in §5) | `pnpm-workspace.yaml` catalog | C8 |
| canvaskit-wasm | 0.42.0 | 0.42.x, only if ADR-001 picks CanvasKit | spike only: `spikes/renderer/web/package.json` | C12 |
| @modelcontextprotocol/sdk | 1.32.1 | 1.32.x | not yet used | TypeScript side of the extension's MCP registration (Prompt 56) |
| better-auth | 1.7.7 | 1.7.x | not yet used | wrapped by `@vellum/auth` (Prompt 61) |
| drizzle-orm | 0.45.4 | 0.45.x | not yet used | account app database (Prompt 61) |
| Hono | 4.13.13 | 4.13.x | not yet used | public API (Prompt 61) |

## Installed engines and the design tool

These are not dependencies. They are the versions to test against, and each release pins its tested versions in ADR-007.

| Component | Resolved | Guide line | Pinned in | Decision |
|---|---|---|---|---|
| Claude Code | 2.1.296 | 2.1.x | not a dependency | C7 |
| Codex CLI | 0.162.1 | 0.162.x | not a dependency | C7 |
| @pen.dev/cli | 0.3.11 | 0.3.x (Node 22.19 or later) | not installed | C9 |

## Reference repositories

Each clone under `refs/` is blob-less and shallow, checked out at the commit below. Pins come from guide §3 and are
recorded in `scripts/refs.tsv`. `docs/reference/REFERENCE-INDEX.md` lists the paths the guide names.

| Repository | Pinned commit | Licence | Use | Newest release check (2026-10-11) |
|---|---|---|---|---|
| penpot | 10955f17e27943429a10e90360d0404e3f56e27c | MPL-2.0 | READ ONLY, never port | 2.18.3 (commit 2026-10-06) is older than the pin (2026-10-09) |
| graphite | 7477cd2fab315494e4c2613bc446c707733e6e7b | Apache-2.0/MIT | read and port with attribution | no GitHub release; tags are not versions |
| loro | 4f60d921d820de3522624a436de77f994a6622b8 | MIT | read and port with attribution | loro-crdt@1.16.4 (commit 2026-09-30) is older than the pin (2026-10-09) |
| taffy | 7d33901cc383489d9945ae796a1ceed0a935ef66 | MIT | read and port with attribution | v0.14.0 (commit 2026-08-24) is older than the pin (2026-10-09) |
| parley | 46fcad316d4ff05dc0d5647fda2d15def7be896e | Apache-2.0/MIT | read and port with attribution | v0.12.0 (commit 2026-10-09 15:37Z) is older than the pin (19:50Z) |
| vello | 235739c4f08eb48d900e1512459d453a9ab77f96 | Apache-2.0/MIT | read and port with attribution | v0.11.0 (commit 2026-10-02) is older than the pin (2026-10-09) |
| kurbo | 9366cda80ebb56f41e5d09509a89dbfe81bc4f5b | Apache-2.0/MIT | read and port with attribution | v0.13.1 (commit 2026-05-13) is older than the pin (2026-10-08) |
| codex | 8c0f7011674145da7e10627769fb221c74fbf7a0 | Apache-2.0 | read and port with attribution | rust-v0.162.1 (commit 2026-10-09) is older than the pin (2026-10-11) |
| goose | 3bd852002903e016ff30947e973f76e2fcfcf90f | Apache-2.0 | read and port with attribution | v1.54.0 (commit 2026-10-08) is older than the pin (2026-10-09) |
| opencode | d21b0790a6c146815f28fea604e41e922f009590 | MIT | read and port with attribution | v1.19.0 (commit 2026-10-11 01:39Z) is older than the pin (09:27Z) |
| rquickjs | a4e45effc347d63c58c2efc3020566e6095cc94f | MIT | read and port with attribution | no GitHub release; tag v0.14.0 (commit 2026-09-18) is older than the pin (2026-10-05) |
| rmcp | c1a8eea312613519b93f53b89a1dc74dc993f5b0 | Apache-2.0 (MIT legacy parts) | read and port with attribution | rmcp-v3.5.1 (commit 2026-10-05) is older than the pin (2026-10-10) |
| vscode-samples | 65c1c44800eeac6fb065a54823ee5ae62c79540a | MIT | read and port with attribution | no release and no tags |
| excalidraw | e65eca999e071503efb893d3379a29fb96ff37c4 | MIT | read and port with attribution | v0.18.1 (commit 2026-04-20) is older than the pin (2026-10-10) |
| tldraw | 7937be360bcb8239d89e420850843f81313fc07b | tldraw licence | DOCS ONLY, never port | v5.5.2 (commit 2026-10-02) is older than the pin (2026-10-09) |

No repository has a release at or after its pin, so every pin stays at the guide's SHA (C6).

## Compatibility decisions

- **C1: Rust 1.98.1 becomes 1.99.0.** The stable channel of 2026-10-01 ships Rust 1.99.0 (`b940084d7`, 2026-09-28), and
  Prompt 1 had pinned the release before it. The MSRV (`rust-version`) moves to 1.99 with it, because the repository
  only tests the pinned toolchain. Clippy with `-D warnings` and the workspace tests pass on 1.99.0.
- **C2: @types/node moves from 26.6.5 to 24.19.2.** The catalog had pinned the Node 26 line, but the runtime is Node 24
  (`.nvmrc`). Types for a newer runtime admit APIs that do not exist where the code runs.
- **C3: TypeScript 7.0.2 becomes 5.9.3.** TypeScript 7 is the guide's first choice. dependency-cruiser 18.5.0 refuses it
  (`typescript: >=2.0.0 <7.0.0`) and then cruises zero modules, so the dependency rules would pass without checking
  anything. The guide's fallback applies: the newest 5.x. `scripts/check-deps.sh` also fails when dependency-cruiser
  reads no TypeScript. Revisit when dependency-cruiser supports TypeScript 7.
- **C4: Node stays on the 24 line.** `.nvmrc` pins `24`, which is the LTS line. The newest LTS patch on 2026-10-11 is
  24.21.0, the version installed here. Node is not in a lockfile, so the line is the pin.
- **C5: cargo-deny 0.20.2.** `cargo deny check` runs all four checks, advisories included, and needs the network.
  `pnpm deps:check` runs bans, licences and sources only, which need no network.
- **C6: Reference pins stay.** The rule was: re-pin to the newest release at or after the pin, and keep the pin where
  there is none. Every release is older than its pin (table above), so all 15 pins are unchanged. `refs.tsv` is the
  single record of them.
- **C7: Installed engines are not dependencies.** Claude Code 2.1.296 and Codex CLI 0.162.1 are the newest in their lines
  at scaffold. The versions Vellum tests against are pinned per release in ADR-007 (Prompts 36 and 37).
- **C8: @types/vscode 1.140.0.** The VS Code API types current at scaffold. The extension's minimum `engines.vscode` is
  set in Prompt 55.
- **C9: @pen.dev/cli 0.3.11 is a design-tool dependency only** (Prompt 3, `scripts/design-snapshot`). It is never shipped
  or imported by product code, and this file records only its published version.
- **C10: dependency-cruiser 18.5.0** is a root devDependency with an exact pin, as guide §5 asks. Its rules live in
  `.dependency-cruiser.cjs`.
- **C11: shadcn 4.21.4** is recorded for the CLI. It generates source into the repository and is not a dependency.
  Nothing is generated yet.
- **C12: canvaskit-wasm 0.42.0** is installed only in the renderer spike (`spikes/renderer/web`), as Renderer A. ADR-001
  (Prompt 4) chose skia-safe in WASM, so no product package depends on it.
- **C13: Lines not yet installed** (Tailwind, Zustand, Zod, i18next, Electron builder, vsce,
  better-auth, drizzle-orm, Hono, the MCP SDK and the unused Rust crates) are recorded at their resolved versions. The
  prompt that adds one installs the recorded version and updates this file in the same change.
- **C14: Unchanged from Prompt 1:** pnpm 12.11.2, Turborepo 2.11.7, Biome 2.5.15, React 19.3.0, Vitest 5.0.3, Next.js
  16.4.0, Electron 44.7.0, serde 1.0.229, serde_json 1.0.154, schemars 1.2.2, ts-rs 12.0.1 and clap 4.6.7. Each is still the
  newest stable patch of its line.
- **C15: emsdk 4.0.6 and Skia from source for emscripten** (Prompt 4, ADR-001). 4.0.6 is the version Penpot pairs with
  skia-safe 0.153.x. rust-skia's prebuilt emscripten binaries do not link with Rust 1.99 (JS longjmp vs wasm exceptions),
  so the emscripten build compiles Skia from source with ninja; the native build keeps the prebuilt binaries.
- **C16: The native Skia build uses FreeType** (Prompt 5, ADR-001 "Text"). Text must rasterize the same natively as in
  the emscripten build, and rust-skia's prebuilt macOS binaries have only CoreText, so the spike's native build also
  compiles Skia from source (`SKIA_GN_ARGS="skia_use_freetype=true skia_use_system_freetype2=false
  skia_enable_fontmgr_custom_empty=true"`) and patches one bindgen alias for the macOS 27 SDK
  (`spikes/renderer/scripts/build-native.sh`). Fonts for the text fixtures are Noto files pinned by SHA-256 in
  `spikes/renderer/fonts.json`, downloaded and not committed.
- **C17: Chrome for Testing 155.0.8059.39 writes the layout oracle** (Prompt 6, ADR-002). It is the version recorded in
  `fixtures/layout-oracle/expected/manifest.json`; CI installs it with `browser-actions/setup-chrome@v2` (v2.2.0 when written; Chrome pinned
  by version in `.github/workflows/ci.yml`). The committed boxes, not the version, are the contract: `--check` passes
  under another Chrome as long as no box moves more than 0.5 px. taffy 0.14.0 on crates.io predates the `refs/taffy`
  pin and spells alignment constants `AlignItems::FLEX_START`, not the pin's `AlignItems::FlexStart`.

- **C18: `proptest` joins the pure-crate allowlist as a dev-dependency; `serde_json` and `ts-rs` gain features** (Prompt 7).
  The guide asks for property tests from Prompt 7 on, and `vellum-types` and `vellum-core` are the crates they cover, so
  `proptest` (default features off, `std` only) is allowed on `vellum-types` in `scripts/rust-deps-allowlist.txt`; it
  never reaches a normal build. `serde_json`'s `float_roundtrip` is on because without it the parser can be one ULP off
  for some `f64`, and a model coordinate must read back as written. `ts-rs`'s `serde-json-impl` lets a node's free-form
  `metadata` (and an instance's placeholder props) export as `JsonValue`. `vendor` entries in the allowlist still bar
  `wasm-bindgen` and the rest from the closure; `pnpm deps:check` passes.
- **C19: `vellum-core` may take `serde_json`, `proptest` and `trybuild` as dev-dependencies** (Prompt 11). Invariant 1
  asks for a trybuild compile-fail test and a property test over op batches, and both live in `vellum-core`. The op
  tests build sample documents from JSON, so `serde_json` comes too. All three are dev-only; `vellum-core`'s normal
  dependencies are still `vellum-types` alone, and `pnpm deps:check` (which reads dev edges) passes. trybuild 1.0.122
  was the newest release on 2026-10-12; it brings `toml`, `glob`, `target-tuple` and their dependencies, which `cargo deny` accepts.

## What the reference clones are for

`pnpm refs` clones the 15 repositories (about 2 GB on disk: goose 655 MB, penpot 509 MB, opencode 240 MB, the rest
under 170 MB each) and checks that every path named in guide §3.2 and §3.3 exists. It needs the network, so it runs on a developer machine and not in CI. Nothing in `refs/` is committed or
edited.

## Prompt 22 renderer production pin

- `skia-safe = 0.153.3`, matching ADR-001's measured spike, now source-only with gl/pdf/jpeg/embed-freetype.
  Default features, binary-cache and textlayout are disabled. GN selects data-backed FreeType, disables
  platform font managers and the low-precision CPU raster path. emsdk remains 4.0.6; Rust remains 1.99.0.
- Noto Color Emoji is bundled at the spike's pinned revision/hash, with its original OFL alongside Inter.
- ADR-016 records the source-build choice, retired macOS 27 patch and local native/WASM pixel evidence.
  The six-target native CI matrix and opt-in hardware matrix are configured, not yet verified remotely.

- **C20: `sha2` and `wuff` join the adapters; both are banned everywhere else** (Prompt 24). `sha2` 0.11.1 hashes assets
  in `vellum-assets-fs` (SHA-256 is the file name and the `AssetId`). `wuff` 0.2.9 unpacks WOFF/WOFF2 in
  `vellum-text-parley`; no other pure-Rust decoder was checked, and the encoders (`woofwoof`, `ttf2woff2`) are not needed
  because Vellum stores the imported file as it came. `deny.toml` bans both with a wrapper, and
  `scripts/rust-deps-allowlist.txt` keeps them out of `vellum-core` and `vellum-types`. WOFF fixtures in
  `crates/adapters/text-parley/tests/fonts/` come from `@fontsource/inter` 5.3.0 (OFL 1.1).

- **C21: native IPC** (Prompt 27, ADR-003). `vellum-ipc-local` pins Tokio 1.53.2 (the recorded baseline),
  `getrandom` 0.4.3, `windows-sys` 0.61.2 and Unix `libc` 0.2.190. Tokio supplies both Unix sockets and
  Windows named pipes; `interprocess` remains unused. The small Win32 boundary creates and checks explicit
  user-only DACLs. Tokio is wrapper-banned outside this adapter. No OS dependency enters the pure crates.

- **C22: the MCP front end is not `rmcp`; `vellum` takes more normal dependencies** (Prompt 29, ADR-021). The daemon already
  owns the transport (ADR-003), the documents are not `Send`, and the HTTP listener is one request per connection, so
  `crates/vellum/src/mcp.rs` answers `initialize`, `ping`, `tools/list`, `tools/call` and `server/discover` itself. `rmcp`
  3.5.1 stays recorded, unused. `crates/vellum` now depends on `getrandom` 0.4.3 (the same pin as the IPC adapter, for node
  ids) and, behind its default `scene` feature, on the four scene adapters (Taffy, Parley, rstar, Skia), so the shipped binary
  draws; the IPC lane builds it with `--no-default-features`. clap gains its `string` feature, which lets the CLI name
  subcommands and flags after the registry's tools at run time. `serde_path_to_error` 0.1.20 (already a workspace
  dependency) names the field at fault in a tool's input.

- **C23: model boundaries** (Prompt 32, ADR-023). `reqwest` =0.13.5 (`rustls`, `json`, no defaults),
  Tokio =1.53.2, async-channel =2.5.0 and url =2.5.8 live in `vellum-model-http`. TLS resolves to patched
  rustls 0.23.46. The four protocol adapters use only core ports and serde_json. `keyring` =4.2.0
  uses macOS Keychain, Windows Credential Manager and Linux Secret Service through its `v1` interface.
- **C24: Storybook in `@vellum/ui-tokens`** (Prompt 43, owner's request). Set up with `npm create storybook@latest
  -- --type html --builder vite --disable-telemetry`, then pinned exactly through the catalog: storybook,
  `@storybook/html-vite`, `@storybook/addon-vitest`, `@storybook/addon-a11y`, `@storybook/addon-docs` 10.6.2 (newest
  patch of the 10.6 line; the baseline had resolved 10.6.1), `@chromatic-com/storybook` 5.5.1, playwright 1.64.1
  (newest 1.64 patch), Vite 8.3.4, `@vitest/browser-playwright` and `@vitest/coverage-v8` 5.0.3 (match Vitest).
  Telemetry is off in `.storybook/main.ts`. One transitive prerelease is allowed by name:
  `@polka/url@1.0.0-next.29` (via sirv ← `@vitest/browser` ← `@storybook/addon-vitest`), because that package has
  never published a stable version; the lockfile test fails on any other. pnpm skips esbuild's install script
  (not approved); Storybook builds and runs without it.
- **C25: React component harness** (Prompt 45). `@storybook/react-vite` 10.6.2 matches C24's existing
  Storybook release; React/React DOM stay 19.3.0, Vite 8.3.4 and Playwright 1.64.1. No existing version
  changed. The HTML token Storybook remains in ui-tokens; editor-react owns the React harness. Both
  disable telemetry. The new CI chrome lane exercises the scripted engine/host boundaries only.
  The owner approved one exact transitive prerelease exception on 2026-10-18:
  `gensync@1.0.0-beta.2`, through Babel ← react-docgen ← React Storybook. The React preset imports
  docgen even when automatic prop documentation is disabled, so removing the dependency broke the
  browser story tests. The standard dependencies are retained; the version test allows only that
  exact name/version in addition to C24's prior exception. No direct prerelease dependency is added.


## Installed engines (Prompt 36)

Claude Code 2.1.301 was detected locally on 2026-10-16. Its `--help`, `auth status --help` and official
headless/MCP documentation were checked. Required flags are probed before accepting a run; no installation
or version pin is imposed on the person's tool. Native process support uses existing tokio 1.53.2 (`process`
feature), libc 0.2.190 and windows-sys 0.61.2 (`JobObjects`) behind the installed-engine adapter. ADR-007.


## Prompt 37 installed Codex verification

Codex CLI **0.158.0** was already installed locally on macOS arm64 and reports ChatGPT sign-in. Both
app-server stdio/dynamic tools and exec JSONL/MCP were exercised with `gpt-6-luna` from its local model
catalog. This is older than C7's resolved 0.162.1; no install/update was performed. ADR-007 and the adapter's
`SUPPORTED_VERSIONS` pin the tested binary to 0.158.0 until another version passes replay and native lanes.
The shared tokio/libc/windows-sys versions already used by Prompt 36 are reused; no new vendor version.

## Prompt 41 export dependencies

- `zip` =4.6.1 (already cached locally; MIT) is confined to `vellum-export-raster`, plus composition-root
  dev tests that read its output. Defaults are disabled: stored archives, no clock or compression dependency.
  Every entry has an explicit 1980-01-01 timestamp. `deny.toml` wrapper-bans it and the pure-crate allowlist
  excludes it. No dependency version already in use was changed.
- Skia remains 0.153.3, source-only. `webp` (encode and decode) joins gl/pdf/jpeg/embed-freetype so WEBP uses the same full
  drawing path and packaged source build; `scripts/skia-config.json` changes the native/WASM cache key.
- **C26: editor chrome stack** (Prompt 46). `@base-ui/react` 1.9.0 (Menu, Tooltip; MIT) and `@playwright/test` 1.64.1 are pinned in
  the catalog and used only by `@vellum/editor-react`. Tailwind and Zustand stay uninstalled: the chrome is plain CSS over the token
  variables (the token lint forbids literals either way), and `editor-core`'s workspace and session are small subscribable objects,
  so a store library had nothing to do. Icons are not a dependency: `scripts/ui-icons-gen.mjs` writes the 49 Lucide paths the chrome
  uses from the catalog already in `crates/adapters/icons/data/lucide.json` (lucide-static 1.59.0, ISC). That version has no
  `history` (the design's name for what it now calls `rotate-ccw-clock`; same drawing) and no `message-square-play` (the design's
  prompt-node icon; `message-square-more` stands in until the catalog has one). Its `git-branch` is drawn differently from the
  design's. The lockfile gained `@base-ui/react`, `@base-ui/utils`, `@floating-ui/*` and `@playwright/test`; none is a prerelease.

## Prompt 55 extension host tooling

- `@vscode/test-electron` 2.5.2 (MIT; Node >=16) is pinned for the tagged VS Code host lane.
  Its `runTests` API was checked against the official VS Code docs through Context7. The local smoke
  runs against the already-installed VS Code Insiders 1.143.0; no editor was installed or upgraded.
- `esbuild` 0.28.2 (MIT; Node >=18), already in the lockfile through Vite, is now also a direct build
  tool for the extension's CommonJS host and static webview bundles. React, VS Code types and the
  renderer retain their existing catalog versions. The workspace package remains scoped; the generated
  VS Code manifest has an unscoped extension name, as the extension host requires.

## Prompt 56 client setup and packaging tooling

- `jsonc-parser` 3.3.1 (MIT) and `smol-toml` 1.4.2 (BSD-3-Clause) are confined to the native
  `host-vscode` adapter. JSONC edits preserve other entries/comments; TOML is parsed for validation
  and only an exact managed block is removed. The extension bundle selects the parser's ESM entry,
  because bundling its UMD dynamic requires failed in the real extension host.
- `@vscode/vsce` 3.6.0 (MIT) and `ovsx` 0.10.5 (EPL-2.0) are pinned development tools. Their
  CLI help and VS Code's current publishing/MCP API docs were checked through Context7 and the
  installed packages. They are not bundled into the extension. Lifecycle scripts remained disabled
  during installation; ordinary packaging does not need their optional native credential helpers.
- Client paths/schemas are a documented baseline, not a claim that every installed client was tested.
  See `docs/runbooks/connect-agents.md`; no installed client was launched or reconfigured by this prompt.

## Prompt 57 desktop host

- Electron 44.7.0 remains the catalog pin; its macOS ARM64 runtime was downloaded and used with Playwright
  1.64.1, Node 24.21.0 and pnpm 12.11.2. No new third-party package or version was introduced.
- Electron security, contextBridge, native-process and utilityProcess documentation and Playwright's Electron
  APIs were read through Context7. The Rust sidecar uses a shell-free native spawn, not a JavaScript utility fork.
- `@vellum/host-api/native-relay` now contains the shared native stdio bridge; browser imports of the Host API
  do not import Node I/O. `@vellum/host-electron/bridge` is a separate browser bridge entrypoint so headless Host
  contract consumers do not acquire DOM dependencies. Electron 44's clipboard methods are asynchronous.
- Local Electron acceptance is macOS ARM64 only. The macOS CI lane is configured but not remotely verified;
  Windows/Linux native acceptance and signed packaging remain outstanding.

## Prompt 58 desktop packaging (2026-10-19)

Pinned `electron-builder` 26.15.3 and `electron-updater` 6.8.9 in apps/desktop (npm's stable versions on this
run). Context7 includes newer v27 signed-manifest docs; these versions use an explicit application-owned
Ed25519 release envelope and artifact-digest verification instead. Electron remains 44.7.0; no Rust dependency
changed. See docs/runbooks/desktop-home.md for the native matrix, trust inputs, preview and acceptance limits.

## Prompt 59 native CLI (2026-10-19)

No third-party version changed. The composition root now links the existing builtin engine, model HTTP,
OpenAI-compatible/Responses, Anthropic, Google and keychain adapters. Clap remains 4.6.7. Its derive/global
argument handling and Tokio's Ctrl-C subscription were checked through Context7. The IPC adapter reuses
its existing Tokio signal/sync features; remote CLI turns are described in ADR-025.

## Prompt 61 cloud ports (2026-10-19)

Bun 1.3.14 was used for the API and migration smoke tests and is pinned in the cloud CI lane.
Exact npm pins: Better Auth 1.7.11, Polar SDK 1.0.2 (2026-10 API entrypoint), Hono 4.13.13,
BullMQ 6.3.17, ioredis 6.0.0, AWS SESv2 SDK 3.1152.0, Drizzle 0.45.4, pg 8.23.1 and @types/pg
8.20.0. Vendor imports stay in cloud wrappers; Hono is the API composition framework. Context7 and
installed package types/docs were checked. Polar 1.x uses async webhooks and snake_case resources;
BullMQ 6 requires an explicit Redis client under native ESM. The pnpm installer generated nine exact
minimumReleaseAgeExclude entries for the selected newly released Better Auth/BullMQ versions.
Rust account HTTP reuses existing reqwest 0.13.5, Tokio 1.53.2 and url 2.5.8; no vendor version changed.

## Prompt 62 snapshot sharing (2026-10-19)

Added `@aws-sdk/client-s3` 3.1152.0, matching the existing SES SDK generation. R2's current official
S3 compatibility docs were checked through Context7; conditional object writes are exercised at the SDK
boundary. The existing reqwest/Tokio adapter also carries the bounded snapshot protocol. No Rust vendor
version changed. Viewer rendering uses the existing shared WASM/Skia ABI. Next route-handler and Turbo
2.11.7 task-order contracts were checked through Context7 and the installed Turbo docs; graph acceptance
verifies that both viewer hosts wait for the root WASM build. See ADR-026 and `docs/runbooks/sharing.md`.

## Prompt 63: sync backend (2026-10-20)

Registry versions verified with `cargo search`/`cargo info`; resolved in Cargo.lock. No pure-crate
allowlist changes. Adapter boundaries and canonical compaction epochs are in ADR-027.

| Dependency | Exact version | Wrapper | Purpose |
|---|---|---|---|
| axum | 0.8.9 | vellum-sync-ws | authenticated bounded WebSocket service |
| tokio | 1.53.2 (existing pin) | sync transport and S3 | I/O workers and isolated blocking work |
| postgres | 0.19.14 | vellum-sync-postgres | transactional update log and capability/entitlement reads |
| redis | 1.7.1, default features disabled | vellum-sync-redis | expiring connection membership |
| object_store | 0.14.2, aws only | vellum-assets-s3 | private immutable canonical snapshots over R2/S3 |
| rusqlite | 0.40.2 (existing pin), bundled | vellum-sync-sqlite | self-host update log and hashed grants |
| sha2 | 0.11.1 (existing pin) | sync disk/SQL and S3 adapters | snapshot integrity and credential digests |
| tungstenite | 0.29.0, test dependency | vellum-sync-ws test client feature | actual loopback WebSocket acceptance |

Self-host composition uses SQLite, private disk and memory presence without a billing/account service.
Cloud composition uses Postgres/Redis/private R2 and the existing entitlement vocabulary. No external
reference code ported, no cloud service deployed. Current advisory failures remain recorded in reports.md.

## Prompt 66 image work (2026-10-21, ADR-029)

| Adapter dependency | Exact pin | Scope |
|---|---|---|
| image | 0.25.10 | Bounded PNG/JPEG/WebP decode and PNG encoding inside image-ops |
| vtracer | 0.6.5 | Stable tracing API inside image-ops; deprecated transitive packages remain advisory failures |
| tract-onnx | 0.23.9 | Stable self-contained CPU inference inside image-ops, transformers disabled |

The experimental prerelease ONNX wrapper was removed. No prerelease or advisory exception was added.
