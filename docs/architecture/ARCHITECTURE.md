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
        │  vellum-wasm: engine + Skia renderer, one WASM module   │
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
  - TypeScript: `dependency-cruiser` rules: `editor-core` can't import React or a renderer; apps can't import `better-auth`, `@polar-sh/*` or `@aws-sdk/*` directly, only our wrapper packages.

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
| Display list | `DISPLAY_LIST_VERSION` in a versioned binary ABI (ADR-015) | Internal; core and renderers ship together. Any change to opcodes, resource schemas or semantics bumps it; renderers refuse a version they do not know |
| Public REST API | `/v1/` path prefix | Deprecation header, 6 months notice |

### 2.6 Decisions are written down

- `docs/architecture/decisions/ADR-NNN-title.md` for every Tier A choice and every tier change: context, decision, alternatives, **"revisit when"** (the condition that should trigger reconsidering it). This document is kept in step with accepted ADRs.
- Example: *ADR-001 Renderer: skia-safe in WASM everywhere.* Revisit `render-vello` when Vello's CPU and GPU renderers agree within our golden tolerance, our hosts enable WebGPU by default (or its WebGL2 path is stable), and it beats the Skia module's 10,000-node frame p95 on the same machine.

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

Signatures as implemented (Prompts 10–28). Every port has a fake and a contract suite in `crates/contracts`, and
each suite is checked against deliberately broken implementations.

```rust
/// Document storage and history. Today: Loro (ADR-004). Alternatives: Automerge, Yrs, plain in-memory.
pub trait DocStore {
    fn adapter_id(&self) -> &'static str;
    fn snapshot(&self) -> Document;                         // our types, not the CRDT's
    fn apply(&mut self, commit: Commit) -> ChangeSet;       // a Commit only `ops` can build: the one write path
    fn undo(&mut self, actor: &Actor) -> Option<ChangeSet>;
    fn redo(&mut self, actor: &Actor) -> Option<ChangeSet>;
    fn undo_turn(&mut self, turn: &TurnId, by: &Actor) -> UndoTurn; // Nothing | Undone(ChangeSet) | Revert(Revert)
    fn cursor(&self) -> SyncCursor;
    fn export_updates(&self, since: &SyncCursor) -> SyncPayload;    // opaque bytes + adapter id
    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError>; // DifferentDocument refused
}

/// Auto-layout. Today: Taffy (ADR-012). Ids, dirty entries and boxes are occurrence paths (`NodeIdPath`), so a
/// definition node used by two instances keeps two boxes.
pub trait LayoutEngine {
    fn layout(&mut self, tree: &LayoutTree, dirty: &DirtySet, text: &dyn TextEngine) -> LayoutResult;
}

/// Text shaping and paragraph layout. Today: Parley (ADR-013). Renderers draw its glyphs, never shape again.
pub trait TextEngine {
    fn measure(&self, text: &RichText, constraints: Extent) -> TextMetrics;
    fn layout(&self, text: &RichText, width: f64) -> TextLayout;         // glyph runs, our types
    fn layout_box(&self, text: &RichText, extent: Extent) -> TextBoxLayout; // applies auto-width / auto-height / fixed
}

/// Fonts as bytes the host supplies, tiered: document fonts, Google Fonts cache, system, bundled Inter (ADR-013).
pub trait FontSource {
    fn resolve(&self, family: &str, weight: u16, style: FontStyle) -> Option<FontHandle>;
    fn data(&self, handle: FontHandle) -> Option<FontData>;  // shared immutable SFNT bytes + collection index
    fn families(&self) -> Vec<String>;
}

/// Hit-testing, culling, snapping. Today: rstar (ADR-014). Keyed by occurrence path.
pub trait SpatialIndex {
    fn rebuild(&mut self, items: &[(NodeIdPath, Rect)]);
    fn update(&mut self, changed: &[(NodeIdPath, Rect)], removed: &[NodeIdPath]);
    fn query_rect(&self, r: Rect) -> Vec<NodeIdPath>;
    fn query_point(&self, x: f64, y: f64) -> Vec<NodeIdPath>;
}

/// Path booleans, stroke outlines and bounds. Today: kurbo + i_overlay.
pub trait PathOps {
    fn boolean(&self, a: &Path, b: &Path, op: BoolOp) -> Path;
    fn outline_stroke(&self, path: &Path, stroke: &StrokeStyle) -> Result<Path, String>;
    fn bounds(&self, path: &Path, stroke: Option<&StrokeStyle>) -> Option<Rect>; // None: draws nothing
}

/// File format, bytes in and out. Today: JSON. Later maybe binary.
pub trait Codec {
    fn id(&self) -> &'static str;
    fn decode(&self, bytes: &[u8]) -> Result<Document, CodecError>; // runs migrations; a newer format is refused
    fn encode(&self, doc: &Document) -> Result<Vec<u8>, CodecError>; // deterministic
}

/// A document at a path: one file (`SingleFile` over a `Codec`) or the directory format (ADR-018).
pub trait DocumentFiles {
    fn id(&self) -> &'static str;
    fn read(&self, fs: &dyn FileSystem, path: &str) -> Result<Loaded, FileError>;   // orphans, interrupted
    fn write(&self, fs: &dyn FileSystem, path: &str, doc: &Document) -> Result<Saved, FileError>;
}

/// Whole files under a root the implementation owns; `/`-separated relative paths, no `..`. Today: `StdFs`.
/// `files::atomic_write` (temp file, sync, rename, sync folder) is built on it.
pub trait FileSystem {
    fn read(&self, path: &str) -> Result<Vec<u8>, FsError>;
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), FsError>;
    fn sync_file(&self, path: &str) -> Result<(), FsError>;
    fn rename(&self, from: &str, to: &str) -> Result<(), FsError>;
    fn remove_file(&self, path: &str) -> Result<(), FsError>;
    fn list(&self, dir: &str) -> Result<Vec<String>, FsError>;
    fn create_dir_all(&self, dir: &str) -> Result<(), FsError>;
    fn sync_dir(&self, dir: &str) -> Result<(), FsError>;
    fn exists(&self, path: &str) -> Result<bool, FsError>;
}

/// Assets (images, fonts) by the SHA-256 of their bytes. Today: local folder, S3-compatible.
pub trait AssetStore {
    fn get(&self, hash: &AssetHash) -> Result<Vec<u8>, AssetError>;   // re-hashes; a mismatch is damage
    fn put(&self, bytes: &[u8], mime: &str) -> Result<AssetHash, AssetError>;
}
pub trait AssetCollect: AssetStore {                                  // stores that can list and remove
    fn list(&self) -> Result<Vec<AssetHash>, AssetError>;
    fn remove(&self, hash: &AssetHash) -> Result<(), AssetError>;
}

/// Which document names changed on disk; never their content. Today: notify (ADR-020).
pub trait FileWatcher {
    fn id(&self) -> &'static str;
    fn watch(&mut self, path: &str) -> Result<(), FsError>;
    fn unwatch(&mut self, path: &str);
    fn changed(&mut self) -> Vec<String>;                             // non-blocking
}

pub trait Clock { fn now(&self) -> Timestamp; }                       // UTC milliseconds
pub trait IdGen {                                                     // page, component, variable and style ids too
    fn node_id(&mut self) -> NodeId;
}

/// Sandboxed JavaScript. Used by the `execute` agent tool (§7.3) AND by script nodes on the canvas (§4.1).
/// Today: QuickJS via `rquickjs` (native and WASM; ADR-005). Alternatives: Boa, a WASM-isolated V8.
pub trait ScriptRuntime {
    fn run(&self, src: &str, api: Rc<dyn ScriptApi>, limits: ScriptLimits) -> Result<ScriptOutput, ScriptError>;
    fn run_diagnostic(&self, src: &str, api: Rc<dyn ScriptApi>, limits: ScriptLimits)
        -> Result<ScriptOutput, ScriptDiagnostic>;                  // typed cause, line/column, excerpt
}
pub trait ScriptApi {
    fn constants(&self) -> BTreeMap<String, ScriptValue>;            // frozen data on `doc`
    fn functions(&self) -> &[&str];                                   // the allowlist
    fn node_cost(&self, function: &str, args: &[ScriptValue]) -> Result<u32, ScriptError>; // reserved before `call`
    fn call(&self, function: &str, args: &[ScriptValue]) -> Result<ScriptValue, ScriptError>;
}
pub struct ScriptLimits { pub time_ms: u32, pub max_ops: u32, pub max_nodes: u32, pub memory_mb: u32 }
// No network, file system, timers, Date, Promise or async. Math.random is a seeded LCG per run and Math uses
// pinned soft-float libm, so output is the same natively and in WASM; `**` refuses (use Math.pow).

/// Runtime shaders for shader fills and effects. Our shader dialect is a small, documented subset;
/// the compiler turns it into what the active renderer runs. Today: SkSL for skia-safe (native and WASM).
/// Alternatives: WGSL for Vello. Importing GLSL ES fragment shaders goes through the same port.
pub trait ShaderCompiler {
    fn compile(&self, src: &str, target: ShaderTarget) -> Result<CompiledShader, ShaderError>;
}
```

