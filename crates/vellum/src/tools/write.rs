//! Writing nodes: `insert_nodes`, `update_nodes`, `copy_nodes`, `replace_nodes`, `move_nodes`, `delete_nodes`
//! (ARCHITECTURE §7.3). Each call is one batch: one transaction and one undo step, all of it or none.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::ops::Op;
use vellum_types::NodePatch;

use super::batch::{Batch, Label, count, ids_in_use, new_node, node_id, place};
use super::spec::{formats, parse_spec};
use super::support::{parse_input, schema_of};
use super::{Tool, ToolCtx, ToolError, Verb};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(InsertNodes));
    tools.push(Box::new(UpdateNodes));
    tools.push(Box::new(CopyNodes));
    tools.push(Box::new(ReplaceNodes));
    tools.push(Box::new(MoveNodes));
    tools.push(Box::new(DeleteNodes));
}

/// The most items one call takes. The Guard's own op limit applies on top.
pub(super) const MAX_ITEMS: usize = 500;

pub(super) fn check_items<T>(items: &[T], field: &str) -> Result<(), ToolError> {
    if items.is_empty() {
        return Err(ToolError::invalid(field, "Give at least one."));
    }
    if items.len() > MAX_ITEMS {
        return Err(ToolError::invalid(
            field,
            format!("At most {MAX_ITEMS} in one call; split the batch."),
        ));
    }
    Ok(())
}

/// The nodes a call gives, as JSON objects or as a spec in one of the registered formats.
pub(super) fn spec_nodes(
    nodes: Option<Vec<Value>>,
    spec: Option<String>,
    format: Option<String>,
) -> Result<Vec<Value>, ToolError> {
    match (nodes, spec) {
        (Some(_), Some(_)) => Err(ToolError::invalid(
            "spec",
            "Give `nodes` or `spec`, not both.",
        )),
        (Some(nodes), None) => Ok(nodes),
        (None, Some(spec)) => {
            let format = format.unwrap_or_else(|| "jsx".to_owned());
            match parse_spec(&format, &spec) {
                None => Err(ToolError::invalid(
                    "format",
                    format!(
                        "There is no spec format {format:?}. The formats are: {}.",
                        formats().collect::<Vec<_>>().join(", ")
                    ),
                )),
                Some(Err(error)) => Err(ToolError::invalid(
                    "spec",
                    format!(
                        "The spec does not read at line {}, column {}: {}",
                        error.line, error.column, error.message
                    ),
                )
                .at_line(error.line, Some(error.column))),
                Some(Ok(nodes)) => Ok(nodes),
            }
        }
        (None, None) => Err(ToolError::invalid(
            "nodes",
            "Give the nodes as `nodes` (JSON) or `spec` (text in `format`).",
        )),
    }
}

/// Where in the input the nodes of [`spec_nodes`] are: `nodes[i]`, or the spec as a whole.
fn node_at(from_spec: bool, i: usize) -> String {
    if from_spec {
        format!("spec[{i}]")
    } else {
        format!("nodes[{i}]")
    }
}

struct InsertNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InsertInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The page or layer to add them to, by id.
    parent: String,
    /// Where among the parent's children, back to front: 0 is the bottom. Defaults to the top (the end).
    index: Option<u32>,
    /// The nodes, in the file format (see read_skill). `id` and `name` may be left out; children nest.
    nodes: Option<Vec<serde_json::Map<String, Value>>>,
    /// The nodes as text instead, in `format`. With "jsx" (the default) a tag is a node's type, attributes are
    /// its properties ("text" or {json}), nested tags are children and text inside <text> is its content:
    /// <frame name="Card" width={320} height="hug" layout="column"><text font="Inter" size={14}>Hi</text></frame>
    spec: Option<String>,
    /// The spec's format: "jsx" or "json".
    format: Option<String>,
}

