# ADR-034: agents reach the document operations the editor has; measured inputs come next

Status: accepted for phase 1 (2026-10-23, requested by the owner: "my MCP should do whatever Pencil's MCP can do,
then more"). Phases 1b and 2 are proposed here and not built. Amends the parity claim in ARCHITECTURE §7.4 for
the rows named below until they land.

## Context

ARCHITECTURE §7.4 says Vellum covers every pen.dev capability under its own names. Checked against the code on
2026-10-23, that holds for the five MCP tools as functions (`execute`, `get_app_state`, `read_skill`, `get_style`,
`browser`), but not for the document operations behind them:

- `vellum_core::ops::Op` has 30 variants. The tool layer builds 10 of them (insert, update, delete, move, copy,
  replace, set variables, create component, make slot, set shell). Twenty have no tool that builds them: group,
  ungroup, wrap in a frame, toggle component, detach, replace in instance, scale, boolean, outline stroke, align,
  distribute, flip, convert script, insert, delete, update and move page, set component, update component, and
  reorder (in effect covered by `move_nodes` with an index).
- The `execute` script API (`doc`) has 21 methods and none of the twenty.
- No tool can add, rename, move or remove a page.
- The daemon applies an agent's transaction with `Inputs::default()` (`crates/vellum/src/daemon/headless.rs`): no
  measured boxes, no path geometry, no `PathOps`, no script outputs. So boolean, outline stroke and convert
  script cannot succeed through an agent, and align, distribute and scale fail on hug or fill sizes. The editor's
  WASM engine supplies measured boxes for top-level layers only (`crates/vellum-wasm/src/engine/mod.rs`).

The comparison used the tools' own schemas and this repository's §7.4, not pen.dev's internals (guide Invariant 14).

## Decision

**Phase 1 (built).** Four typed tools, each one batch committed as one transaction through the Guard, using the
same `Batch`, `Label` and error mapping as the write tools (ADR-021, `crates/vellum/src/tools/batch.rs`):

| Tool | Items | Core ops |
|---|---|---|
| `manage_pages` | `insert`, `update`, `move`, `delete` | InsertPage, UpdatePage, MovePage, DeletePage |
| `manage_components` | `update` (name, props, variants; what is left out stays), `remove` | UpdateComponent, SetComponent (None) |
| `instance_nodes` | `detach`, `toggle` (`componentId` for a layer), `replace` (instance, path, node) | Detach, ToggleComponent, ReplaceInInstance |
| `arrange_nodes` | `group` (`into`: group or frame), `ungroup`, `flip`, `scale` (`factor`, `anchor`), `align`, `distribute` | Group, WrapInFrame, Ungroup, Flip, Scale, Align, Distribute |

Rules the tools follow:

- **Wrappers take their place from the children.** A group or frame sits at the bounds of its children when they
  all have fixed positions and fixed sizes (a group takes `hug` size; a frame takes the bounds as a fixed box).
  When they all follow a layout, the wrapper follows it too. Mixing the two is refused, with the item named.
- **The file format is not inlined.** Props and variants are accepted as objects in the file format and read with
  serde inside the tool; the input schema does not expand `Node`, `PropDef` or `Variant`. The first draft's
  `manage_components` schema was 1.6 MB. `write_tools` now fails any schema over 16 KB.
- **Verbs reuse existing codes.** The editor's step map (`packages/editor-core/src/agent/steps.ts`) is exhaustive
  over `Verb`, so a new verb would change the UI. Pages and arrangements are `updated`; instances and component
  definitions are `made_component`. The map names the new tools for their running steps.
- **Destructive flags.** `manage_pages`, `manage_components` and `instance_nodes` remove or redefine things and
  are destructive; `arrange_nodes` removes nothing and is not.

Not chosen: extending only `execute` (typed tools serve clients that prefer schemas, ARCHITECTURE §7.3 layer 2);
one tool with an `action` enum per op (flat batches keep the per-item error paths that the write tools use).

**Phase 1b (proposed, not built): measured inputs.** Before an agent's transaction is applied, the daemon computes
`Inputs` for the document: measured boxes from the layout engine, path geometry and a `PathOps` adapter
(`vellum-pathops-kurbo`, a dependency of the composition root `crates/vellum`), and script outputs from the script
runtime. That makes boolean, outline stroke and convert script work through agents, and hug-sized align,
distribute and scale too. Until it lands, those ops return the core's "missing" error, and the tools do not
pretend otherwise.

**Phase 2 (proposed, not built).**
- `execute` methods for the phase 1 ops, so scripts reach what typed tools reach.
- A print channel, a name-to-id map and layout warnings in an `execute` result, and several images returned.
- `browser`: a read-back of a selector's DOM and computed styles, and a screenshot placed on the canvas.
- Raw Chrome DevTools commands, and file writes from scripts, only behind `PermissionBroker`. Neither exists today.
- Host controls an agent could use (selection, viewport, undo, version history). These act on a live editor and
  outside the canvas, so they need an owner decision on the permission each one takes. The reports' open
  "share authority" item is the same question for sharing.

## Consequences

- Agents can now create, rename, reorder and remove pages, edit component definitions, detach or override
  instances, and group, flip, scale, align and distribute layers. The write tools' Guard, leases and undo apply
  unchanged: each call is one transaction and one undo step.
- The tool count goes from 39 to 43. Names are new and schemas only grow (ADR-021).
- §7.4 still says Vellum covers every capability. Until phases 1b and 2 land, booleans, outline stroke and convert
  script are not reachable by agents, nor are align, distribute and scale on hug-sized layers, nor the `execute`
  methods for the new ops.
- No new network, file or permission path. `PermissionBroker` and the browser's network rules are untouched.
- Tests: `crates/vellum/tests/write_tools.rs` (the new tools, the registration and destructive table, the schema
  size guard), `crates/vellum/tests/snapshots/mcp-tools.json` (additions only).