The `ScriptApi` handed to scripts is a thin binding over **Ops** (§5): every mutating call plans an ordinary op against a staged copy of the document, so a script can never do anything the op layer doesn't allow, and a successful run is committed as one transaction (ADR-005, ADR-019). Each run gets a fresh runtime; arguments and results are copied as bounded JSON; source is a strict function body with an explicit `return`.

Not every swap point is a trait on this list. `LibrarySource` (resolver) hands the core decoded library files by normalized path, and `Services` / `RenderRead` (document API) are the bounded, in-memory scene and render reads `execute` and screenshots use; hosts inject both.

The `Engine` struct in core is constructed with these as generics/trait objects. Hot paths (layout, spatial index) use generics for zero-cost dispatch; cold paths (codec, assets) use `dyn`.

### 3.2 Rendering port

The core never draws. It produces a **display list** (our types in `vellum-types::display`, versioned), and a renderer consumes it (ADR-015).

```rust
pub struct DisplayList { pub commands: Vec<DrawCmd>, pub resources: Vec<Resource> }
pub enum DrawCmd {
    Save, Restore,
    Transform([f32; 6]),                     // SETS the absolute local-to-device matrix (f64 viewport rebase, then f32)
    Clip { shape: u32 },                     // stays in device space once established
    Path { shape: u32, paint: u32, stroke: Option<u32> },
    GlyphRun { glyphs: u32, paint: u32, stroke: Option<u32> },
    Image { image: u32, dst: [f32; 4] },
    Effect { effect: u32 },                  // configures the enclosing Layer
    Layer { opacity: f32, blend: BlendMode },// one isolated group; Restore composites it
}
pub enum Resource { Shape(Shape), Paint(RenderPaint), Stroke(RenderStroke), Glyphs(RenderGlyphRun),
                    Image(RenderImage), Effect(RenderEffect) }   // first-use ordered, referenced by index
```

Shapes are typed descriptors (rect with radii, ellipse arcs and rings, polygon, star, line, SVG path data with a
fill rule, boolean operands), not vendor paths; renderers lower them. Glyph runs carry exact font, glyph and cluster
identities, variation coordinates, synthesis, decorations and links; font bytes travel beside the buffer. The list
crosses the WASM boundary as one byte buffer with fixed opcodes (see `docs/runbooks/display-list.md`); the decoder
refuses unknown versions, malformed lengths, bad references, unbalanced state, non-finite scalars and trailing
bytes, and caps input at 64 MiB.

```ts
// TypeScript side, packages/renderer-api
export interface Renderer {
  readonly state: "ready" | "context-lost" | "disposed";
  resize(viewport: Viewport): void;          // width, height, dpr
  draw(frame: Frame): void;                  // displayList bytes + font and image resources; a failed frame keeps the last good one
  readPixels(): Uint8Array;                  // unpremultiplied RGBA
  dispose(): void;
}
```

