# ADR-021: the tool registry, its MCP and CLI front ends, and the read tools

Status: implemented for Prompt 29 in the `vellum` composition root. The write tools and `execute` arrive in
Prompt 30. Skills and styles in Prompt 31.

## Decision

**The registry is the API.** `crates/vellum/src/tools/` holds `Tool`, `Registry`, `ToolCtx`, `ToolError` and
the eleven read tools. ARCHITECTURE §13 puts the registry in `crates/vellum`, so it is not a separate crate.

```rust
pub trait Tool {
    fn name(&self) -> &'static str;       // stable; a breaking change is a new name (§2.5)
    fn version(&self) -> u32;
    fn description(&self) -> &'static str; // shown to the agent
    fn input_schema(&self) -> schemars::Schema;
    fn read_only(&self) -> bool;
    fn destructive(&self) -> bool;         // defaults to !read_only
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError>;
}
```

It differs from ARCHITECTURE §3.4 only by the three extra methods, which MCP and the CLI need to describe a
tool. `run` still takes and returns JSON. A tool that produces a picture puts it under an `image` key
(`{ mime, base64, width, height }`); the MCP front end turns that into an image block and leaves the base64
out of `structuredContent`.

**Input schemas** come from `#[derive(JsonSchema)]` on each tool's input struct, with `deny_unknown_fields`,
so a misspelled field is an error that names it (`serde_path_to_error` gives the path). The generator inlines
shared types and writes an optional field as its plain type, not `["string", "null"]`: smaller, and no client
has to chase a `$ref` or a `null`. Field doc comments are the field descriptions the agent reads.

**Errors** are `ToolError { kind, message, path }`. A message says what is wrong and what to do (`No document is
open. Call open_document ...`). Kinds: `invalid_input`, `not_found`, `document`, `refused`, `render`,
`internal`. The Guard's and the target's errors map onto them. A batch error with an op index is Prompt 30.

**Tools reach documents only through `ToolCtx`**, which holds `Documents` (the targets of Prompt 28) and the
connection's `Session`. A tool cannot tell a live file from a headless one. The daemon-wide "active document"
(the one opened or used last) is what a call without `document` means; with one document open it is that one.

**The read tools** and what each is built from:

| Tool | Built from |
|---|---|
| `open_document` | `Documents::target` / `create_blank`; reports pages and who holds the file. Not read-only: it can make a file (never replace one) |
| `get_app_state` | `Documents::view` (selection, viewport, leases, pauses from the editor, or from the headless Guard), plus notes for agents and context nodes in scope |
| `get_nodes`, `query_nodes`, `find_properties` | a one-pass `Index` of the document (pages in order, then component trees) |
| `get_variables`, `list_components`, `list_fonts` | `resolve_variable` under a `Theme`; the document; `Services::font_families` |
| `snapshot_layout`, `find_space` | `Services::layout` (new) and `Method::FindSpace` on the snapshot, so they answer the same for a live file |
| `screenshot` | `DocumentTarget::render`: the editor's renderer when one holds the file (Invariant 6) |

Every list is bounded and says when it is cut (`truncated`, `childCount`, `total`). `get_app_state` scopes
notes and context to the selection, its ancestors, and what is inside it; with no selection, to the page.

