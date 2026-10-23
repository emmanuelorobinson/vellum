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

## The write tools (Prompt 30)

Every call is one transaction and one undo step, by the agent this connection is, through the Guard. All of it
applies or none of it does.

| Tool | Use |
|---|---|
| `execute` | A JavaScript program against `doc`; `validate_only` reports what would change |
| `insert_nodes` | Nodes as JSON (`nodes`) or a spec (`spec`, `format` `jsx` or `json`) |
| `update_nodes`, `copy_nodes`, `replace_nodes`, `move_nodes`, `delete_nodes` | Batches of node changes |
| `replace_properties` | Every matching value of a property in a scope (`#111` → `$color/text`) |
| `set_variables` | Add, change (by id or name) or remove variables |
| `create_component`, `create_instance`, `make_slot` | Components, instances, slots |
| `claim_frame`, `release_frame` | Hold a frame against other agents; 60 s without activity frees it |
| `report_progress` | A progress step for the person; changes nothing |

```sh
vellum tool insert_nodes --parent <frame id> --spec '<rect name="Rule" width={390} height={1}/>'
vellum tool update_nodes --updates '[{"id":"<id>","set":{"name":"Title"}}]'
vellum tool execute --code 'return doc.query({type: "text"}).length;'
```

A refused batch fails with `op` (which item, from 0), `path` (the field, as the input spells it) and the reason;
a failed script with its `line`. Fix that item and call again. Each call also leaves a step for the Agent panel
(`Session::steps`).

## Icons, pictures and imports (Prompt 42)

`search_icons`, `insert_icon`, `fill_image`, `import`. See [Icons, pictures and imports](imports.md): the libraries,
the importers, what asks for permission and what each leaves out.

## Pages, components, instances and arrangement (ADR-034)

| Tool | Batch items | Use |
|---|---|---|
| `manage_pages` | `insert`, `update`, `move`, `delete` | Add, rename, change the kind of, reorder and remove pages. Inserts run first |
| `manage_components` | `update` (name, props, variants), `remove` | Rename a definition, replace its props or variants, remove one no instance uses |
| `instance_nodes` | `detach`, `toggle` (`componentId`), `replace` (`instance`, `path`, `node`) | Detach instances, make a layer a component, turn an instance back into layers, override a layer inside one |
| `arrange_nodes` | `group` (`into`: group or frame), `ungroup`, `flip`, `scale`, `align`, `distribute` | Group or frame layers, take groups apart, mirror, scale, line up and space layers |

```sh
vellum tool manage_pages --insert '[{"name":"Onboarding"}]'
vellum tool arrange_nodes --group '[{"ids":["textTitle1","textBody01"],"name":"Intro"}]'
vellum tool instance_nodes --replace '[{"instance":"instBtn001","path":"buttonRoot/buttonText","node":{"type":"text","name":"Label","width":"hug","height":"hug","content":[{"text":"Go","font":"Inter","size":14}]}}]'
```

A group or frame sits at the bounds of its children when they all have fixed positions and sizes. Layers that
follow a layout can be grouped only with each other. Booleans, outline stroke and convert script are not in these
tools yet, and neither are align, distribute or scale on hug-sized layers: the daemon applies agents' changes
without measured boxes, path geometry or script outputs (ADR-034, phase 1b).

## Adding a tool

Prompt 31 adds `list_skills`, `read_skill(path?)` and `get_style(name?)`; all work without an open document.
See [Skills, styles and parity](skills-and-parity.md) for source configuration and the coverage inventory.

1. Write a struct implementing `Tool` in `tools/`, with an input struct that derives `Deserialize` and
   `JsonSchema` and has `#[serde(deny_unknown_fields)]`. Doc comments on its fields are what the agent reads.
2. Parse with `parse_input`, reach documents through `ctx`, return JSON. Say what was cut (`truncated`).
   Error messages say what to do next.
3. Register it in the module's `register`.
4. Add tests in `crates/vellum/tests/tools.rs` (read tools) or `write_tools.rs` (mutating tools). A mutating
   tool sets `verb`, builds a `Batch` with a `Label` per op, and calls `ctx.commit` once.
5. Review and rewrite the schema snapshot: `VELLUM_UPDATE_SNAPSHOTS=1 cargo test -p vellum --test mcp`.
   Names are stable and schemas only grow (ARCHITECTURE §2.5); a breaking change is a new tool name.

The MCP front end and the CLI need no change.

## Tests

```sh
cargo test -p vellum --test tools      # registry and read tools over fakes (real scene for layout and drawing)
cargo test -p vellum --test write_tools  # write tools, execute (real QuickJS), batch errors, steps
cargo test -p vellum --test mcp        # the protocol, errors, images, sessions, the schema snapshot
cargo test -p vellum --test mcp_stdio  # real `vellum mcp`, real daemon, real files and Skia
cargo test -p vellum --test cli        # `vellum tool`
node scripts/ipc.mjs                   # the transport lane, built without the renderer
```

When a stdio test fails, its daemon's output is printed under "daemon output".