| Implementation | Where | Status |
|---|---|---|
| `render-skia` (Rust, `skia-safe` 0.153, source-built) | Daemon, CLI, workers natively; editor through `renderer-skia` (the same crate compiled to `wasm32-unknown-emscripten` in one module with `vellum-core`, drawing into WebGL2) | v1 default everywhere (ADR-001, ADR-016) |
| `renderer-canvas2d` (TS over the same WASM module's geometry and glyph rasterization) | Editor fallback (S02), tests | v1 basic: no effects, mesh averaged, shader paints transparent, linear burn as normal; it reports these simplifications. Exports always use the full renderer (ADR-017) |
| `render-vello` (Rust → WASM, WebGPU) | Editor + native | Later, behind a flag, on ADR-001's revisit conditions |

CanvasKit is not used: one module means a frame never crosses the JS boundary per draw call. Skia never lays out
text (no `textlayout`); it draws Parley's glyphs. Every renderer uses Skia's FreeType manager with fonts passed in as
bytes (`FontMgr::custom_empty`); CoreText, DirectWrite and fontconfig are compiled out, so the same document draws the
same everywhere. Skia is built from source with CI caching, with low-precision raster pipelines disabled so native
and WASM CPU rasters agree (ADR-016). Gradients, meshes, all 18 blend modes, shadows and blurs are drawn; filtered
scenes are rasterized on the CPU and composited, so GPU and CPU agree (ADR-017). Prepared scenes and raster tiles
are cached with fixed limits (8 frames, 32 MiB of input; 512 px tiles, 16 MiB per scene).

**Goldens compare like with like:** CPU raster against CPU raster (native and WASM agree within 1), GPU against GPU,
and a cross-backend comparison (an agent's headless screenshot against the editor's canvas) uses an edge tolerance:
per channel 2, at most 2% of pixels differing, p95 delta at most 1. Text tolerates per channel 3 and 0.01% of
pixels. The golden suite renders every fixture through every renderer, including the basic projection against
Canvas2D, and compares premultiplied colour plus alpha.

### 3.3 Editor ports (TypeScript)

| Port | Purpose | Implementations |
|---|---|---|
| `Host` | File I/O, watching, daemon discovery, opening links, clipboard, theme | `host-vscode`, `host-electron`, `host-browser`, `host-fake` |
| `Renderer` | §3.2 | `renderer-skia` (the WASM module), `renderer-canvas2d` |
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

**The tool registry is the product's real API.** MCP is one adapter over it. It lives in `crates/vellum/src/tools/` (ADR-021).

```rust
pub trait Tool {
    fn name(&self) -> &'static str;              // "insert_nodes"; stable, a breaking change is a new name
    fn version(&self) -> u32;
    fn description(&self) -> &'static str;       // shown to the agent
    fn input_schema(&self) -> schemars::Schema;  // an object that refuses unknown fields
    fn read_only(&self) -> bool;
    fn destructive(&self) -> bool;               // defaults to !read_only
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError>;
}
```

`run` takes and returns JSON; a picture goes under an `image` key (`{ mime, base64, width, height }`), which the
MCP front end sends as an image block. `ToolError` is `{ kind, message, path, op, line }`: kinds `invalid_input`,
`not_found`, `document`, `refused`, `render`, `internal`; `op` is the batch item at fault, `line` the script or
spec line. A failed tool is a result the agent can read and fix, not a protocol error.

| Front end (adapter) | Uses the registry via |
|---|---|
| MCP server | Hand-written dispatcher (`mcp.rs`: `initialize`, `ping`, `tools/list`, `tools/call`, `server/discover`) over the daemon's stdio bridge and opt-in HTTP; `rmcp` if MCP features beyond tools are needed (ADR-021) |
| CLI | `clap` subcommands generated from tool schemas |
| Built-in agent (`AgentEngine`) | Agent loop calling tools through a `ToolBridge` over the registry |
| Future protocols | New adapter only; tools unchanged |

`ToolCtx` routes to a **`DocumentTarget`** port (in `vellum-core`, ADR-020) with two implementations: `LiveTarget` (forward ops to the editor that has the file open, through an `EditorLink`) and `HeadlessTarget` (native engine + file, saved after every transaction). Tools don't know which they're talking to.

```rust
pub trait DocumentTarget {
    fn mode(&self) -> TargetMode;                                   // live or headless
    fn snapshot(&mut self) -> Result<Document, TargetError>;        // unsaved changes included
    fn transact(&mut self, tx: Transaction) -> Result<Applied, TargetError>; // through the Guard
    fn render(&mut self, target: &str, scale: f64) -> Result<ScriptValue, TargetError>;
    fn lease(&mut self, frame: &NodeId, agent: &AgentId, change: LeaseChange) -> Result<(), TargetError>;
    fn save(&mut self) -> Result<(), TargetError>;
    fn is_dirty(&mut self) -> Result<bool, TargetError>;
}
// TargetError: Write (the Guard's rejection), ReadOnly, Gone, File, Render, Lease
```
 Leases are a target capability (`DocumentTarget::lease(frame, agent, Claim | Release)`), so the editor wire protocol carries claims too. `execute` runs its script on a scratch copy of the document and sends the resulting diff to the target as one transaction; the target's Guard still decides (ADR-021).

```rust
pub trait ModelProvider: Send + Sync {    // built-in agent and `vellum prompt`
    fn id(&self) -> &str;
    fn capabilities(&self) -> ModelCaps;  // tools, vision, reasoning, context size (None = unknown, never unlimited)
    fn status(&self) -> ProviderStatus;
    fn complete(&self, req: ChatRequest, cancel: Cancellation) -> ModelFuture<'_, Result<ChatStream, ModelError>>;
}

/// HTTP, injected by the composition root so protocol adapters never import another adapter.
pub trait ModelTransport: Send + Sync {
    fn start(&self, req: ModelHttpRequest, cancel: Cancellation) -> ModelFuture<'_, Result<WireStream, ModelError>>;
}

pub trait SecretStore: Send + Sync {      // API keys
    fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError>;  // missing vs. locked/unavailable
    fn set(&self, key: &str, value: SecretString) -> Result<(), SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;                 // Clear key
}
```

Futures are plain `std` futures; core takes no runtime. Conversations carry native tool calls and results,
inline images and opaque provider continuation blocks, which hosts must send back unchanged. Errors are
payload-free enums. `SecretString` never serializes and formats as redacted; credentials echoed by a server are
redacted from the stream. There are no automatic retries: the engine decides whether a replay is safe (ADR-023).

| Port | Implementations |
|---|---|
| `ModelProvider` | `openai-compat` (Chat Completions: **OpenRouter**, default `deepseek/deepseek-v4.1-flash`; DeepSeek, xAI, Moonshot/Kimi, Together, Fireworks, Z.AI, OpenCode, Ollama, LM Studio, any custom endpoint), `openai-responses` (OpenAI and compatible Responses endpoints), `anthropic` (Anthropic API, AWS Bedrock via its native Messages SSE route, Google Vertex, Microsoft Foundry, custom Messages endpoints; hosts supply and refresh cloud credentials), `google` (Gemini via AI Studio key), `fake` (scripted responses for tests). Reasoning effort is a request field, passed through where the model supports it. Each provider reports a status for the UI: Connected, API key set, Signed in, Configured externally (not tested by us), Failed. |
| `ModelTransport` | `model-http` (HTTPS except loopback, no redirects, bounded SSE, cancellation drops the request) |
| `SecretStore` | `keyring` (OS keychain), `env` (read-only `VELLUM_<PROVIDER>_API_KEY`, for CI; used only when the keychain has no entry, never when it fails), `memory` |
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

/// Conversations live on the user's computer, never in the design file (ADR-006).
pub trait SessionStore {
    fn create(&self, session: NewSession) -> Result<(), StoreError>;
    fn sessions(&self, doc: &DocKey) -> Result<Vec<SessionMeta>, StoreError>;   // newest activity first
    fn session(&self, id: &SessionId) -> Result<Option<SessionMeta>, StoreError>;
    fn rename(&self, id: &SessionId, title: &str) -> Result<(), StoreError>;    // the only metadata write
    fn append(&self, session: &SessionId, event: NewEvent) -> Result<EventSeq, StoreError>; // places from 1, no gaps
    fn events(&self, session: &SessionId, since: Option<EventSeq>) -> Result<Vec<SessionEvent>, StoreError>;
    fn put_image(&self, bytes: &[u8]) -> Result<ImageRef, StoreError>;          // content-addressed cache
    fn image(&self, image: &ImageRef) -> Result<Option<Vec<u8>>, StoreError>;
    fn delete(&self, id: &SessionId) -> Result<(), StoreError>;                 // events + pictures only it named
}

/// Requests from an agent to do something outside the canvas (read or write a repo file, run a command).
pub trait PermissionBroker {
    async fn request(&self, req: PermissionRequest) -> PermissionDecision; // allow once, allow for session, deny
}

/// Skills: instructions agents read before working. Built-in, per design system, and user-added SKILL.md folders.
pub trait SkillSource {
    fn list(&self) -> Result<Vec<SkillMeta>, ContentError>;   // a missing or malformed configured folder is an error
    fn read(&self, path: &str) -> Result<String, ContentError>;
}

/// Visual styles agents can apply when no design system is set.
pub trait StyleLibrary {
    fn list(&self) -> Result<Vec<StyleMeta>, ContentError>;
    fn read(&self, name: &str) -> Result<VisualStyle, ContentError>;
}
```

| Port | Implementations |
|---|---|
| `AgentEngine` | `builtin` (our loop over any `ModelProvider`: one self-check after writes; long tool results cut with a marker; old results pruned to markers, then older rounds summarized, near the context limit; Stop races every provider wait), `claude-code` (the user's installed Claude Code, run headlessly with our MCP server attached; uses whatever sign-in it already has: Claude subscription, API key, Bedrock, Vertex), `codex` (same for Codex CLI, including ChatGPT sign-in), `fake`. Later adapters: `gemini-cli`, `copilot` (GitHub sign-in), `cursor-agent`, `opencode`. A new engine is one adapter. |
| `SessionStore` | `sqlite` at `~/.vellum/sessions.db` (keyed by the file's repo-relative path + repo root; rusqlite with bundled SQLite; events append-only, enforced by a trigger; pictures in `~/.vellum/cache/` by SHA-256), `memory` |
| `PermissionBroker` | `editor` (shows a Permission card in the Agent panel), `cli` (prompts in the terminal, or `--allow`/`--deny` flags), `policy` (pre-approved rules from settings, e.g. "read anything in this repo") |
| `SkillSource` | `builtin` (format schema, `execute` API, workflows), `design-system` (each system's `guidelines.md`), `folder` (user-added `SKILL.md` folders), stacked in that order; a later source owns a duplicate path. Folder sources read only explicitly configured files through an authorized rooted `FileSystem` (no recursive scan, no home-directory discovery) (ADR-022) |
| `StyleLibrary` | Visual styles (palette, type, radius, density and a short description) that agents can apply when no design system is set. `builtin` (six recipes) + `folder` (registered JSON files). Recipes are starting points for canvas work; existing variables, guidelines and the person's decisions win, and fonts are preferences, not claims they are installed. |

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
| `Importer` | Turn foreign content into our `Document` subtree | `svg`, `image` (PNG/JPEG/WEBP as image nodes), `figma-clipboard` (paste), `figma-file` (via Figma's REST API with the user's token; reading a locally exported `.fig` file only after legal review), `html` (DOM + computed styles → frames, from `BrowserPort`) | `svg`, `image`, `figma-clipboard` v1; the rest v1.1 |
| `BrowserPort` | Load a URL (including `localhost` dev servers), screenshot the page or an element, return a DOM + computed-style snapshot for the `html` importer | `chromium-cdp` (desktop: Electron's own Chromium; daemon: a managed headless Chromium via CDP), `fake` | v1.1 |
| `IconLibrary` | Searchable icon sets inserted as vector nodes | `lucide`, `material-symbols` (outlined, rounded, sharp), `phosphor`, `feather`, user folder of SVGs | v1 |
| `ImageSource` | Fill image slots with real pictures | `stock` (a licensed stock API, key in `SecretStore`), `generate` (any image model through `ModelProvider`-style adapter), `local` | `stock` v1, `generate` v1.1 |
| `ImageOps` | Remove background, trace image to vectors | `local` (ONNX segmentation model, `vtracer` for tracing), `fake` | v1.1 |

All importers produce `Document` content and go through Ops like everything else, so imported content is undoable, attributed and diffable.

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
 │                   values: [{value | $alias: <variable id>, when: {axis: value}}]}
 │                   the last entry whose `when` matches wins, by position, not specificity
 ├ styles      text styles, effect styles
 ├ fonts       custom fonts carried with the document (files in <name>.assets/fonts/)
 ├ components  id → node tree + props (typed, each with optional bindings [{node: path, property}])
 │                   + variants (each an override set on the one tree, chosen by its axis values)
 └ pages[]     id, name, kind: canvas | slides, children: Node[]

Node (tagged union)
  frame      layout: none | row | column | grid; grid columns, rows? (equal tracks, auto placement); gap;
             padding (n, [vertical, horizontal] or [top, right, bottom, left]); justify: start | center | end
             | space-between | space-around (rows and columns only); align; cornerRadius;
             layoutIncludesStroke (reserves this frame's own inward stroke beside padding: inside full width,
             center half, outside none); clip; theme? {axis: value}; slot? {suggested: componentId[]}
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
  icon       library, icon (the icon's name; `name` is the layer name), weight (100–700), fill
  instance   componentId, library? (an import alias; absent = local), variant, props (literal values),
             overrides {nodeIdPath → exactly one of props (a checked NodePatch) | replace (a node) | children},
             slotContent {slotId → Node[]}
  script     src (.js file relative to the document), inputs {name → value | $var (any type; a colour arrives
             as the text #RRGGBB)}, clip
             children are derived output: rendered, never saved, not in undo history
  note       sticky note for people (content, colour); agents can read it
  context    canvas-placed instructions for agents (content); read with the frame it sits in or points at
  prompt     a saved request on the canvas (content, engine/model?); Run sends it to the agent panel
  embed      live web view (url, device, zoom, scroll, radius, stroke, effects) — v1.1, uses BrowserPort
  conflict   left by a three-way merge in the conflicted node's place and id: ours?, theirs? (each side's
             version without children; absent when that side removed it), properties (what both changed),
             children (merged, live). Choosing a version is a Replace with that version

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

Every property that takes a variable can be a literal or a variable reference by id: {"$var": "<variable id>"}.
The variable's `name` (a path such as color/primary) is for people; tools accept names and resolve them to ids
at their own boundary. Text runs have no colour of their own: a text node's fills colour it.
```

- **References are ids** (ADR-010). Variable, component, style, asset and node references hold ids, never names, so a rename on one branch and a use on another still merge cleanly.
- **Themes and modes.** Variables can vary along several **axes** at once (Mode: Light/Dark, Device: Phone/Tablet, Brand: A/B). A frame can set a theme value for its subtree; children inherit it unless they set the same axis themselves. The first value of each axis is the document default. A frame's choice naming an axis or value the document lacks is ignored and reported as a problem on that node.
- **Aliases.** A variable's value can reference another variable of the same type, followed under the same theme. A cycle along the entries a theme selects is an error; the op layer refuses any alias loop when it is set.
- **Binding types** are checked when a binding is made, once per transaction: opacity takes a number variable, `visible` a boolean, a colour a colour, an icon's name a string. A variable that changes type while something uses it is refused.
- **Props** drive the nodes and properties their `bindings` name (ADR-011). A text prop bound to `content` replaces the characters with one run in the first run's style. Two props cannot drive the same property, and identity and children are not prop targets.
- **Overrides.** Instances can override any property, replace a descendant node entirely, or replace its children, addressed by an ID path (`card/actions/button`), including paths through nested instances. Expansion order: definition, props, selected variant, instance overrides, slot content, then the instance's own common fields. Inherited layers can't be reparented inside an instance; that's what slots are for.
- **Slots** are empty frames inside a component that instances fill with their own content. `suggested` lists components that usually go there; suggestions guide humans and agents but never restrict.
- **Sizing** (ADR-002, ADR-012) matches the browser, checked against Chrome on 186 fixtures. Children sit at the start unless they fill; fixed and hug never shrink; fill children share the space equally and ignore their content. Fill on the main axis of a hugging parent, fill with nothing to fill (a top-level frame, a child of a `layout: none` frame, an absolute child) and hug on a `layout: none` frame use their fallback (else hug, or refused). Top-level and absolute frames hug at max-content; `x`, `y` are measured from the parent's outer edge. Grid `justify`/`align` place items in their cells. Text: auto-width hugs both axes, auto-height hugs its height at the given width, fixed keeps both and reports overflow (ADR-013).
- **Containment.** Frames, groups and canvas pages hold any node; a boolean holds shapes, text, groups and booleans; a slides page holds frames, notes, context and prompts at its top level. A node may move between pages but not between a page and a component. Wrap, group and ungroup keep positions (and refuse a rotated or flipped wrapper); in an update, `null` puts a property back to its default.
- **Field spellings** are those of the generated reference in `docs/format/`; the sketch above names concepts.
- **Script nodes** point at a `.js` file beside the document. The file's leading comments declare a schema (1) and up to 128 inputs (`@input columns: number(min=1) = 3`; types number, string, enum, color, boolean, ref), which become controls in the properties panel; inputs can bind to variables. The `ScriptRuntime` port runs it with hard limits (1,000 nodes, 2 s, 64 MiB, no I/O, seeded randomness); the program sees only frozen `doc.inputs` and `doc.apiVersion` and returns an array of nodes, checked through ops on an isolated page and given ephemeral ids. The output is kept outside the document and the store and rendered inside the script node's bounds; a failed run removes the old output. **Convert to layers** replaces it with a frame holding a snapshot of the output, in one attributed, undoable transaction (ADR-019). Scripts use the same node schema as the file, so agents can write them.
- **Shaders** are runtime shader files beside the document, used as a fill or an effect. Uniforms can bind to variables; the renderer supplies the node's size and, for effects, its rendered content. A `ShaderCompiler` port (§3.1) turns our shader dialect into whatever the active renderer runs.
- **Agent context on the canvas.** `agentNotes` (on any node), `context` nodes (a block of instructions placed beside frames) and `note` nodes (sticky notes for people) are all returned by `get_app_state`, `get_nodes` and `execute` reads; skills tell agents to respect notes and context. `prompt` nodes keep a reusable request on the canvas next to what it's about.
- **Slides** are frames on a `slides` page, ordered for presenting and for multi-page PDF export.
- **Metadata** is free-form JSON per node for tooling and agents (e.g. `{"source": "src/components/Card.tsx"}`); the editor ignores it.

These types are plain Rust structs with `serde`, `schemars` and `ts-rs`. They are the shared language of every port; no port signature mentions a vendor type.

### 4.2 `DocStore` today: Loro

The Loro adapter (`adapters/doc-loro`, loro 1.16) maps our tree onto Loro's **movable tree** with fractional indexes (reparenting merges correctly): the pages in order and one root per component, each document node one tree node whose meta map holds each property as its own entry, so concurrent edits to different properties merge (ADR-004). Binary sync updates come from Loro, exposed only through the `DocStore` trait; nothing depends on Loro's iteration order or snapshot bytes, which differ between native and WASM. Two stores opened from the same file share a genesis change derived from its content, so they sync without duplicating it.

**Undo is not Loro's.** Loro's undo is per peer and grows with the document; Vellum's is per actor and per turn. `vellum_core::history`, shared by every store, records each commit's inverse and footprint and replays it adjusted to the document as it is: someone else's later change to a property, a node someone built on, a later move are kept. ⌘Z takes the actor's latest change back as far as it can. "Undo this turn" is undone outright when nothing later depends on it; otherwise the store changes nothing and returns a `Revert` that `ops::revert` applies as a new, undoable transaction. Writing a property's current value is not a change. A 200-node batch on a 10,000-node document undoes in about 15 ms.

### 4.3 On-disk format: `.vellum`

- **Deterministic serialization:** keys in a fixed order, 2-space indent, numbers normalized.
- **Stable IDs** so a moved node diffs as a move.
- **Quantized in the codec only:** positions, sizes, radii, gaps and paddings to 1/100 px; ratios, opacities and gradient stops to 1/1000; angles to 1/100 degree. A file in a newer format is refused with the version it needs.
- **Assets** beside the file in `<name>.assets/<sha-256>`, one file per hash with no extension, via `AssetStore` (ADR-018): images and custom fonts (TTF, OTF, WOFF, WOFF2, stored as imported and unpacked on load; they travel with the document and are served before bundled fonts). Unreferenced assets are removed only on an explicit save, keeping any the undo history can still bring back. Script `.js` files, shader files and library files are referenced by relative path (a `src` into `<name>.assets/` is an asset), so a design folder can be moved or zipped as a unit.
- **Writes are atomic:** a temporary file beside the target, synced, renamed over it, and the folder synced.
- **Directory format** (`app.vellum/`: `index.json` with page stubs, `pages/<page-id>.json`, a `.saving` marker during multi-file saves) is a second `DocumentFiles`, not a `Codec`: bytes in and out cannot say which page a byte belongs to. Every file is a normal document written by the single-file codec. A save cut short leaves whole files but possibly pages from different moments, reported as `interrupted`. Suggested (never required) at 10,000 nodes or 4 MB of JSON; its benefit is per-page Git diffs, not faster saves.
- **Spec and schema** published in `docs/format/` (including `files.md`), generated from `vellum-types`.

### 4.4 Libraries

- **A library is a document with `kind: library`**, saved as `*.lib.vellum`. Turning a document into a library is a one-way change in the editor and its helper (the editor asks first); the op underneath (`SetShell`) can still set it back, because a checkout of an older file arrives as ops.
- **Importing** adds an alias to `imports` with a relative `.lib.vellum` path. An instance names a library component with `library: <alias>` beside its `componentId`. A library's definitions resolve their variables, themes and nested components in the library; the instance's own overrides and slot content resolve in the importing document. Instances keep a link; when the library changes, importing documents pick up the change on next open (and live, when both are open in the same editor). The core reads libraries through an injected `LibrarySource`.
- **Overrides win** over library changes, as with local components.
- **Pasting between documents:** content from a library into a document that hasn't imported it asks **Link to library** (adds the import) or **Paste as copies** (independent). Pasting from an ordinary document copies without a link. Variables (and their theme values), styles, fonts and component dependencies travel with pasted content; on a name clash the person chooses **Use existing** (same type) or **Add renamed**. Paste produces ordinary ops; asset, font and script bytes are carried by the host, which reports any that are missing (ADR-011, ADR-018).
- **Missing library:** if the path no longer resolves, the editor shows the library as missing and offers **Locate…**; instances keep rendering from the last known copy cached in `~/.vellum/cache/` (flagged as missing; the copy holds the document only, so its images are missing too).
- **Starter design systems** ship as libraries.

### 4.5 External changes and Git

- **External change** (`git checkout`, merge, an agent editing the file directly): the daemon's watcher fires and the daemon re-reads the file; a content hash equal to the last known file is a self-write and is ignored. Otherwise core runs the three-way merge (last file, the engine's document with any unsaved edits, the new file), diffs the engine's document against the result by node ID, and applies that as one transaction by `Actor::File`. With no unsaved edits this equals diffing old against new. A file with text-merge markers, a half-written file, an interrupted folder save or a deleted file is reported and nothing is applied (ADR-020). Open editors update in place.
- **Merge driver:** `vellum merge-driver` does a three-way merge by node ID and property (ADR-010). A change beats a removal. The same property changed both ways gives a `conflict` node holding both versions; where no node can hold both (both moved one node, both reordered the same children, a variable, style, page or the name changed both ways) ours is kept and the conflict is reported. It works on `Document`, so it doesn't care which `DocStore` or `Codec` is in use.