**Core additions** (`vellum-core`): `Services::layout` and `Services::font_families` (defaults say "requires a
host scene service"; not part of the script API, so the d.ts and `doc.apiVersion` are unchanged),
`FontSource::families`, `PauseSet::{paused, stopped}`, `Problem::nodes` public, `problem_json`. The renderer's
incomplete-render error now names the node and kind in words.

**MCP front end** (`mcp.rs`). It is not `rmcp`. Reasons:

1. ADR-003 settled the transport: a bridge that relays lines, a private end marker, bounded frames, a
   handshake that already authenticated and versioned the connection. `rmcp`'s server wants to own the
   handshake and the byte stream; fitting it needs a duplex pair per connection and still cannot serve the
   opt-in HTTP `POST /rpc`, which is one request per connection with no session.
2. The documents are not `Send`. One thread owns them (below); `rmcp`'s service is async and multi-task.
3. The surface used is five methods. `initialize`, `ping`, `tools/list`, `tools/call` and `server/discover`
   are about 150 lines, and the same dispatcher serves the stdio bridge, HTTP and the CLI.

The cost is that protocol conformance is ours. It speaks the `initialize` revisions 2024-11-05 to 2025-11-25
(an unknown version is answered with the newest) and answers `server/discover` for the stateless revision
(2026-07-28); `tools/list` and `tools/call` need no session state, so a client that skips `initialize` works.
Capabilities are `tools` only. If MCP features beyond tools are wanted (resources, progress, elicitation for
the PermissionBroker) revisit `rmcp`; the dispatcher is one file. `rmcp` stays in the version baseline as
"not used".

A tool result is `content` (the value as compact JSON, then an image block when there is a picture),
`structuredContent` (the value), and `isError`. A failed tool is an error *result* with `structuredContent.error
{ kind, message, path }`, so the agent can read and fix it. A protocol failure (unknown tool, malformed call) is
a JSON-RPC error. `tools/list` carries `readOnlyHint`, `destructiveHint`, `openWorldHint: false` and
`_meta["vellum/version"]`. The whole list is a snapshot test (`tests/snapshots/mcp-tools.json`, ARCHITECTURE
§2.5); `VELLUM_UPDATE_SNAPSHOTS=1` rewrites it.

**One thread owns the documents** (`vellum-ipc-local`). The daemon builds the handler on a dedicated thread
and sends it every request over a channel (`RpcHandler`: `opened`, `request`, `closed`, `idle`). Requests run
one at a time, so a slow tool delays the others; a tool that panics is answered with `-32603` and the thread
carries on. Tokio stays inside the IPC adapter (deny.toml). `idle` runs about four times a second and calls
`Documents::poll` and `tick` (watcher, recovery copies); `poll` also runs before every `tools/call`, so a read
sees the file as another program last saved it.

**Where a document may be.** The hello gains an optional `cwd`: the directory the client was started in. Per
connection, a relative document path means a path in it, the key a document goes by is the absolute path, and
a path outside it is refused (`../`, an absolute path elsewhere). Without a `cwd` (HTTP, an old client) the
path is used as given, and the production locator requires an absolute `.vellum` path. The locator roots a
document at the nearest folder above it with a `.git`, so a library or assets folder one level up stays
inside. **Anything outside the client's directory is refused until the PermissionBroker (Prompt 35) can ask.**
This is deliberately stricter than "inside the repo": a client started in a subfolder cannot reach the rest of
the repo.

**CLI front end** (`cli.rs`). `vellum tool <name> --field value` has a subcommand per tool and a flag per
schema property (`max_nodes` is `--max-nodes`; an enum is a list of allowed values; an array repeats or takes
commas; an object is JSON). Help comes from the schema. The call goes to the daemon like any client's, so it
reaches a live file; the result prints as JSON, `--json` prints the whole MCP result, and a failure exits 1 with
the reason on stderr (usage errors exit 2). `--image-out FILE` saves a picture. The `vellum shell`, `vellum
prompt` and `vellum export` commands of §7.8 are later prompts.

**Production composition** (`daemon::production`): `StdFs` rooted per repo, `JsonCodec`, `LoroDocStore`
(random peer id below 2^63), `RandomIds` (ten characters from the id alphabet, OS randomness), a monotonic
system clock, `NotifyWatcher` (a machine that cannot watch still serves tools), recovery copies in
`<state>/recovery`, and the scene: Taffy, Parley, rstar, Skia. If Skia cannot start, the services say so on
every call.

**The `scene` feature** (default on) gates the four scene adapters in `crates/vellum`. `scripts/ipc.mjs`
builds the daemon with `--no-default-features`, so the IPC lane compiles no renderer, as ADR-020 and the
runbook promised. This replaces ADR-020's consequence "does not build Skia": the shipped binary does.

## Consequences

- Prompt 30 adds mutating tools to the same registry; the MCP and CLI front ends need no change. A mutating
  tool needs an `Origin` for the session: `Session::client` is already filled from `initialize`, and the
  `ActorId`, name and hue arrive with sessions (Prompt 34).
- One set of scene services serves every document. It has no document path, so **libraries (`imports`) and
  script-node outputs do not resolve in the daemon's layout, screenshots and `find_space`**. Instances of a
  library component draw as missing. Fixing it means services per document (a factory in `Env`).
- Note, context and prompt nodes have no drawing in the display list, so a `screenshot` of a frame that holds
  one fails and names it. The same limit applies to `doc.screenshot`. This needs a decision on how the canvas
  draws them (Invariant 6), which is a renderer and design question, not this prompt's.
- Live files are tested only in process, with a fake editor. A real editor connects with the host relay,
  whose wire protocol (ADR-020) is still open; `EditorLink::view` is part of what it must carry.
- The daemon serves one request at a time, so `get_nodes` on a large file blocks a concurrent `screenshot`.
  Budgets (2,000 nodes, 700 KiB images) keep each call short. Revisit with the first measured stall.
- An `rmcp` swap, if ever wanted, replaces `mcp.rs` and the line handling in `vellum-ipc-local`; tools do not
  change.

## Reference note

`refs/rmcp` (README, `counter_stdio`, `structured_output`, `progress_demo`, and `model.rs` for the revision
list, the `tools/call` result fields and `server/discover`) was read for the wire shapes. Nothing was ported.
`refs/penpot/mcp/.../HighLevelOverviewTool.ts` (ideas only, MPL-2.0) is a tool whose only job is to return usage
instructions. Vellum keeps the idea that an agent needs a short orientation first, and puts it where a client
cannot skip it: the server `instructions` at `initialize` and `get_app_state` as the named first call; the
long form is `read_skill` (Prompt 31). The tool set, names, schemas and text are Vellum's own (Invariant 14).
