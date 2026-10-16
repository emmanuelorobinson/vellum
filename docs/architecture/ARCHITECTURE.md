# Vellum — System Architecture

> **Vellum** is a working codename. Swap it for the real name later.
> An open-source, local-first vector design canvas for the IDE, desktop, browser and terminal, with an MCP server so coding agents can design.
> Everything runs on **one shared Rust core**. The cloud is optional and only adds sync, sharing and multiplayer.

---

## 1. The big picture

```
 ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
 │ VS Code / Cursor │  │  Desktop app     │  │ Browser (later)  │
 │ extension host   │  │  (Electron)      │  │ (Next.js shell)  │
 └────────┬─────────┘  └────────┬─────────┘  └────────┬─────────┘
          │       implements the Host interface       │
          ▼                     ▼                     ▼
        ┌────────────────────────────────────────────────────────┐
        │  EDITOR (same bundle everywhere)                        │
        │  editor-react (UI) → editor-core (tools, commands)      │
        │  vellum-wasm (engine)  ·  Renderer (CanvasKit today)    │
        └───────────────┬──────────────────────────┬─────────────┘
                        │ Daemon protocol (v1)     │ Sync protocol (v1)
                        │ local socket / pipe      │
                        ▼                          ▼
 ┌──────────────────────────────────┐   ┌──────────────────────────────┐
 │ `vellum` daemon (native Rust)    │   │ OUR CLOUD (optional)         │
 │ Tool registry → MCP · CLI ·      │   │ Sync server · API · workers  │
 │ agent runner (engines, sessions) │   │ behind service ports         │
 │ headless render · script sandbox │   │                              │
 └───────────────┬──────────────────┘   └──────────────────────────────┘
                 │ MCP (stdio / streamable HTTP)
                 ▼
     External: Claude Code · Cursor · Codex · Gemini CLI · Copilot
     Built-in engines: our loop on any model · your installed Claude Code / Codex
```

**Core rules:**
1. There is one document engine, `vellum-core`. The editor runs it as WASM; the daemon, CLI, CI tools and cloud workers run it natively.
2. Every edit, by hand or by agent, is an **operation** applied by the core. Nothing else mutates a document.
3. **The core knows no vendors.** Every third-party engine (CRDT, layout, text, rendering, storage, models, billing) sits behind an interface we own. Choosing which implementation to use happens in one place per program: its *composition root*.

---

## 2. Engineering principles

This section is the contract for how the codebase is shaped. It exists so any part can be replaced, rewritten or handed to another engineer (or agent session) without the rest noticing.

### 2.1 Hexagonal layout: the dependency rule

```
           ┌─────────────────────────────────────────────┐
           │  COMPOSITION ROOTS                           │  pick adapters from config
           │  vellum-wasm · vellum (daemon) · sync-server │
           │  apps/* (account, api, extension, desktop)   │
           └──────────────┬──────────────────────────────┘
                          │ depends on
           ┌──────────────▼──────────────┐
           │  ADAPTERS                    │  wrap one vendor each
           │  doc-loro · layout-taffy ·   │
           │  text-parley · render-skia · │
           │  llm-openai-compat · …       │
           └──────────────┬──────────────┘
                          │ implements
           ┌──────────────▼──────────────┐
           │  CORE + PORTS                │  pure logic, no I/O,
           │  vellum-core (ports = traits)│  no vendor crates
           │  vellum-types (data, DTOs)   │
           └─────────────────────────────┘
```

- **Dependencies point inward only.** Core never imports an adapter. Adapters never import each other. Only composition roots know which adapters exist.
- **Core is pure.** No file system, network, clock or randomness in `vellum-core`; those come in through ports (`Clock`, `IdGen`, `AssetStore`). This makes it deterministic, testable, and runnable in WASM, native and workers alike.
- **Enforced, not hoped for.**
  - Rust: `vellum-core` and `vellum-types` have an allowlist of dependencies checked in CI (`cargo-deny` bans + a script over `cargo tree`). Adding `loro` or `tokio` to core fails the build.
  - TypeScript: `dependency-cruiser` rules: `editor-core` can't import React or CanvasKit; apps can't import `better-auth`, `@polar-sh/*` or `@aws-sdk/*` directly, only our wrapper packages.

### 2.2 What gets abstracted (and what doesn't)

Abstraction has a cost: indirection, lowest-common-denominator APIs, more code. So each dependency is put in a tier deliberately.

| Tier | Rule | Examples |
|---|---|---|
| **A: Swap-likely** | Full port (trait/interface), ≥2 implementations from day one (real + fake), contract test suite | CRDT, renderer, text, layout, host, model provider, agent engine, script runtime, browser, importers, billing, object storage, file codec |
| **B: Wrap thinly** | One internal module re-exports what we use; app code never imports the vendor | BetterAuth, BullMQ, PostHog, Sentry, SES, Meilisearch |
| **C: Use directly** | Foundational; swapping means a rewrite anyway, so abstraction would be waste | Rust, `serde`, `tokio`, `axum`, React, Next.js, tRPC, Drizzle, Postgres |

A dependency moves up a tier only with an ADR (§2.6).

### 2.3 Own the data, not the library's data

Vendor lock-in usually hides in persisted formats, not code. So:

- **No library's native format is ever the system of record.** On disk it's our `.vellum` JSON. In the cloud the sync server stores Loro updates for speed, but **also writes a `.vellum` snapshot** to object storage on every compaction. Replacing Loro means replaying from snapshots, not reverse-engineering binaries.
- **IDs are ours** (`NodeId`, `PageId`, `VariableId`), never a library's internal handles. Adapters map between them.
- **Display list, daemon protocol, sync envelope and file format are our types**, versioned, with JSON Schema generated from Rust.

### 2.4 Every port ships with a fake and a contract suite

- Each Tier A port has a **fake** (in-memory, deterministic) used in unit tests and for building other modules before the real adapter exists.
- Each port has a **contract test suite** in `crates/contracts` (Rust) or `packages/contracts` (TS): a set of behaviour tests any implementation must pass. A new adapter isn't mergeable until it passes the suite.
- **Golden tests** sit above the contracts: fixture documents rendered, laid out and round-tripped, compared byte-for-byte or pixel-for-pixel. They're what make a swap safe to ship.

### 2.5 Versioned boundaries

| Boundary | Versioning | Compatibility promise |
|---|---|---|
| `.vellum` file format | `meta.formatVersion`, migrations in core | Read every older version forever; write current only |
| MCP tools | Tool names stable; schema changes are additive; breaking change = new tool name (`insert_nodes_v2`) with the old one kept for 2 minor releases | Agents' prompts and skills keep working |
| `execute` script API | `doc.apiVersion` (major.minor) | Same rule as MCP tools: additive changes only within a major; old major kept for 2 minor releases |
| Skills (`read_skill` paths) | Paths stable; content versioned with the release | Agents' saved prompts that name a skill path keep working |
| Script node files | `@schema` header in each `.js` file | Scripts written for an older schema keep running |
| Daemon protocol (editor ↔ daemon) | `v` in handshake | Daemon supports current and previous version |
| Sync protocol | Envelope version + CRDT adapter ID | Server refuses mismatched clients with a clear upgrade error |
| Display list | `DISPLAY_LIST_VERSION` const | Internal; core and renderers ship together, checked at init |
| Public REST API | `/v1/` path prefix | Deprecation header, 6 months notice |

### 2.6 Decisions are written down

- `docs/adr/NNNN-title.md` for every Tier A choice and every tier change: context, decision, alternatives, **"revisit when"** (the condition that should trigger reconsidering it).
- Example: *ADR-0004 Renderer: CanvasKit.* Revisit when WebGPU ships in all three VS Code platforms and Vello text passes our golden suite.

### 2.7 How a swap is done (the playbook)

1. Write the new adapter in its own crate/package.
2. Pass the contract suite, then the golden suite.
3. Ship behind a config flag (`renderer = "vello"`), off by default.
4. **Dual-run where possible:** render with both and diff images in CI; apply ops to both CRDTs and compare exported JSON.
5. Flip the default in a release; keep the old adapter one more release; delete it.

### 2.8 Work packages

Because modules talk only through ports with fakes, they can be built in parallel and independently: by you, a contractor, or separate agent sessions. Each work package in §13 lists its port, its fake, and its contract suite, so "done" is objective.

---

## 3. Ports (the swap points)

### 3.1 Engine ports (`vellum-core::ports`)

Trait sketches; final signatures are settled in M0.

