# Tools: the registry, MCP and the CLI

The tool registry (`crates/vellum/src/tools/`) is Vellum's API. MCP (`vellum mcp`) and the CLI (`vellum tool`)
are generated from it; ADR-021 has the decisions.

## Try it

```sh
cargo build -p vellum
vellum tool --help                       # every tool, from the registry
vellum tool get_nodes --help             # its flags, from the tool's input schema
cd ~/code/my-repo
vellum tool open_document --path design/app.vellum
vellum tool get_app_state
vellum tool query_nodes --type text --name title
vellum tool screenshot --node <frame id> --image-out frame.png
```

The first call starts the daemon. Documents are `.vellum` files inside the directory you ran the command in; the
document opened last is the one later calls use, or pass `--document`. A tool that fails exits 1 with the
reason on stderr; `--json` prints the whole MCP result.

To use it from an MCP client, register `vellum mcp` as a stdio server. The client starts it in the project's
directory, and that directory is where documents are looked up.

## The tools (Prompt 29)

| Tool | Use |
|---|---|
| `get_app_state` | What is open, the selection and viewport, leases, paused agents, notes for agents and context nodes in scope. Call first |
| `open_document` | Open a file, or make one with `create` |
| `get_nodes` | Nodes by id, children to a depth |
| `query_nodes` | Find by name, type, text, component, variable |
| `find_properties` | Every distinct value of a property in a scope (font sizes, radii, colours) |
| `get_variables`, `list_components`, `list_fonts` | What the document defines for reuse |
| `snapshot_layout` | Computed bounds and layout problems |
| `find_space` | An empty area for a new frame |
| `screenshot` | A PNG of a page or node, drawn the way the editor draws it |

## Adding a tool

1. Write a struct implementing `Tool` in `tools/`, with an input struct that derives `Deserialize` and
   `JsonSchema` and has `#[serde(deny_unknown_fields)]`. Doc comments on its fields are what the agent reads.
2. Parse with `parse_input`, reach documents through `ctx`, return JSON. Say what was cut (`truncated`).
   Error messages say what to do next.
3. Register it in the module's `register`.
4. Add tests in `crates/vellum/tests/tools.rs`.
5. Review and rewrite the schema snapshot: `VELLUM_UPDATE_SNAPSHOTS=1 cargo test -p vellum --test mcp`.
   Names are stable and schemas only grow (ARCHITECTURE §2.5); a breaking change is a new tool name.

The MCP front end and the CLI need no change.

## Tests

```sh
cargo test -p vellum --test tools      # registry and read tools over fakes (real scene for layout and drawing)
cargo test -p vellum --test mcp        # the protocol, errors, images, sessions, the schema snapshot
cargo test -p vellum --test mcp_stdio  # real `vellum mcp`, real daemon, real files and Skia
cargo test -p vellum --test cli        # `vellum tool`
node scripts/ipc.mjs                   # the transport lane, built without the renderer
```

When a stdio test fails, its daemon's output is printed under "daemon output".