---

## 5. `vellum-core` modules

| Module | Responsibility | Ports it uses |
|---|---|---|
| **Ops** | The only way to change a document: `insert`, `update`, `delete`, `move`, `copy`, `replace`, `set_variable`, `create_component`, `detach`, `toggle_component`, `replace_in_instance`, `make_slot`, `convert_script`, `wrap_in_frame`, `group` / `ungroup`, `boolean`, `align` / `distribute`, `reorder`, `flip`, `scale` (resizes content, text, strokes, radii, spacing and effects together; variable-bound sizes become fixed values and instances get overrides), `outline_stroke`, and for whole documents `insert_page`, `delete_page`, `update_page`, `move_page`, `set_shell` (meta, imports, themes, styles, fonts), `set_component`, `update_component`. Validated, applied as one transaction, one undo step; references and binding types are checked once, against where the transaction ends (a refusal still names the op that first broke it). `DocStore::apply` takes a `Commit` that only `ops` can build. Every transaction carries an **origin** `{actor, session?, turn?}`, which is what makes per-person undo, "Undo this turn" and the History ledger possible. | `DocStore`, `IdGen`, `PathOps` |
| **Guard** | Checked before every agent transaction: is this actor paused, is the target inside a frame leased by someone else, is the batch within limits (default 1,000 ops and 1,000 new layers per transaction). Rejections are typed, returned to the agent with a reason and recorded in History. Human edits skip lease checks (your edits win). | — |
| **History** | Per-actor undo/redo and "Undo this turn" for every store: inverses with footprints, replayed around later work by others (§4.2). | — |
| **Layout check** | Finds problems agents should fix, as typed problems with world-space boxes addressed by occurrence path: text overflowing its box (from the same text layout the display list uses), children clipped by any clipping ancestor (rounded and transformed), overlapping flow siblings, nodes outside every frame, broken hug cycles. Feeds `snapshot_layout(problems_only)` (ADR-015). | `TextEngine`, `SpatialIndex` |
| **Resolver** | Variables (per theme), component instances, props, variants, overrides, slots and libraries → a tree of resolved **occurrences** (each with its address, source and problems). An unresolved reference stays a reference and is reported, never guessed. Cached per node and kept good by the change set and a variable-to-readers index (ADR-010, ADR-011). | `LibrarySource` |
| **Layout** | Builds the `LayoutTree` from resolved occurrences and resolves hug / fill / fixed (ADR-002) before any vendor style; incremental via dirty sets. A fill on the main axis of a hugging parent uses its fallback or hugs, and says so (ADR-012). | `LayoutEngine`, `TextEngine` |
| **Scene** | One scene per page from resolved roots and layout boxes: point and marquee selection on transformed boxes, inherited visibility and locking, rounded clips, snapping from indexed candidates only, `find_space` (deterministic best-first search, bounded at 10,000 probes). Bounds-based, not shape coverage (ADR-014). | `SpatialIndex` |
| **Display list** | Visible occurrences → a versioned binary `DrawCmd` buffer plus resources (§3.2). Culls conservatively: strokes, shadows, blurs and text overflow widen the query; SVG paths and shaders are never culled. | `FontSource`, `PathOps`, `TextEngine` |
| **Diff / merge** | `Document` diff → ops in an order the op layer accepts; three-way merge by id and property, with conflict nodes (§4.5). | — |
| **Document API** | The `execute` program API and script-node semantics: one method registry that also generates `vellum-doc.d.ts` and `doc.apiVersion`; staged calls, one commit (§7.3, ADR-019). | `ScriptRuntime`, `Services` |
| **Files** | Atomic writes, asset collection on explicit save, fonts, library files and cache, the directory format, the large-file suggestion (§4.3). | `FileSystem`, `AssetStore`, `Codec` |
| **Sessions and turns** | The session event codec, usage by token kind, estimated cost from `pricing.json` (micro-USD, always an estimate), conversation history for the next turn, and the turn state machine as a table (§7.6, ADR-006). | `SessionStore` |
| **Export** | SVG, specs, tokens via registries. | `TokenFormat`, `SpecFormat` |