```rust
/// Document storage and history. Today: Loro. Alternatives: Automerge, Yrs, plain in-memory.
pub trait DocStore {
    fn snapshot(&self) -> DocumentData;                     // our types, not the CRDT's
    fn apply(&mut self, tx: Transaction) -> Result<ChangeSet, OpError>;
    fn undo(&mut self, actor: ActorId) -> Option<ChangeSet>;
    fn redo(&mut self, actor: ActorId) -> Option<ChangeSet>;
    fn export_updates(&self, since: &SyncCursor) -> SyncPayload; // opaque bytes + adapter id
    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError>;
}

/// Auto-layout. Today: Taffy. Alternatives: Yoga, our own.
pub trait LayoutEngine {
    fn layout(&mut self, tree: &LayoutTree, dirty: &DirtySet, text: &dyn TextEngine) -> LayoutResult;
}

/// Text shaping and paragraph layout. Today: Parley. Alternatives: cosmic-text, Skia Paragraph.
pub trait TextEngine {
    fn measure(&self, text: &RichText, constraints: Size) -> TextMetrics;
    fn layout(&self, text: &RichText, width: f32) -> TextLayout;  // glyph runs, our types
}

/// Fonts. Today: bundled + Google Fonts + project folder.
pub trait FontSource {
    fn resolve(&self, family: &str, style: FontStyle) -> Option<FontHandle>;
}

/// Hit-testing, culling, snapping. Today: rstar.
pub trait SpatialIndex {
    fn rebuild(&mut self, items: &[(NodeId, Rect)]);
    fn update(&mut self, changed: &[(NodeId, Rect)], removed: &[NodeId]);
    fn query_rect(&self, r: Rect) -> Vec<NodeId>;
    fn query_point(&self, p: Point) -> Vec<NodeId>;
}

/// Path booleans and tessellation. Today: kurbo + i_overlay.
pub trait PathOps {
    fn boolean(&self, a: &Path, b: &Path, op: BoolOp) -> Path;
    fn bounds(&self, p: &Path, stroke: Option<&Stroke>) -> Rect;
}

/// File format. Today: JSON single file, JSON directory. Later maybe binary.
pub trait Codec {
    fn id(&self) -> &'static str;
    fn decode(&self, bytes: &[u8]) -> Result<DocumentData, CodecError>; // runs migrations
    fn encode(&self, doc: &DocumentData) -> Vec<u8>;                    // deterministic
}

/// Assets (images, fonts) by content hash. Today: local folder, S3-compatible.
pub trait AssetStore {
    fn get(&self, hash: &AssetHash) -> Result<Bytes, AssetError>;
    fn put(&self, bytes: Bytes, mime: &str) -> Result<AssetHash, AssetError>;
}

pub trait Clock { fn now(&self) -> Timestamp; }
pub trait IdGen { fn node_id(&mut self) -> NodeId; }

/// Sandboxed JavaScript. Used by the `execute` agent tool (§7.3) AND by script nodes on the canvas (§4.1).
/// Today: QuickJS via `rquickjs` (native and WASM). Alternatives: Boa, a WASM-isolated V8.
pub trait ScriptRuntime {
    fn run(&self, src: &str, api: &dyn ScriptApi, limits: ScriptLimits) -> Result<ScriptOutput, ScriptError>;
}
pub struct ScriptLimits { pub time_ms: u32, pub max_ops: u32, pub max_nodes: u32, pub memory_mb: u32 }
// No network, file system, timers or async. Math.random is seeded per run, so output is deterministic.

/// Runtime shaders for shader fills and effects. Our shader dialect is a small, documented subset;
/// the compiler turns it into what the active renderer runs. Today: SkSL for CanvasKit and skia-safe.
/// Alternatives: WGSL for Vello. Importing GLSL ES fragment shaders goes through the same port.
pub trait ShaderCompiler {
    fn compile(&self, src: &str, target: ShaderTarget) -> Result<CompiledShader, ShaderError>;
}
```

The `ScriptApi` handed to scripts is a thin binding over **Ops** (§5): every call a script makes becomes an ordinary validated op, so a script can never do anything the op layer doesn't allow.

The `Engine` struct in core is constructed with these as generics/trait objects. Hot paths (layout, spatial index) use generics for zero-cost dispatch; cold paths (codec, assets) use `dyn`.

### 3.2 Rendering port

The core never draws. It produces a **display list** (our types, versioned), and a renderer consumes it.

```rust
pub enum DrawCmd {
    Save, Restore, Transform(Affine), Clip(ClipShape),
    Path { path: PathRef, paint: Paint },
    GlyphRun { font: FontHandle, glyphs: GlyphSlice, paint: Paint },
    Image { asset: AssetHash, dst: Rect, fit: ImageFit },
    Effect { kind: EffectKind },            // shadows, blurs
    Layer { opacity: f32, blend: BlendMode },
}
```

```ts
// TypeScript side, packages/renderer-api
export interface Renderer {
  readonly id: string;                       // "canvaskit" | "vello" | "canvas2d"
  init(surface: HTMLCanvasElement, opts: RendererOptions): Promise<void>;
  draw(list: DisplayListView, viewport: Viewport): void;
  resize(w: number, h: number, dpr: number): void;
  capabilities(): RendererCaps;              // effects, blend modes, max texture
  dispose(): void;
}
```

| Implementation | Where | Status |
|---|---|---|
| `renderer-canvaskit` (TS, WebGL2) | Editor | v1 default |
| `render-skia` (Rust, `skia-safe`) | Daemon, CLI, workers | v1 default |
| `renderer-canvas2d` (TS) | Editor fallback, tests | v1, no effects; also the reference fake |
| `render-vello` (Rust → WASM, WebGPU) | Editor + native | Later, behind flag |

The golden suite renders every fixture through every renderer and compares against reference images with a per-renderer tolerance.

### 3.3 Editor ports (TypeScript)

| Port | Purpose | Implementations |
|---|---|---|
| `Host` | File I/O, watching, daemon discovery, opening links, clipboard, theme | `host-vscode`, `host-electron`, `host-browser`, `host-fake` |
| `Renderer` | §3.2 | `canvaskit`, `canvas2d` |
| `EngineClient` | Typed wrapper over the WASM engine, so UI never touches `wasm-bindgen` output | `engine-wasm`, `engine-fake` (tests, Storybook) |
| `DaemonClient` | Live-mode link to the daemon | `daemon-ws`, `daemon-fake` |
| `SyncClient` | Multiplayer link | `sync-ws`, `sync-noop` |
| `Telemetry` | Opt-in events and errors | `telemetry-posthog-sentry`, `telemetry-noop` (default) |

```ts
export interface Host {
  readFile(uri: string): Promise<Uint8Array>;
  writeFile(uri: string, data: Uint8Array): Promise<void>;     // atomic
  watch(uri: string, onChange: () => void): Disposable;
  discoverDaemon(): Promise<DaemonEndpoint | null>;
  openExternal(url: string): void;
  clipboard: { read(): Promise<ClipboardData>; write(d: ClipboardData): Promise<void> };
  theme(): ThemeInfo;
}
```

**The editor is split into two packages** so the UI framework is itself replaceable:

- `editor-core`: framework-agnostic controller. Tool state machines (select, frame, pen, text, hand), command registry, keybindings, selection, viewport math. Plain TS + Zustand vanilla stores. No React.
- `editor-react`: panels and chrome built on shadcn / Base UI, subscribing to `editor-core` stores.

Tools, commands and panels are registered through **registries** (`registerTool`, `registerCommand`, `registerPanel`). Built-in features use the same registries a future plugin API will, so plugins become a matter of exposing what already exists.

### 3.4 Agent and AI ports (daemon)

**The tool registry is the product's real API.** MCP is one adapter over it.

```rust
pub trait Tool {
    fn name(&self) -> &'static str;              // "insert_nodes"
    fn version(&self) -> u32;
    fn input_schema(&self) -> schemars::Schema;
    fn run(&self, ctx: &ToolCtx, input: Value) -> Result<Value, ToolError>;
}
```

| Front end (adapter) | Uses the registry via |
|---|---|
| MCP server | `rmcp`, stdio + streamable HTTP |
| CLI | `clap` subcommands generated from tool schemas |
| Built-in agent (`AgentEngine`) | Agent loop calling tools directly |
| Future protocols | New adapter only; tools unchanged |

`ToolCtx` routes to a **`DocumentTarget`** port with two implementations: `LiveTarget` (forward ops to the editor that has the file open) and `HeadlessTarget` (native engine + file). Tools don't know which they're talking to.

```rust
pub trait ModelProvider {                 // built-in agent and `vellum prompt`
    fn id(&self) -> &str;
    fn capabilities(&self) -> ModelCaps;  // tools, vision, context size
    async fn complete(&self, req: ChatRequest) -> Result<ChatStream, ModelError>;
}

pub trait SecretStore {                   // API keys
    fn get(&self, key: &str) -> Option<SecretString>;
    fn set(&self, key: &str, value: SecretString) -> Result<(), SecretError>;
}
```