impl Tool for InsertNodes {
    fn name(&self) -> &'static str {
        "insert_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Add nodes (with their children) to a page or a layer, as JSON or as a compact tag spec. All of them go in \
         as one change, or none do. Returns the new top-level ids in order. A mistake is reported with the index \
         and field of the node to fix."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<InsertInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Designed
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let InsertInput {
            document,
            parent,
            index,
            nodes,
            spec,
            format,
        } = parse_input(input)?;
        let from_spec = spec.is_some();
        let nodes = spec_nodes(
            nodes.map(|nodes| nodes.into_iter().map(Value::Object).collect()),
            spec,
            format,
        )?;
        check_items(&nodes, if from_spec { "spec" } else { "nodes" })?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut taken = ids_in_use(&doc);
        let mut batch = Batch::default();
        let (mut created, mut total) = (Vec::new(), 0);
        for (i, value) in nodes.into_iter().enumerate() {
            let at = node_at(from_spec, i);
            let node = new_node(value, &doc, &mut ctx.docs().tool_ids(), &mut taken, &at)
                .map_err(|e| e.at_op(i))?;
            let mut to = place(&doc, &parent, index, None, "parent")?;
            to.index += i;
            created.push(node.id().as_str().to_owned());
            total += count(&node);
            batch.push(
                Op::Insert {
                    to,
                    node: Box::new(node),
                },
                Label::new(i, at.clone())
                    .rename("node", at)
                    .rename("to.parent", "parent")
                    .rename("to.index", "index"),
            );
        }
        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "created": created, "nodes": total}))
    }
}

struct UpdateNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Update {
    /// The layer.
    id: String,
    /// The properties to set, in the file's spelling. `null` puts one back to its default. Children are changed
    /// with insert_nodes, move_nodes and delete_nodes.
    set: serde_json::Map<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The changes, applied in order.
    updates: Vec<Update>,
}

impl Tool for UpdateNodes {
    fn name(&self) -> &'static str {
        "update_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Set properties of layers: name, size, fills, layout, text content and so on, or null to reset one. All \
         updates apply as one change, or none do; a mistake names the update and property to fix."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<UpdateInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Updated
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let UpdateInput { document, updates } = parse_input(input)?;
        check_items(&updates, "updates")?;
        let (path, _) = ctx.document(document.as_deref())?;
        let mut batch = Batch::default();
        for (i, update) in updates.into_iter().enumerate() {
            let at = format!("updates[{i}]");
            let id = node_id(&update.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            let patch = NodePatch::new(update.set.into_iter().collect()).map_err(|e| {
                ToolError::invalid(format!("{at}.set"), super::batch::sentence(&e.to_string()))
                    .at_op(i)
            })?;
            batch.push(
                Op::Update { id, patch },
                Label::new(i, at.clone()).rename("patch", format!("{at}.set")),
            );
        }
        let done = ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "changed": done.applied.changes.updated.len()}))
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Placement {
    /// The layer.
    id: String,
    /// The page or layer it goes into, by id.
    parent: String,
    /// Where among the parent's children, back to front. Defaults to the top (the end).
    index: Option<u32>,
}

struct CopyNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CopyInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// What to copy and where each copy goes.
    copies: Vec<Placement>,
}

impl Tool for CopyNodes {
    fn name(&self) -> &'static str {
        "copy_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Copy layers, with everything inside them, to a page or layer. Copies get new ids, returned in the order \
         of `copies`. One change for the whole batch."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<CopyInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Copied
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let CopyInput { document, copies } = parse_input(input)?;
        check_items(&copies, "copies")?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut batch = Batch::default();
        for (i, copy) in copies.iter().enumerate() {
            let at = format!("copies[{i}]");
            let id = node_id(&copy.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            let to = place(
                &doc,
                &copy.parent,
                copy.index,
                None,
                &format!("{at}.parent"),
            )
            .map_err(|e| e.at_op(i))?;
            batch.push(
                Op::Copy { id, to },
                Label::new(i, at.clone())
                    .rename("to.parent", format!("{at}.parent"))
                    .rename("to.index", format!("{at}.index")),
            );
        }
        let done = ctx.commit(&path, batch)?;
        let created: Vec<&str> = (0..copies.len())
            .filter_map(|i| done.applied.created.get(&i).map(|id| id.as_str()))
            .collect();
        Ok(json!({"document": path, "created": created}))
    }
}