**WASM boundary:** the engine and the Skia renderer are one emscripten module (ADR-001), with plain C exports and a thin hand-written JS layer. The engine owns the document. JS sends ops and input events in and gets back change notifications; display-list buffers stay in WASM memory (no JSON, no per-command calls), and a frame needs only the view transform from JS. The React UI never holds its own copy of the tree.

---

## 6. Clients

| Layer | Choice | Swap point |
|---|---|---|
| Build | Vite (static bundle; a VS Code webview can't run a server) | — (Tier C) |
| Controller | `editor-core` (TS, Zustand vanilla) | Framework-agnostic by design |
| UI | `editor-react`: React + shadcn + Base UI | Replaceable without touching `editor-core` |
| Canvas | `Renderer` port: Skia in the WASM module, on WebGL2; Canvas2D basic fallback | §3.2 |
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

**Electron over Tauri** (no ADR yet): Tauri uses each OS's web engine, so WebGL, fonts and canvas behave slightly differently per platform. *Revisit when* Tauri can bundle a single engine or our golden suite passes on WebKit and WebView2. Because the shell only implements `Host`, switching is a contained job.

---

## 7. The `vellum` daemon

One static Rust binary: MCP server, CLI, agent runner, headless renderer and file watcher. Composition root for native.

| Need | Crate |
|---|---|
| MCP adapter | Hand-written over the IPC transport (ADR-021); `rmcp` not used |
| Local IPC to editors and CLI | Tokio's own Unix socket and named-pipe APIs plus direct Win32 security calls, in `vellum-ipc-local` (ADR-003); `interprocess` not used |
| Script runtime (`execute`, script nodes) | `rquickjs` behind `ScriptRuntime` |
| File watching | `notify` |
| Headless rendering | `render-skia` adapter |
| Headless browser (v1.1) | `chromiumoxide` (CDP) behind `BrowserPort` |
| Session store | `rusqlite` (bundled SQLite) behind `SessionStore` (ADR-006) |
| CLI adapter | `clap` |
| Logging | `tracing` |

### 7.1 Live vs headless

Per file: if an editor holds it, `LiveTarget` forwards ops to that editor (canvas updates in real time, agent cursors shown). Otherwise `HeadlessTarget` loads it into a native engine, applies ops through the Guard, renders, and saves after every transaction; a failed save keeps the change and retries on the next save. External agents work in both modes, so an agent can design with no app open, in CI, or on a server.

- **One writer per file** (ADR-020). The first editor becomes the writer; a headless target hands over only once everything is saved. Later editors attach and follow every applied transaction, unsaved changes included. When the writer closes, the first attached editor is promoted; when nobody is left, the next tool call opens the file headless again. A writer that stops answering is dropped. Without the daemon an editor cannot know who writes, so it opens read-only.
- **Recovery copies.** For a live file with unsaved changes the daemon writes a copy to `~/.vellum/recovery/` at most every 5 s (two atomic files named by a hash of the path). A save, Discard backup or a deliberate close removes it; a lost writer keeps it. Restoring applies it as one transaction by `Actor::File`.

### 7.2 Local connection security

- The daemon listens on a **Unix domain socket** (`vellum.sock`, 0600, macOS/Linux) or **named pipe** (`\\.\pipe\vellum-<user SID>-<run-dir hash>`, Windows, a DACL for the current user only, remote clients refused) inside `~/.vellum/run/`, a directory only the user can read (ADR-003). There is no TCP port, so no web page in any browser can reach it, which removes the whole DNS-rebinding class of attacks. Unsafe existing paths (wrong owner, broad permissions, symlinks) are refused, never repaired.
- **One daemon per runtime directory.** `startup.lock` serializes starts and the daemon holds `daemon.lock` for its lifetime (OS file locks, released on a crash). Hosts and `vellum mcp` start or reuse it; there is no PID killing.
- Editor webviews never connect directly: the VS Code extension host and Electron's main process hold the connection and relay messages to the webview.
- Each connection sends a bounded hello with a token from `~/.vellum/run/daemon.json` (0600; a fresh 256-bit token each start), a protocol version, a channel and the client's working directory. Authentication comes before version negotiation. At most 64 sessions; handshakes have a 3 s deadline; frames are capped at 1 MiB.
- A TCP listener exists only behind an explicit `vellum daemon --listen <ip:port>` flag for remote dev containers: one bounded `POST /rpc` per connection with a Bearer token, an exact `Host`, and an allowlisted `Origin` when present (`null` refused). No CORS, cookies or websockets.

### 7.3 Tools

**Two layers over one registry.** Both are thin front ends over Ops (§5), so they share validation, the Guard, attribution and undo.

**Layer 1 — `execute`: one script, many operations.** The agent sends a short JavaScript program that runs in the `ScriptRuntime` sandbox against a `doc` API. Loops and variables let one call do what would otherwise take dozens of calls, which cuts tokens and round trips sharply. The whole script is **one transaction and one undo step**; if any op fails, nothing is applied and the agent gets the failing line and reason. Each mutating call plans its op against a staged copy; the finished stage is diffed back into ops, planned once more against the live document and committed once, so the target's Guard still decides (ADR-019). In the daemon the script runs on a scratch copy and the diff goes to the target as one transaction (ADR-021).

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
- Limits per call: 5 s, 2,000 API calls, 2,000 created nodes (counted before each call, descendants and instance content included), 64 MiB; callers may lower them and the Guard's own limits still apply. Each mutating call must leave valid references on its own. No network, file system, timers, Date or async; `**` refuses (use `Math.pow`); the program ends with an explicit `return`.
- Screenshots and exports from a script are capped at 8,192 px per side and 16.7 megapixels, and return MIME, base64 and size; nothing is written to disk.
- `execute({ code, validate_only: true })` dry-runs the script and reports what would change.
- The API is versioned (`doc.apiVersion`) from one registry that also generates `vellum-doc.d.ts` for agents; breaking changes ship as a new major version with the old one kept for two minor releases (§2.5).

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

Mutating tools take a batch, run as one transaction and one undo step, and return created IDs. A bad op rejects the batch with a precise error the agent can act on: the batch item (`op`), the field in the input's own spelling (`nodes[1].opacity`) and a plain reason.

**How the daemon runs tools** (ADR-021):

- **One thread owns the documents.** Requests run one at a time; a tool that panics is answered as an error and the thread carries on.
- **Each MCP connection is one agent**, named after its client's `initialize` name, with its own `AgentId` and `SessionId`; its transactions carry them. External calls carry no turn; the built-in engine's do.
- **Every call is a step** (verb, target frame, the first document it read or wrote, counts, thumbnail request, error, timing). After each call the daemon moves the connection's steps into the session store as an external session, one per document, with thumbnails drawn and cached (§7.6).
- **Where a document may be.** The connection's hello carries the client's working directory. A relative path means a path in it, and anything outside it is refused until the `PermissionBroker` can ask (§7.7).
- **The `vellum tool <name>` CLI** has a subcommand per tool and a flag per schema property, and goes to the daemon like any client.

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

- `SessionStore` keeps an append-only event log per session (ADR-006): your message and context, the messages a turn added (the next turn's history), plan, each step (tool, arguments and result previews cut at 2,000 characters with a marker, thumbnail reference, duration, outcome done / failed / rejected), usage, and each turn transition. Every event carries its origin (actor, session, turn) and the store refuses an event from another session.
- **Usage** is counted per token kind (input, output, cache read, cache write; `ModelUsage` means the same for every provider) with an estimated cost in micro-USD from the shipped `pricing.json`, always labelled as an estimate. A model or kind with no price has no estimate, never a guessed one.
- Thumbnails and screenshots are cached in `~/.vellum/cache/` by SHA-256, not stored in the database; messages hold references to them.
- Sessions are keyed by repo root (the nearest folder holding `.git`) + the file's repo-relative path, so renaming a branch doesn't lose them. Deleting a session removes its events and the cached images no other session uses.
- External MCP sessions are recorded the same way (their tool calls only, one session per document), which is what lets them appear in the Agent panel and History.
- **Turns** follow guide §10.1 as a table: queued → planning → (awaiting plan →) running ⇄ awaiting permission → done | failed, and Stop from any non-terminal state — only a person stops a turn. A step running over 120 s is flagged stuck without changing state. Every transition appends one event with its actor. The session dot is a total function of the state; a stopped turn shows done ("Stopped by you").

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
editor = "skia"             # "canvas2d" fallback, "vello" experimental

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
| WASM bundle | Core and Skia renderer in one module, about 2 MB gzipped with Parley text (measured 1.95 MB in ADR-001; QuickJS adds about 0.4 MB); fonts load as side resources |

Port design respects these: hot-path ports are generic (static dispatch), the display list stays in WASM memory with the renderer, and benchmarks run against every adapter, not just the default. Measured so far on an Apple M5 Pro only: 232 fps panning 10,000 nodes in the spike (ADR-001), a 200-node batch planned in 13 ms and undone in 15 ms at 10,000 nodes (ADR-004).

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
      codec-json/  codec-json-dir/  assets-fs/  assets-s3/  watch-notify/  ipc-local/
      render-skia/  render-vello/ (later)  shader-sksl/ (v1.1)
      llm-openai-compat/  llm-openai-responses/  llm-anthropic/  llm-google/  model-http/  secrets-keyring/
      script-quickjs/  sessions-sqlite/  browser-cdp/ (v1.1)
      engine-builtin/  engine-claude-code/  engine-codex/
      import-svg/  import-image/  import-figma-clipboard/  import-figma-api/ (v1.1)  import-html/ (v1.1)
      icons/  images-stock/  image-ops/ (v1.1)
      export-tokens/  export-html/  export-raster/
    vellum-wasm/         composition root: browser engine + Skia renderer (one emscripten module)
    vellum/              composition root: daemon (tool registry, MCP + CLI adapters, live/headless targets)
    sync-server/         composition root: sync (ports + Postgres/Redis/R2 and SQLite/disk adapters)
    vellum-testkit/      stub model endpoint, fixture documents, Chrome layout oracle runner, pixel diff

  packages/
    types-ts/            generated from vellum-types (never hand-edited)
    renderer-api/        Renderer interface
    renderer-skia/       renderer-canvas2d/
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
    ui-tokens/           shared tokens (light, dark, high contrast)
    config/              brand and exact copy strings

  apps/
    extension/  desktop/  account/  api/  web/  docs/

  docs/
    architecture/decisions/  architecture decision records (ADR-NNN-title.md)
    format/              .vellum spec + generated JSON Schema
  fixtures/              golden documents, reference images, layout-oracle HTML
```

### Work packages

| Package | Port(s) | Fake for others to build against | Done when |
|---|---|---|---|
| WP1 Types + ports | all `vellum-core::ports` | fakes in `contracts/` | Ports reviewed, ADRs 001–006 written |
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
| **M0: Spine** | WP1 + fakes, minimal core ops, Taffy + Parley adapters, the Skia WASM module + canvas2d drawing a display list in a web page, CI gates (§11) wired | Pan/zoom a 1,000-node document at 60 fps, and a dependency-rule violation fails CI |
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

1. ~~**Text in the display list.**~~ Resolved by ADR-001 and ADR-013: Parley lays out, Skia draws positioned glyphs; no Skia paragraph layout.
2. ~~**`DocStore` granularity.**~~ Resolved by ADR-004: transactions over our types with an in-memory store and Loro behind one contract suite; undo lives in core, not in the CRDT. An Automerge adapter was not built.
3. ~~**Loro at scale.**~~ Resolved by ADR-004 and ADR-018: about 29 ms to open and 4 MB of heap at 10,000 nodes; the directory format is suggested at 10,000 nodes or 4 MB and never required.
4. **`execute` vs typed tools:** which should the skill recommend by default, per model size? Measure token cost, error rate and design quality with 2–3 models in M1. Same question for the compact JSX-like insert syntax vs JSON.
5. ~~**QuickJS in the editor.**~~ Resolved by ADR-004 and ADR-005: about 0.4 MB gzip in the editor module and 0.2 ms for a 500-op script; the same corpus runs natively and in WASM with identical results.
6. **Stock and generated images:** pick a stock provider with a licence that allows use in shipped products, and decide whether generated images need a visible "generated" marker in the file.
7. **Installed-engine contracts:** Claude Code and Codex headless modes and their permission systems change between releases; pin tested versions per Vellum release and detect unsupported ones at start-up.
8. ~~**CanvasKit vs Vello.**~~ Resolved by ADR-001: Skia in WASM, no CanvasKit; Vello on its revisit conditions.
9. **Interop:** Figma import via their REST API and SVG paste are fine; reading other tools' proprietary formats (including `.pen`) needs legal review first. Matching a competitor's *capabilities* is fine, and plain descriptive tool names (`execute`, `get_variables`) that happen to match help agents carry habits over. Copying its script API, prompts, skills text, docs or UI is not; ours are written from scratch.
10. ~~**Daemon lifecycle.**~~ Resolved by ADR-003: one daemon per runtime directory (by default one per user), started or reused through a startup lock by every host and `vellum mcp`.
11. **License:** Apache-2.0 for core, CLI, format and ports (adoption) vs AGPL for the sync server (protects hosted business).