| Port | Implementations |
|---|---|
| `ModelProvider` | `openai-compat` (Chat Completions: **OpenRouter**, default `deepseek/deepseek-v4.1-flash`; DeepSeek, xAI, Moonshot/Kimi, Together, Fireworks, Z.AI, OpenCode, Ollama, LM Studio, any custom endpoint), `openai-responses` (OpenAI and compatible Responses endpoints), `anthropic` (Anthropic API, AWS Bedrock, Google Vertex, Microsoft Foundry, custom Messages endpoints), `google` (Gemini via AI Studio key), `fake` (scripted responses for tests). Reasoning effort is a request field, passed through where the model supports it. Each provider reports a status for the UI: Connected, API key set, Signed in, Configured externally (not tested by us), Failed. |
| `SecretStore` | `keyring` (OS keychain), `env`, `memory` |
| `PromptPack` | Versioned system prompts loaded from files, not hard-coded, so they can be tuned without a release. A pack (`packages/prompt-packs/packs/`) has an `id`/`version` header and `system` (with a `{{skills}}` slot), `self-check` and `summarize` sections (ADR-024) |

The agent-first editor (DESIGN.md) needs four more ports. Each is a swap point for the same reasons as the rest.

```rust
/// Who actually runs a turn. The editor and CLI only talk to this (ADR-024).
pub trait AgentEngine: Send + Sync {
    fn id(&self) -> &str;                                  // "builtin", "claude-code", "codex"
    fn availability(&self) -> EngineStatus;                // ready, missing, signed out, usage limit, failed
    // Returns at once; the stream borrows the bridge and is not Send (the daemon's documents live on one thread).
    // Failures are the final event, `Ended(Failed(EngineError))`; every turn ends with exactly one `Ended`.
    fn run_turn<'a>(&'a self, turn: TurnRequest, tools: &'a dyn ToolBridge)
        -> Result<Box<dyn TurnStream + 'a>, EngineError>;
    fn interrupt(&self, turn: &TurnId);                    // from any thread; unknown turns are ignored
}

/// The engine's only way to the canvas: the tool registry, called as the turn's agent (steps, Guard, origin).
pub trait ToolBridge {
    fn tools(&self) -> Vec<BridgeTool>;                    // name, description, schema, read_only
    fn call(&self, name: &str, input: Value) -> ToolReply; // JSON, or the error as JSON with `failed`
}

/// Conversations live on the user's computer, never in the design file.
pub trait SessionStore {
    fn sessions(&self, doc: &DocKey) -> Vec<SessionMeta>;
    fn append(&self, session: SessionId, event: SessionEvent) -> Result<(), StoreError>;
    fn events(&self, session: SessionId, since: Option<EventSeq>) -> Vec<SessionEvent>;
    fn delete(&self, session: SessionId);
}

/// Requests from an agent to do something outside the canvas (read or write a repo file, run a command).
pub trait PermissionBroker {
    async fn request(&self, req: PermissionRequest) -> PermissionDecision; // allow once, allow for session, deny
}

/// Skills: instructions agents read before working. Built-in, per design system, and user-added SKILL.md folders.
pub trait SkillSource {
    fn list(&self) -> Vec<SkillMeta>;
    fn read(&self, path: &str) -> Result<String, SkillError>;
}
```