struct ReplaceNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Replacement {
    /// The layer to replace.
    id: String,
    /// What goes in its place, in the file format. It may keep the old id.
    node: serde_json::Map<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReplaceInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The layers and what replaces each.
    replacements: Vec<Replacement>,
}

impl Tool for ReplaceNodes {
    fn name(&self) -> &'static str {
        "replace_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Remove layers and put new ones in exactly their place in the layer order. Use it to swap a layer for a \
         different kind of layer; to change properties use update_nodes. Returns the new ids."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ReplaceInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Replaced
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ReplaceInput {
            document,
            replacements,
        } = parse_input(input)?;
        check_items(&replacements, "replacements")?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut taken = ids_in_use(&doc);
        let mut batch = Batch::default();
        let mut created = Vec::new();
        for (i, replacement) in replacements.into_iter().enumerate() {
            let at = format!("replacements[{i}]");
            let id = node_id(&replacement.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            let node = new_node(
                Value::Object(replacement.node),
                &doc,
                &mut ctx.docs().tool_ids(),
                &mut taken,
                &format!("{at}.node"),
            )
            .map_err(|e| e.at_op(i))?;
            created.push(node.id().as_str().to_owned());
            batch.push(
                Op::Replace {
                    id,
                    node: Box::new(node),
                },
                Label::new(i, at.clone()).rename("node", format!("{at}.node")),
            );
        }
        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "created": created}))
    }
}

struct MoveNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MoveInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// What moves where, in order. An index counts the parent's children after the layer has left its old place.
    moves: Vec<Placement>,
}

impl Tool for MoveNodes {
    fn name(&self) -> &'static str {
        "move_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Move layers to another parent or page, or to another place among their siblings. A layer cannot move \
         into itself or between a page and a component. One change for the whole batch."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<MoveInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Moved
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let MoveInput { document, moves } = parse_input(input)?;
        check_items(&moves, "moves")?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut batch = Batch::default();
        for (i, step) in moves.iter().enumerate() {
            let at = format!("moves[{i}]");
            let id = node_id(&step.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            let to = place(
                &doc,
                &step.parent,
                step.index,
                Some(&step.id),
                &format!("{at}.parent"),
            )
            .map_err(|e| e.at_op(i))?;
            batch.push(
                Op::Move { id, to },
                Label::new(i, at.clone())
                    .rename("to.parent", format!("{at}.parent"))
                    .rename("to.index", format!("{at}.index")),
            );
        }
        let done = ctx.commit(&path, batch)?;
        Ok(
            json!({"document": path, "moved": moves.len(), "changed": done.applied.changes.updated.len()}),
        )
    }
}

struct DeleteNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DeleteInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The layers to remove, each with everything inside it.
    ids: Vec<String>,
}

impl Tool for DeleteNodes {
    fn name(&self) -> &'static str {
        "delete_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Remove layers and everything inside them, as one change. The person can undo it; still, read what you \
         are removing first."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<DeleteInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Deleted
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let DeleteInput { document, ids } = parse_input(input)?;
        check_items(&ids, "ids")?;
        let (path, _) = ctx.document(document.as_deref())?;
        let mut batch = Batch::default();
        for (i, id) in ids.iter().enumerate() {
            let at = format!("ids[{i}]");
            let id = node_id(id, &at).map_err(|e| e.at_op(i))?;
            batch.push(
                Op::Delete { id },
                Label::new(i, at.clone()).rename("id", at),
            );
        }
        let done = ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "removed": done.applied.changes.removed.len()}))
    }
}