| Port | Implementations |
|---|---|
| `AgentEngine` | `builtin` (our loop over any `ModelProvider`: one self-check after writes; long tool results cut with a marker; old results pruned to markers, then older rounds summarized, near the context limit; Stop races every provider wait), `claude-code` (the user's installed Claude Code, run headlessly with our MCP server attached; uses whatever sign-in it already has: Claude subscription, API key, Bedrock, Vertex), `codex` (same for Codex CLI, including ChatGPT sign-in), `fake`. Later adapters: `gemini-cli`, `copilot` (GitHub sign-in), `cursor-agent`, `opencode`. A new engine is one adapter. |
| `SessionStore` | `sqlite` at `~/.vellum/sessions.db` (keyed by the file's repo-relative path + repo root), `memory` |
| `PermissionBroker` | `editor` (shows a Permission card in the Agent panel), `cli` (prompts in the terminal, or `--allow`/`--deny` flags), `policy` (pre-approved rules from settings, e.g. "read anything in this repo") |
| `SkillSource` | `builtin` (format schema, `execute` API, workflows), `design-system` (each system's `guidelines.md`), `folder` (user-added `SKILL.md` folders), stacked in that order |
| `StyleLibrary` | Visual styles (palette, type, radius, density and a short description) that agents can apply when no design system is set. `builtin` + `folder`. |

### 3.5 Export ports

| Port | Implementations |
|---|---|
| `TokenFormat` | `css-vars`, `tailwind-v4`, `dtcg-json` (W3C Design Tokens), later `swift`, `kotlin` |
| `SpecFormat` (for `get_spec`) | `layout-spec-v1` (framework-neutral); stack hints are a separate, swappable `StackProfile` (`shadcn`, `react-native`, `swiftui`) |
| `ImageExporter` | `png`, `jpeg`, `webp`, `pdf` (one page per frame, or slide order for presentations), `svg` |
| `CodeExporter` | `html-tailwind`, `html-css` (with or without a full HTML scaffold; assets inlined or as files). Deterministic, no AI. Framework code (React, shadcn, SwiftUI) stays agent-written from `get_spec`. |

Each is a small trait with a registry, so adding SwiftUI tokens is one new file plus a contract-suite pass.

### 3.6 Import and asset ports

| Port | Purpose | Implementations | Release |
|---|---|---|---|
| `Importer` | Turn foreign content into our `DocumentData` subtree | `svg`, `image` (PNG/JPEG/WEBP as image nodes), `figma-clipboard` (paste), `figma-file` (via Figma's REST API with the user's token; reading a locally exported `.fig` file only after legal review), `html` (DOM + computed styles → frames, from `BrowserPort`) | `svg`, `image`, `figma-clipboard` v1; the rest v1.1 |
| `BrowserPort` | Load a URL (including `localhost` dev servers), screenshot the page or an element, return a DOM + computed-style snapshot for the `html` importer | `chromium-cdp` (desktop: Electron's own Chromium; daemon: a managed headless Chromium via CDP), `fake` | v1.1 |
| `IconLibrary` | Searchable icon sets inserted as vector nodes | `lucide`, `material-symbols` (outlined, rounded, sharp), `phosphor`, `feather`, user folder of SVGs | v1 |
| `ImageSource` | Fill image slots with real pictures | `stock` (a licensed stock API, key in `SecretStore`), `generate` (any image model through `ModelProvider`-style adapter), `local` | `stock` v1, `generate` v1.1 |
| `ImageOps` | Remove background, trace image to vectors | `local` (ONNX segmentation model, `vtracer` for tracing), `fake` | v1.1 |

All importers produce `DocumentData` and go through Ops like everything else, so imported content is undoable, attributed and diffable.

### 3.7 Cloud service ports

Cloud code (TypeScript and Rust) talks to vendors only through these packages:

| Port | Package / crate | Today | Alternatives ready by design |
|---|---|---|---|
| `Auth` (`getSession`, `requireUser`, `verifyDeviceCode`) | `@vellum/auth` | BetterAuth | Any OIDC provider, Clerk |
| `Billing` (`createCheckout`, `getEntitlements`, `parseWebhook`) | `@vellum/billing` | Polar.sh | Paddle, Stripe |
| `Entitlements` (read-only, used by sync server) | table + `@vellum/billing` | Postgres table written by billing webhooks | — |
| `ObjectStore` | `@vellum/storage`, `adapters/assets-s3` | Cloudflare R2 (S3 API) | Backblaze B2, MinIO for self-host |
| `Mailer` | `@vellum/mail` | Amazon SES | OpenSES, SMTP |
| `JobQueue` (`enqueue`, `worker`) | `@vellum/jobs` | BullMQ + Redis | pg-boss (self-host without Redis) |
| `UpdateLog` + `SnapshotStore` (sync server) | `sync-server::ports` | Postgres + R2 | SQLite + local disk (self-host image) |
| `Presence` | `sync-server::ports` | Redis | In-memory (single node, self-host) |
| `Search` (later) | `@vellum/search` | Meilisearch | Postgres full-text |
| `Telemetry` | `@vellum/telemetry` | PostHog + Sentry (cloud) | No-op, OpenTelemetry |

Billing is the clearest example of why: **app code asks "does this user have `cloud_sync`?"**, never "is this a Polar subscription?". Entitlements are our vocabulary; the billing adapter maps vendor plans onto them.

---

## 4. The document model

### 4.1 Types (`vellum-types`)

```
Document
 ├ meta        {formatVersion, name, createdWith, kind: document | library}
 ├ imports     alias → relative path of a library file (*.lib.vellum)
 ├ themes      axis → ordered values, e.g. {Mode: [Light, Dark], Device: [Phone, Tablet]}; first value is the default
 ├ variables   id → {name, type: color | number | string | boolean,
 │                   values: [{value | $alias, when: {axis: value}}]}   last matching entry wins
 ├ styles      text styles, effect styles
 ├ fonts       custom fonts carried with the document (files in <name>.assets/fonts/)
 ├ components  id → node tree + props + variants
 └ pages[]     id, name, kind: canvas | slides, children: Node[]

Node (tagged union)
  frame      layout: none | row | column | grid; gap; padding (1, 2 or 4 values); justify: start | center | end
             | space-between | space-around; align; layoutIncludesStroke; clip; theme? {axis: value};
             slot? {suggested: componentId[]}
  group      children, effects
  rect       cornerRadius (1 or 4 values)
  ellipse    innerRadius, startAngle, sweepAngle            (rings, arcs, pie slices)
  polygon    sides, cornerRadius        star   points, innerRatio        line
  vector     path (SVG path data), viewBox, fillRule: nonzero | evenodd
  boolean    op: union | subtract | intersect | exclude, children
  text       content (rich runs: font, weight, style, size, lineHeight, letterSpacing, underline,
             strikethrough, link), align: left | center | right | justify, verticalAlign: top | middle | bottom,
             resize: auto-width | auto-height | fixed, textStyle?
  image      asset, fit: fill | fit | stretch, crop (affine transform)
  icon       library, name, weight (100–700), fill
  instance   componentId, variant, props, overrides {nodeIdPath → props | replacement node | children},
             slotContent {slotId → Node[]}
  script     src (.js file relative to the document), inputs {name → value | $var}, clip
             children are derived output: rendered, never saved, not in undo history
  note       sticky note for people (content, colour); agents can read it
  context    canvas-placed instructions for agents (content); read with the frame it sits in or points at
  prompt     a saved request on the canvas (content, engine/model?); Run sends it to the agent panel
  embed      live web view (url, device, zoom, scroll, radius, stroke, effects) — v1.1, uses BrowserPort

Common to every node: id, name, visible (or $boolean var), locked, opacity, rotation, flipX, flipY,
                      position: auto | absolute (inside a layout), width/height: number | $var | hug | fill
                      (each with an optional fallback), fills[], stroke?, effects[], blendMode,
                      agentNotes?, prototype? (link to frame + transition), metadata {key → JSON}

Paint (fills and stroke paints, painted in order; each can be hidden):
  solid      color | $var
  gradient   linear | radial | angular, stops [{color, position}], transform
  mesh       columns, rows, points with optional handles, colors
  image      asset, fit, crop, opacity
  shader     src (runtime shader file), uniforms {name → number | bool | color | vecN | $var}

Stroke:   paints[], width (uniform or per side), align: inside | center | outside, cap, join, dash
Effects:  drop-shadow, inner-shadow (offset, blur, spread, color, blendMode), layer-blur,
          background-blur, shader (src, uniforms; can sample the node's own rendering)
BlendMode: normal, darken, multiply, color-burn, linear-burn, lighten, screen, color-dodge, linear-dodge,
          overlay, soft-light, hard-light, difference, exclusion, hue, saturation, color, luminosity

Every property can be a literal or a variable reference: {"$var": "color/primary"}
```

- **Themes and modes.** Variables can vary along several **axes** at once (Mode: Light/Dark, Device: Phone/Tablet, Brand: A/B). A frame can set a theme value for its subtree; children inherit it unless they set the same axis themselves. The first value of each axis is the document default.
- **Aliases.** A variable's value can reference another variable of the same type.
- **Overrides.** Instances can override any property, replace a descendant node entirely, or replace its children, addressed by an ID path (`card/actions/button`). Inherited layers can't be reparented inside an instance; that's what slots are for.
- **Slots** are empty frames inside a component that instances fill with their own content. `suggested` lists components that usually go there; suggestions guide humans and agents but never restrict.
- **Script nodes** point at a `.js` file beside the document. The file's header declares inputs (`@input columns: number(min=1) = 3`), which become controls in the properties panel; inputs can bind to variables. The `ScriptRuntime` port runs it with hard limits (1,000 nodes, 2 s, no I/O, seeded randomness) and its returned nodes render inside the script node's bounds. **Convert to layers** replaces it with a frame holding a snapshot of the output. Scripts use the same node schema as the file, so agents can write them.
- **Shaders** are runtime shader files beside the document, used as a fill or an effect. Uniforms can bind to variables; the renderer supplies the node's size and, for effects, its rendered content. A `ShaderCompiler` port (§3.1) turns our shader dialect into whatever the active renderer runs.
- **Agent context on the canvas.** `agentNotes` (on any node), `context` nodes (a block of instructions placed beside frames) and `note` nodes (sticky notes for people) are all returned by `get_app_state`, `get_nodes` and `execute` reads; skills tell agents to respect notes and context. `prompt` nodes keep a reusable request on the canvas next to what it's about.
- **Slides** are frames on a `slides` page, ordered for presenting and for multi-page PDF export.
- **Metadata** is free-form JSON per node for tooling and agents (e.g. `{"source": "src/components/Card.tsx"}`); the editor ignores it.

These types are plain Rust structs with `serde`, `schemars` and `ts-rs`. They are the shared language of every port; no port signature mentions a vendor type.

### 4.2 `DocStore` today: Loro

The Loro adapter (`adapters/doc-loro`) maps our tree onto Loro's **movable tree** (reparenting merges correctly), maps for properties and lists for child order. Undo, history and binary sync updates come from Loro, exposed only through the `DocStore` trait.

### 4.3 On-disk format: `.vellum`

- **Deterministic serialization:** keys in a fixed order, 2-space indent, numbers normalized.
- **Stable IDs** so a moved node diffs as a move.
- **Assets** beside the file in `<name>.assets/`, by content hash, via `AssetStore`: images, custom fonts (TTF, OTF, WOFF, WOFF2; they travel with the document) and shader files. Script `.js` files and library files are referenced by relative path, so a design folder can be moved or zipped as a unit.
- **Directory format** (`app.vellum/`, one file per page) as a second `Codec` for large documents.
- **Spec and schema** published in `docs/format/`, generated from `vellum-types`.

### 4.4 Libraries

- **A library is a document with `kind: library`**, saved as `*.lib.vellum`. Turning a document into a library is a one-way change (the editor asks first).
- **Importing** adds an alias to `imports` with a relative path. Instances from a library keep a link; when the library changes, importing documents pick up the change on next open (and live, when both are open in the same editor).
- **Overrides win** over library changes, as with local components.
- **Pasting between documents:** content from a library into a document that hasn't imported it asks **Link to library** (adds the import) or **Paste as copies** (independent). Pasting from an ordinary document copies without a link. Variables (and their theme values) travel with pasted content; on a name clash the person chooses **Use existing** or **Add renamed**.
- **Missing library:** if the path no longer resolves, the editor shows the library as missing and offers **Locate…**; instances keep rendering from the last known copy cached in `~/.vellum/cache/`.
- **Starter design systems** ship as libraries.

### 4.5 External changes and Git

- **External change** (`git checkout`, merge, an agent editing the file directly): the daemon's watcher fires, core **diffs old vs new `DocumentData` by node ID** and applies the difference as ops. Open editors update in place.
- **Merge driver:** `vellum merge-driver` does a three-way merge by node ID and property, leaving a conflict marker node only when the same property changed on both sides. It works on `DocumentData`, so it doesn't care which `DocStore` or `Codec` is in use.

---

## 5. `vellum-core` modules

| Module | Responsibility | Ports it uses |
|---|---|---|
| **Ops** | The only way to change a document: `insert`, `update`, `delete`, `move`, `copy`, `replace`, `set_variable`, `create_component`, `detach`, `toggle_component`, `replace_in_instance`, `make_slot`, `convert_script`, `wrap_in_frame`, `group` / `ungroup`, `boolean`, `align` / `distribute`, `reorder`, `flip`, `scale` (resizes content, text, strokes, radii, spacing and effects together; variable-bound sizes become fixed values and instances get overrides), `outline_stroke`. Validated, applied as one transaction, one undo step. Every transaction carries an **origin** `{actor, session?, turn?}`, which is what makes per-person undo, "Undo this turn" and the History ledger possible. | `DocStore`, `IdGen` |
| **Guard** | Checked before every agent transaction: is this actor paused, is the target inside a frame leased by someone else, is the batch within limits. Rejections are returned to the agent with a reason and recorded in History. Human edits skip lease checks (your edits win). | — |
| **Layout check** | Finds problems agents should fix: text overflowing its box, children clipped by a parent, overlapping siblings in a flex container, nodes off any frame. Feeds `snapshot_layout(problems_only)`. | `TextEngine`, `SpatialIndex` |
| **Resolver** | Variables (per mode), component instances and overrides → resolved tree. Cached per subtree. | — |
| **Layout** | Builds the `LayoutTree` from resolved frames; incremental via dirty sets. | `LayoutEngine`, `TextEngine` |
| **Scene** | Hit-testing, marquee, snapping, viewport culling. | `SpatialIndex` |
| **Display list** | Visible nodes → `DrawCmd` buffer in shared memory. | `FontSource`, `PathOps` |
| **Diff / merge** | `DocumentData` diff → ops; three-way merge. | — |
| **Export** | SVG, specs, tokens via registries. | `TokenFormat`, `SpecFormat` |

**WASM boundary:** the engine owns the document. JS sends ops and input events in and gets back display-list buffers (shared memory, no JSON) plus change notifications. The React UI never holds its own copy of the tree.

---

## 6. Clients

| Layer | Choice | Swap point |
|---|---|---|
| Build | Vite (static bundle; a VS Code webview can't run a server) | — (Tier C) |
| Controller | `editor-core` (TS, Zustand vanilla) | Framework-agnostic by design |
| UI | `editor-react`: React + shadcn + Base UI | Replaceable without touching `editor-core` |
| Canvas | `Renderer` port, CanvasKit on WebGL2 | §3.2 |
| Overlays | Second lightweight canvas: handles, guides, agent cursors and leases | Owned by `editor-core` |
| i18n | `i18next`, your 13 target languages, RTL layout for Arabic | Strings in `locales/`, not code |

### Hosts

| Host | Stack | Notes |
|---|---|---|
| **VS Code extension** | `CustomEditorProvider` for `*.vellum`, editor in a webview, `host-vscode` | VS Code Marketplace + Open VSX (Cursor, Windsurf); one-click MCP registration |
| **Desktop** | Electron, `host-electron`, daemon as sidecar | Same Chromium as VS Code, so rendering matches. Signed builds for macOS (Apple Silicon and Intel), Windows (x64, ARM64) and Linux (AppImage, `.deb`, tarball; x64 and ARM64). Auto-update with Check for Updates in the menu. |
| **Browser** (later) | Next.js page, `host-browser` | Cloud files and share links only |

**Host behaviours.**
- **VS Code commands:** Vellum: New Design File, Toggle Design / Text View (open the same file as JSON), Open Welcome File, Connect Agents, Pause All Agents. An Activity Bar icon opens a sidebar with recent design files and agent sessions.
- **Drafts (desktop):** "New file" without a folder creates a draft in `~/.vellum/drafts/` that saves automatically; **Save As…** moves it into a project.
- **Crash recovery (desktop):** unsaved changes to an existing file are written to a recovery copy in `~/.vellum/recovery/` every few seconds. After an unexpected quit, reopening offers Save, Save As…, Discard backup or Cancel. Recovery copies are not history; Git is. In the IDE, VS Code's own hot exit does this job.
- **Local preferences** (pixel grid, snapping, nudge amounts, zoom behaviour, animations) are stored per user by the host, never in the file.

**Electron over Tauri** (ADR-0006): Tauri uses each OS's web engine, so WebGL, fonts and canvas behave slightly differently per platform. *Revisit when* Tauri can bundle a single engine or our golden suite passes on WebKit and WebView2. Because the shell only implements `Host`, switching is a contained job.

---

## 7. The `vellum` daemon

One static Rust binary: MCP server, CLI, agent runner, headless renderer and file watcher. Composition root for native.

| Need | Crate |
|---|---|
| MCP adapter | `rmcp` |
| Local IPC to editors and CLI | `tokio` + `interprocess` (Unix domain socket on macOS/Linux, named pipe on Windows) |
| Script runtime (`execute`, script nodes) | `rquickjs` behind `ScriptRuntime` |
| File watching | `notify` |
| Headless rendering | `render-skia` adapter |
| Headless browser (v1.1) | `chromiumoxide` (CDP) behind `BrowserPort` |
| Session store | `rusqlite` behind `SessionStore` |
| CLI adapter | `clap` |
| Logging | `tracing` |

### 7.1 Live vs headless

Per file: if an editor holds it, `LiveTarget` forwards ops to that editor (canvas updates in real time, agent cursors shown). Otherwise `HeadlessTarget` loads it into a native engine, applies ops, renders and saves. External agents work in both modes, so an agent can design with no app open, in CI, or on a server.

### 7.2 Local connection security

- The daemon listens on a **Unix domain socket** (macOS/Linux) or **named pipe** (Windows) inside `~/.vellum/run/`, a directory only the user can read. There is no TCP port, so no web page in any browser can reach it, which removes the whole DNS-rebinding class of attacks.
- Editor webviews never connect directly: the VS Code extension host and Electron's main process hold the connection and relay messages to the webview.
- Each connection still presents a token from `~/.vellum/run/daemon.json` (0600) and a protocol version.
- A TCP listener exists only behind an explicit `vellum daemon --listen` flag for remote dev containers, with the token required and `Origin`/`Host` checks on.

### 7.3 Tools

**Two layers over one registry.** Both are thin front ends over Ops (§5), so they share validation, the Guard, attribution and undo.

**Layer 1 — `execute`: one script, many operations.** The agent sends a short JavaScript program that runs in the `ScriptRuntime` sandbox against a `doc` API. Loops and variables let one call do what would otherwise take dozens of calls, which cuts tokens and round trips sharply. The whole script is **one transaction and one undo step**; if any op fails, nothing is applied and the agent gets the failing line and reason.

```js
// execute({ code })
const page = doc.page("Onboarding");
const step = doc.insert(page, { type: "frame", name: "Step 2", width: 390, height: 844,
                                layout: "column", gap: "$space/4", padding: 24, fill: "$color/surface" });
for (const habit of ["Drink water", "Read 10 pages", "Walk 20 min"]) {
  doc.insert(step, { component: "HabitRow", props: { label: habit } });
}
doc.layoutProblems(step);           // overflow, clipping, overlaps
return doc.screenshot(step, { scale: 1 });   // image returned to the agent
```

- The `doc` API is ours and documented in the `execute` skill: `page`, `get`, `query`, `insert`, `update`, `copy`, `replace`, `move`, `delete`, `setVariables`, `createComponent`, `makeSlot`, `instance`, `findSpace`, `layoutProblems`, `screenshot`, `export`, `fillImage`, `icon`, `claim`, `release`.
- Limits per call: 5 s, 2,000 ops, 64 MB. No network, file system or timers.
- `execute({ code, validate_only: true })` dry-runs the script and reports what would change.
- The API is versioned (`doc.apiVersion`); breaking changes ship as a new major version with the old one kept for two minor releases (§2.5).

**Layer 2 — typed tools.** One tool per operation, for clients and smaller models that do better with schemas than with code, and for quick reads.

| Group | Tools |
|---|---|
| **Context** | `get_app_state` (open documents, active document, selection, viewport, leases, paused agents, Notes for agents in scope), `open_document` (open or create a file in the editor or headlessly) |
| **Skills and styles** | `list_skills`, `read_skill(path?)` (format schema, `execute` API, workflows, design-system guidelines, user skills), `get_style(name?)` (list or load a visual style) |
| **Read** | `get_nodes` (subtrees by ID, depth-limited), `query_nodes` (by name, type, component, text, variable use), `snapshot_layout(node?, problems_only?)` (structure with computed bounds, or just layout problems), `find_properties` (all distinct values of a property in a scope, e.g. every font size used), `get_variables`, `list_components` |
| **Write** | `execute`, `insert_nodes` (JSON or compact JSX-like spec), `update_nodes`, `copy_nodes`, `replace_nodes`, `move_nodes`, `delete_nodes`, `replace_properties` (change every matching value in a scope, e.g. `#111` → `$color/text`), `set_variables`, `create_component`, `create_instance`, `make_slot` |
| **Space and checks** | `find_space(width, height, near?)` (free area on the canvas for new frames), `screenshot(node, scale?)` |
| **Assets** | `list_fonts` (available and custom fonts), `search_icons` / `insert_icon`, `fill_image(node, query \| prompt \| path)` (stock, generated or local), `import(url \| path)` (SVG, image, Figma file, web page) |
| **Code and tokens** | `get_spec(frame, stack?)`, `export(nodes, format)` (png, jpeg, webp, pdf, svg, html-tailwind, html-css), `import_tokens` / `export_tokens` |
| **Coordination** | `claim_frame` / `release_frame`, `spawn_agents(tasks[], mode)` (§7.5), `report_progress` (updates the plan card and turn) |
| **Browser** (v1.1) | `browser(url, action: screenshot \| import \| import_element, selector?)`, including `localhost` dev servers |

Mutating tools take a batch, run as one transaction and one undo step, and return created IDs. A bad op rejects the batch with a precise error the agent can act on.

### 7.4 pen.dev parity check

Checked against pen.dev's public docs (October 2026) and its earlier tool set as documented by its community. Vellum covers every capability, under its own names and API.

**MCP tools**

| pen.dev | Vellum | Release | Note |
|---|---|---|---|
| `execute` (read, modify, render, export via a script) | `execute` | v1 | Same idea, our own `doc` API |
| `get_app_state` (document metadata, selection) | `get_app_state` | v1 | Also returns leases, paused agents, Notes for agents |
| `read_skill` (schema, `execute` docs, workflows) | `read_skill`, `list_skills` | v1 | Also serves design-system guidelines and user skills |
| `get_style` (list or load a visual style) | `get_style` | v1 | Backed by `StyleLibrary` |
| `browser` (desktop app only) | `browser` | v1.1 | Desktop and headless daemon |
| `spawn_agents` (behind a start-up flag) | `spawn_agents` | v1 | On for built-in engines; a setting for external clients |
| Earlier `batch_design` (insert, copy, update, replace, move, delete, image ops) | `execute`, typed write tools, `fill_image` | v1 | |
| Earlier `batch_get` | `get_nodes`, `query_nodes` | v1 | |
| Earlier `snapshot_layout` (incl. problems only) | `snapshot_layout` | v1 | Backed by core's Layout check |
| Earlier `get_screenshot` | `screenshot` | v1 | |
| Earlier `find_empty_space_on_canvas` | `find_space` | v1 | |
| Earlier `get_variables` / `set_variables` | same names | v1 | |
| Earlier `search_all_unique_properties` / `replace_all_matching_properties` | `find_properties` / `replace_properties` | v1 | |
| Earlier `get_guidelines`, `get_style_guide` | `read_skill`, `get_style` | v1 | |
| Earlier `get_editor_state`, `open_document` | `get_app_state`, `open_document` | v1 | |
| Earlier `export_nodes` | `export` | v1 | |

**Everything around the tools**

| pen.dev capability | Vellum | Release |
|---|---|---|
| Integrated agent with sessions, model picker, reasoning effort, Stop | Agent panel, `AgentEngine`, effort per request | v1 |
| Claude Code, Codex and other providers as the agent (including sign-in with a ChatGPT account for Codex) | `claude-code` and `codex` engines use the user's own installed, signed-in tools; API providers via `ModelProvider` | v1 |
| Selection and attached files/images as context | Composer context chips, @ mentions, drag and drop | v1 |
| Skills via `/` menu, custom `SKILL.md` folders | `SkillSource` (`folder`), `/` menu in the composer | v1 |
| Permission requests for file and command actions; auto mode; skip-all | `PermissionBroker` with Ask, policy rules, engine auto mode, skip-all (off, with warning) | v1 |
| Context usage and estimated cost | Turn footer + usage popover; cost from a local price table, always labelled as an estimate | v1 |
| Notification when an agent finishes in the background | Host OS notification on turn end (setting) | v1 |
| Parallel agents 1–6: Split work, Side by side, per-agent model | Parallel stepper with **Split** and **Side by side** modes; per-lane model in Side by side | v1 |
| "Let it cook": successive variants exploring Layout or Style, 2–6 | **Explore** mode: N successive variants varying Layout or Style | v1 |
| Stop current agent / Stop all | Lease-tab Stop, session Stop, Pause all | v1 |
| Script nodes (code on canvas: `.js` file, header inputs, sandbox, convert to layers) | `script` node + `ScriptRuntime` | v1 |
| Slots with suggested components | `slot` on frames inside components | v1 |
| Components, variables, design libraries | Components, variables + modes, libraries | v1 |
| Built-in icon sets (Material Symbols, Lucide, Feather, Phosphor) | `IconLibrary` with the same open-source sets | v1 |
| Image import, PNG/JPEG/SVG; remove background; image to vectors | `Importer` (`image`, `svg`); `ImageOps` | v1 / v1.1 |
| Figma file import and layer paste | `figma-clipboard` v1, `figma-file` v1.1 | v1 / v1.1 |
| Web page or element import via built-in browser, screenshots | `BrowserPort` + `html` importer | v1.1 |
| HTML + Tailwind / HTML + CSS export, scaffold option | `CodeExporter` | v1 |
| Export PNG, JPEG, WEBP, PDF at 1×–3× | `ImageExporter` | v1 |
| Slides and presentations | `slides` page kind, presenting, PDF in slide order | v1.1 |
| Share a design: snapshot link, Update snapshot, revoke, recipients download a ZIP, missing-dependency warning | Share snapshots (Pro, §9) | v1 |
| CLI: agent mode, interactive shell, app and headless modes, exports, models list, verbose mode, CI keys | `vellum` CLI (§7.8) | v1 |
| One-click MCP setup for Claude Code, Codex (also ChatGPT desktop and the Codex IDE extension), Gemini CLI, Antigravity, OpenCode, Kiro, Claude Desktop; disabling removes the entry | Connect agents dialog writes and removes each client's MCP config entry | v1 |
| Many providers: Claude subscription / API / Bedrock / Vertex / Foundry, ChatGPT sign-in / OpenAI key, Gemini, Copilot, Cursor, OpenRouter, DeepSeek, xAI, Moonshot/Kimi, Together, Fireworks, Z.AI, OpenCode, custom endpoints | `ModelProvider` adapters + installed engines (§3.4); `copilot` and `cursor-agent` engines later | v1 / later |
| Theme axes (e.g. Light/Dark × Device) with frame-level theme | `themes` axes + `theme` on frames (§4.1) | v1 |
| Boolean variables for visibility, aliases | Variable types and aliases (§4.1) | v1 |
| Fills: colour, linear / radial / angular / **mesh** gradients, image (fill, fit, stretch, crop), **shader**; 18 blend modes | Paints and `ShaderCompiler` (§4.1, §3.1) | v1 (shaders v1.1) |
| Effects: drop and inner shadow, layer and background blur, shader effect | Effects (§4.1) | v1 (shader effect v1.1) |
| Strokes with per-side width, alignment, caps, joins | Stroke (§4.1) | v1 |
| Ellipse arcs and rings, polygons, paths with fill rule | Shapes (§4.1) | v1 |
| Text: underline, strikethrough, links, vertical alignment, justify, auto-width / auto-height / fixed | Text node (§4.1) | v1 |
| Sticky notes, prompt nodes, context nodes on the canvas | `note`, `prompt`, `context` nodes | v1 |
| Live website (browser) node on the canvas | `embed` node | v1.1 |
| Custom fonts stored with the document | `fonts` + assets (§4.1, §4.3) | v1 |
| Scale tool (K) that scales content proportionally | `scale` op | v1 |
| Component toggle back to element, Replace inside instance, detach keeps nested links | Ops (§5) | v1 |
| Libraries (`.lib.pen`), default libraries, paste with link or copies, variable conflict dialog, Locate missing library | Libraries (§4.4) | v1 |
| Remove background, vectorize image, agent edits an image (e.g. new background) | `ImageOps` + image-model adapter | v1.1 |
| Desktop dashboard: Recents, Drafts, Design Systems, list/grid, sort, multi-select | Desktop home (DESIGN A02) | v1 |
| Crash recovery backups (desktop); Reload/Ignore on external change (IDE) | Recovery copies; external changes applied as ops (§4.5, §6) | v1 |
| Settings: pixel grid, snap to pixel grid / objects, scroll-wheel zoom, invert zoom, nudge amounts, animations off | Local preferences (§6) | v1 |
| Shader gallery and other examples ("Goodies") | Examples gallery: shaders, scripts, design systems | v1.1 |
| Hosted models included with a paid account | Not in v1 (BYO keys and installed engines only); possible later | later |

**Where Vellum deliberately differs**
- **No account needed locally.** The CLI, MCP server, exports and built-in agent all work without signing in; only cloud features need an account.
- **External agents don't need the app open.** Headless mode serves MCP clients directly.
- **The file is open and documented**, so tools can read `.vellum` without going through our server.

### 7.5 Multiple agents and orchestration

- **Identity.** Every agent (external MCP session, built-in lane, spawned helper) gets an `ActorId`, a name and a hue. All its transactions carry that origin.
- **Leases.** `claim_frame` leases a frame (60 s idle expiry). The Guard rejects other agents' writes inside it with "frame leased by X"; human edits are never blocked.
- **Plans.** For multi-frame requests the engine first produces a plan (frames, pages, intents). Approving it creates **placeholders**: lease records with planned bounds (from `find_space`), drawn as overlays and visible to every agent through `get_app_state`. They're session state, not file content, so an abandoned plan leaves nothing in the file.
- **Parallel modes.**
  - *Split:* the orchestrator divides plan items into lanes; each lane is its own engine run with its own leases.
  - *Side by side:* each lane gets the same brief and its own free area; lanes may use different models.
  - *Explore:* after the first result, the engine produces N successive variants that vary only Layout or only Style.
- **`spawn_agents`** lets an agent (built-in or external) start helper lanes with explicit tasks; they report back through `report_progress`.
- **Pause and Stop** flip state the Guard checks on every agent transaction, so a paused agent's next write is rejected immediately, even if its engine keeps running.
- **Undo this turn** reverts every transaction whose origin carries that turn ID, using the same inverse-op machinery as History's Revert, and never touches other actors' changes.

### 7.6 Sessions and turns

- `SessionStore` keeps an append-only event log per session: your message and context, plan, each step (tool, arguments preview, result preview, thumbnail reference, duration, outcome), usage (input/output tokens, cache hits), and the turn's origin ID.
- Thumbnails and screenshots are cached in `~/.vellum/cache/` by content hash, not stored in the database.
- Sessions are keyed by repo root + the file's repo-relative path, so renaming a branch doesn't lose them. Deleting a session removes its events and cached images.
- External MCP sessions are recorded the same way (their tool calls only), which is what lets them appear in the Agent panel and History.

### 7.7 Permissions

Canvas edits never need permission: they're attributed, visible and undoable. Anything **outside** the canvas does:

| Action | Default |
|---|---|
| Read a file in the current repo (specs, code, CSS) | Ask once per session, or allowed by a policy rule |
| Write a file (tokens export, script `.js`, exported HTML/images) | Ask |
| Run a command (installed engines may want to) | Ask |
| Read outside the repo, network access by the built-in agent | Ask every time |

`PermissionBroker` shows a Permission card in the Agent panel (Allow once, Allow for this session, Deny). Installed engines that have their own permission system (e.g. Claude Code's auto mode) can be left to decide when the user turns that on. "Skip all permission prompts" exists, is off by default, and shows a persistent warning while on.

### 7.8 CLI

```bash
# Agent mode
vellum prompt design/app.vellum "Add a settings screen" \
  --engine builtin --model openrouter/deepseek/deepseek-v4.1-flash --effort medium \
  --attach DESIGN.md --repo . --parallel 3 --mode split \
  --usage usage.json --preview-each-change --max-failed-calls 5
vellum prompt --tasks tasks.json            # runs every task in the file
vellum models --engine builtin              # list models per engine
vellum prompt --out new.vellum "Create a pricing page" --export pricing.png --export-scale 2
vellum prompt design/app.vellum "Match this" --attach mock.png --verbose   # stream thinking and full tool I/O
vellum version · vellum status              # versions, daemon and engine status; sign-in only matters for cloud features
# CI: keys come from env vars (VELLUM_OPENROUTER_API_KEY, ANTHROPIC_API_KEY, …); no Vellum account needed

# Interactive shell: call tools directly, live against the app or headless
vellum shell design/app.vellum              # attaches to the running editor if it has the file
vellum shell --headless -i in.vellum -o out.vellum
vellum> read_skill({ path: "execute.md" })
vellum> execute({ code: 'doc.insert(doc.page("Home"), { type: "frame", name: "Hero", width: 1440 })' })
vellum> save()

# Files
vellum new design/app.vellum --system starter-minimal
vellum export design/app.vellum --frame "Login" --format png --scale 2 -o login.png
vellum export design/app.vellum --frame "Login" --format html-tailwind --scaffold -o login.html
vellum tokens export design/app.vellum --format tailwind-v4 > app/theme.css
vellum tokens import design/app.vellum app/globals.css
vellum diff main..HEAD design/app.vellum --png
vellum mcp                                  # MCP server over stdio
vellum merge-driver
```

Every command supports `--json`. Exit codes are non-zero on any failure, including a failed export.

---

## 8. AI

- **Agent-first, model-agnostic.** Any MCP client can design, and the built-in agent runs on any `ModelProvider` or on the user's installed Claude Code or Codex via `AgentEngine`.
- **Quality levers we own:** skills (`read_skill`), styles (`get_style`), Notes for agents, `execute` for efficient multi-op edits, `snapshot_layout(problems_only)` and `screenshot` for self-checking, and plans before multi-frame work. All prompts and skills are versioned files, not code.
- **Keys** live in `SecretStore`, never in documents or repos.
- **Design → code:** `get_spec` returns layout, tokens (by variable name) and components for agents to implement; `CodeExporter` gives deterministic HTML + Tailwind or HTML + CSS.
- **Code → design:** agent-driven via `execute`, or from a running page via the `html` importer (v1.1).
- **Token sync** is deterministic (`TokenFormat`), no AI.

---

## 9. Cloud side (optional)

```
 ┌────────────────┐   ┌──────────────────────────┐   ┌──────────────────┐
 │ account app    │   │ sync server (Rust)       │   │ workers          │
 │ Next.js, tRPC  │   │ axum + tokio             │   │ JobQueue consumer│
 │ @vellum/auth   │   │ DocStore adapter (Loro)  │   │ runs `vellum`    │
 │ @vellum/billing│   │ UpdateLog · Snapshot ·   │   │ headless render  │
 │                │   │ Presence ports           │   │                  │
 └──────┬─────────┘   └──────────┬───────────────┘   └────────┬─────────┘
        └──────── Postgres (Drizzle / sqlx) · Redis · R2 ─────┘
```

| Service | Stack | Purpose |
|---|---|---|
| **Account app** | Next.js + tRPC + Drizzle via `@vellum/auth`, `@vellum/billing` | Signup, billing, dashboard, share viewer |
| **Public API** | Hono + Bun (REST, `/v1/`) | CLI and extension endpoints, billing webhooks |
| **Sync server** | Rust: axum + tokio, `DocStore` adapter, `UpdateLog` / `SnapshotStore` / `Presence` ports | One room per document; merge, persist, broadcast; writes `.vellum` snapshots on compaction (§2.3) |
| **Workers** | `JobQueue` + `vellum` binary | Exports, thumbnails, PR previews |
| **PR previews** | GitHub App + Action | Changed frames rendered on PRs |
| **Docs** | Fumadocs | User docs, MCP reference, format spec, ADRs |
| **Hosting** | Netcup RS 1000 G12 + Dokploy, Infisical, GitHub Actions | |

**Self-hosted sync image** uses the alternative adapters already required by §3.7: SQLite `UpdateLog`, local-disk `SnapshotStore`, in-memory `Presence`, no Redis. This is the self-host offer and the proof that the cloud ports are real.

**Permissions** (viewer / commenter / editor) are enforced in the sync server on every update, not only in the UI.

**Share snapshots (Pro, v1).** Sharing is explicit, not automatic sync: **Create share link** uploads a snapshot of the document plus everything it needs (assets, fonts, imported libraries, script files) and returns a link to the web viewer. **Update snapshot** publishes later edits to the same link; **Delete share link** revokes it (copies already downloaded stay with whoever downloaded them). Before uploading, the editor checks dependencies and warns about missing files ("2 images not found — share anyway?"). Viewers can open it read-only, use Inspect, and **Download** a ZIP that opens in Vellum. Uploads run in the daemon, so they continue if the dialog is closed. Live multiplayer (v1.1) is a separate, opt-in mode on top.

---

## 10. Configuration and composition roots

Each program has exactly one place that chooses implementations:

```toml
# ~/.vellum/config.toml (daemon) — editor reads the equivalent from host settings
[engine]
doc_store = "loro"
text      = "parley"
layout    = "taffy"
codec     = "json"          # or "json-dir"

[render]
native = "skia"
editor = "canvaskit"        # "canvas2d" fallback, "vello" experimental

[ai]
engine   = "builtin"        # or "claude-code", "codex"
provider = "openrouter"
model    = "deepseek/deepseek-v4.1-flash"
effort   = "medium"

[agents]
parallel_default = 1
plan_threshold_frames = 2   # plan first when a request touches this many frames
spawn_agents_external = false

[permissions]
read_repo   = "ask"         # "ask" | "allow"
write_files = "ask"
run_commands = "ask"
skip_all    = false

[assets]
icons        = ["lucide", "material-symbols", "phosphor", "feather"]
images_stock = "none"       # a stock provider, key in SecretStore
images_generate = "none"

[telemetry]
enabled = false
```

- Unknown or unavailable adapter → fail fast at startup with a clear message, never silently fall back (except the documented renderer fallback to `canvas2d`).
- Cloud services read the same kind of config from Infisical-provided environment variables.
- **Feature flags** for experimental adapters live in this config, not scattered `if` statements.

---

## 11. Quality gates

| Gate | What it protects |
|---|---|
| Dependency rules (`cargo-deny`, `cargo tree` script, `dependency-cruiser`) | The hexagonal layout (§2.1) |
| Contract suites per port | Any adapter is a valid adapter |
| Golden render suite (all renderers × fixtures) | Visual output across swaps |
| Format round-trip (load → save byte-identical; every migration has a fixture) | Users' files |
| MCP tool schema snapshot test | No accidental breaking change to agents |
| Generated types check (`ts-rs`, `schemars`) | Rust and TS can't drift |
| Performance benchmarks (fail on >10% regression) | §12 budgets |
| Fuzzing (`cargo-fuzz`) on codecs, merge driver and op application | Malformed files and hostile agent input |
| `execute` sandbox suite: escapes (I/O, network, timers), resource limits, determinism (same script → same ops) | Agent scripts and script nodes can't harm the machine or the file |
| Tool parity suite: every tool in the §7.4 table has a scripted-agent e2e test | No capability silently disappears |
| Agent evals: fixed briefs × engines × models, scored on layout problems, token use and screenshot review | Quality of skills and prompts as they change |

---

## 12. Performance budget

| Target | Budget |
|---|---|
| Pan/zoom | 60 fps on a 5,000-node page (M1 / mid-range Windows laptop) |
| Drag a node | Input → frame under 16 ms |
| Open a 2 MB file | Under 500 ms to first paint |
| Agent batch of 200 nodes | Applied and painted under 100 ms |
| WASM bundle | Core under 2 MB gzipped; CanvasKit loaded in parallel and cached |

Port design respects these: hot-path ports are generic (static dispatch), the display list crosses the WASM boundary as shared memory, and benchmarks run against every adapter, not just the default.

---

## 13. Monorepo layout (Cargo workspace + pnpm/Turborepo)

```
vellum/
  Cargo.toml · pnpm-workspace.yaml · turbo.json

  crates/
    vellum-types/        data types, display list, protocols (serde, schemars, ts-rs) — no deps beyond these
    vellum-core/         ops, resolver, layout, scene, display list, diff/merge, export + PORTS (traits)
    contracts/           contract test suites for every Rust port + fakes
    adapters/
      doc-loro/  layout-taffy/  text-parley/  index-rstar/  pathops-kurbo/
      codec-json/  codec-json-dir/  assets-fs/  assets-s3/
      render-skia/  render-vello/ (later)
      llm-openai-compat/  llm-anthropic/  llm-google/  secrets-keyring/
      script-quickjs/  sessions-sqlite/  browser-cdp/ (v1.1)
      engine-builtin/  engine-claude-code/  engine-codex/
      import-svg/  import-image/  import-figma/  import-html/ (v1.1)
      icons/  images-stock/  image-ops/ (v1.1)
      export-html/  export-raster/
    vellum-wasm/         composition root: browser engine
    vellum/              composition root: daemon (tool registry, MCP + CLI adapters, live/headless targets)
    sync-server/         composition root: sync (ports + Postgres/Redis/R2 and SQLite/disk adapters)

  packages/
    types-ts/            generated from vellum-types (never hand-edited)
    renderer-api/        Renderer interface
    renderer-canvaskit/  renderer-canvas2d/
    host-api/            Host interface
    host-vscode/  host-electron/  host-browser/  host-fake/
    engine-client/       EngineClient over vellum-wasm (+ fake)
    editor-core/         controller: tools, commands, registries (no React)
    editor-react/        UI: shadcn / Base UI panels
    contracts/           TS contract suites (Renderer, Host, EngineClient)
    auth/  billing/  storage/  mail/  jobs/  telemetry/   cloud wrapper packages (§3.7)
    design-systems/      starter systems + guidelines.md
    prompt-packs/        versioned prompts
    skills/              built-in skills served by read_skill (format, execute API, workflows)
    styles/              built-in visual styles served by get_style
    ui/                  shared tokens, icons

  apps/
    extension/  desktop/  account/  api/  web/  docs/

  docs/
    adr/                 architecture decision records
    format/              .vellum spec + generated JSON Schema
  fixtures/              golden documents and reference images
```

### Work packages

| Package | Port(s) | Fake for others to build against | Done when |
|---|---|---|---|
| WP1 Types + ports | all `vellum-core::ports` | fakes in `contracts/` | Ports reviewed, ADRs 0001–0006 written |
| WP2 Core ops + resolver | `DocStore` (fake) | — | Op contract tests pass on fake store |
| WP3 Loro adapter | `DocStore` | — | Passes `DocStore` contract + round-trip |
| WP4 Layout + text | `LayoutEngine`, `TextEngine`, `FontSource` | fixed-metric fake text | Layout goldens pass |
| WP5 Renderers | `Renderer`, `render-skia` | `canvas2d` | Render goldens pass on all three |
| WP6 Editor core + UI | `Host`, `EngineClient`, `Renderer` | `host-fake`, `engine-fake` | Storybook + e2e on fakes, then real |
| WP7 Daemon + tools | `Tool`, `DocumentTarget`, `ModelProvider` | `llm-fake`, headless target | Agent e2e (scripted model) passes |
| WP8 Hosts | `Host` | — | Host contract suite passes in VS Code and Electron |
| WP9 Cloud | §3.7 ports | SQLite/in-memory adapters | Self-host image and cloud pass the same e2e |
| WP10 Script + `execute` | `ScriptRuntime`, `doc` API | — | Sandbox suite passes; same script gives the same ops in WASM and native |
| WP11 Agent runner | `AgentEngine`, `SessionStore`, `PermissionBroker`, `SkillSource` | `engine-fake`, `sessions-memory` | Plan → parallel build → undo-turn e2e passes on the fake engine, then on `builtin` and `claude-code` |
| WP12 Import, assets, export | `Importer`, `IconLibrary`, `ImageSource`, `CodeExporter`, `ImageExporter` | fixture files | Import/export goldens pass |

---

## 14. Build order

| Milestone | Scope | Done when |
|---|---|---|
| **M0: Spine** | WP1 + fakes, minimal core ops, Taffy + Parley adapters, CanvasKit + canvas2d drawing a display list in a web page, CI gates (§11) wired | Pan/zoom a 1,000-node document at 60 fps, and a dependency-rule violation fails CI |
| **M1: Headless + MCP** | Codec v0, Loro adapter, tool registry, MCP + CLI adapters, `execute` + typed tools, `read_skill`, `snapshot_layout`, `find_space`, `render-skia`, `screenshot` | Claude Code designs a login screen headlessly with `execute`; PNG matches the editor's golden |
| **M2: Editor in VS Code** | `editor-core`, `editor-react`, `host-vscode`, live target | Watch an agent build a screen live inside VS Code |
| **M3: Real design tool** | Auto-layout UI, text editing, pen tool, booleans, images, components, variables, undo | You'd design your own next screen in it |
| **M4: Agent panel + code bridge** | Agent runner (WP11): sessions, plans, Split / Side by side / Explore, permissions, usage; engines `builtin`, `claude-code`, `codex`; skills and styles; `TokenFormat`s, `get_spec`, `CodeExporter`; icons, stock images, script nodes, slots | Spec → plan → 4 parallel agents → undo one turn; design → shadcn code → tokens back with no manual fixes |
| **M5: Git + desktop** | Watcher diff-apply, merge driver, `vellum diff`, PR Action, `host-electron` | A design change reviewed and merged like code |
| **M6: Launch** | Docs, format spec v1, installers, accounts + Pro via cloud ports | Public beta |
| **M7: Multiplayer** | Sync server, presence, comments, Team plan, self-host image | Two people and two agents in one document, on our cloud and on a self-hosted box |
| **M8: Import and present (v1.1)** | `BrowserPort` + `browser` tool + `html` importer, Figma file import, image generation, background removal and tracing, slides | Import a running localhost page as editable layers; present a slide deck and export it as PDF |

The spine comes first on purpose: ports, fakes and CI gates in M0 are cheap, and retrofitting them after M3 is not.

---

## 15. Open questions to resolve early

1. **Text in the display list:** Parley glyph runs drawn via CanvasKit `drawGlyphs`, or CanvasKit Paragraph feeding sizes back? The `TextEngine` port makes this swappable, but `render-skia` must match the editor either way. Prototype both in M0.
2. **`DocStore` granularity:** is the trait at the right level (transactions over our types), or does it leak CRDT concepts? Test by writing the in-memory fake and a spike Automerge adapter against it in M0.
3. **Loro at scale:** load time and memory on 10k+ node files; when to default to the directory codec.
4. **`execute` vs typed tools:** which should the skill recommend by default, per model size? Measure token cost, error rate and design quality with 2–3 models in M1. Same question for the compact JSX-like insert syntax vs JSON.
5. **QuickJS in the editor:** script nodes must run in the WASM editor too. Check the size and speed of `rquickjs` compiled to WASM; fall back to running scripts in the daemon and streaming results if it's too heavy.
6. **Stock and generated images:** pick a stock provider with a licence that allows use in shipped products, and decide whether generated images need a visible "generated" marker in the file.
7. **Installed-engine contracts:** Claude Code and Codex headless modes and their permission systems change between releases; pin tested versions per Vellum release and detect unsupported ones at start-up.
8. **CanvasKit vs Vello:** revisit per ADR-0004.
9. **Interop:** Figma import via their REST API and SVG paste are fine; reading other tools' proprietary formats (including `.pen`) needs legal review first. Matching a competitor's *capabilities* is fine, and plain descriptive tool names (`execute`, `get_variables`) that happen to match help agents carry habits over. Copying its script API, prompts, skills text, docs or UI is not; ours are written from scratch.
10. **Daemon lifecycle:** one per user vs per workspace; how extension and desktop share it.
11. **License:** Apache-2.0 for core, CLI, format and ports (adoption) vs AGPL for the sync server (protects hosted business).
