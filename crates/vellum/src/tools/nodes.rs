//! Reading nodes: `get_nodes`, `query_nodes`, `find_properties` (ARCHITECTURE §7.3).
//!
//! Every answer is bounded. When something is left out the answer says so (`truncated`, `childCount`), because
//! an agent that is not told a list is partial will treat it as whole.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_types::{Document, Node, NodeKind, referenced_variables};

use super::support::{Index, node_text, parse_input, schema_of};
use super::{Tool, ToolCtx, ToolError};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(GetNodes));
    tools.push(Box::new(QueryNodes));
    tools.push(Box::new(FindProperties));
}

/// How many nodes `get_nodes` returns in full unless asked for more.
const DEFAULT_MAX_NODES: usize = 300;
/// The most it will return in full.
const MAX_NODES: usize = 2_000;
/// The deepest `get_nodes` goes.
const MAX_DEPTH: usize = 8;
/// How many node ids `find_properties` lists for each value.
const EXAMPLES: usize = 5;

struct GetNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GetInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The ids of the nodes to read. Page ids are not nodes; use query_nodes with `scope` to list a page.
    ids: Vec<String>,
    /// How many levels of children to include: 0 is the node alone with a `childCount`, 1 (the default) adds
    /// its children. At most 8.
    depth: Option<u32>,
    /// Stop after this many nodes in all (default 300, at most 2000). Nodes past it come back as stubs with
    /// `truncated: true`.
    max_nodes: Option<u32>,
}

struct Budget {
    left: usize,
    cut: bool,
}

fn node_json(node: &Node, depth: usize, budget: &mut Budget) -> Value {
    let children = node.children();
    if budget.left == 0 {
        budget.cut = true;
        let mut stub = json!({
            "id": node.id().as_str(),
            "name": node.name(),
            "type": node.kind().as_str(),
            "truncated": true,
        });
        if !children.is_empty() {
            stub["childCount"] = json!(children.len());
        }
        return stub;
    }
    budget.left -= 1;
    let mut out = serde_json::to_value(node.without_children()).expect("a node serialises");
    if !children.is_empty() {
        if depth == 0 {
            out["childCount"] = json!(children.len());
        } else {
            out["children"] = Value::Array(
                children
                    .iter()
                    .map(|child| node_json(child, depth - 1, budget))
                    .collect(),
            );
        }
    }
    out
}

impl Tool for GetNodes {
    fn name(&self) -> &'static str {
        "get_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Read nodes by id, with their children down to `depth`. Use it to look at a frame before changing it. \
         Children past the depth are counted in `childCount`; nodes past `max_nodes` come back as stubs."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<GetInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let GetInput {
            document,
            ids,
            depth,
            max_nodes,
        } = parse_input(input)?;
        if ids.is_empty() {
            return Err(ToolError::invalid("ids", "Give at least one node id."));
        }
        let depth = depth.map_or(1, |d| d as usize);
        if depth > MAX_DEPTH {
            return Err(ToolError::invalid(
                "depth",
                format!("The depth is at most {MAX_DEPTH}."),
            ));
        }
        let max_nodes = max_nodes.map_or(DEFAULT_MAX_NODES, |n| n as usize);
        if !(1..=MAX_NODES).contains(&max_nodes) {
            return Err(ToolError::invalid(
                "max_nodes",
                format!("max_nodes is between 1 and {MAX_NODES}."),
            ));
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let index = Index::build(&doc);
        let mut budget = Budget {
            left: max_nodes,
            cut: false,
        };
        let (mut nodes, mut missing, mut seen) = (Vec::new(), Vec::new(), Vec::new());
        for id in &ids {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            match index.get(id) {
                None => missing.push(id.clone()),
                Some(entry) => {
                    let mut out = node_json(entry.node, depth, &mut budget);
                    if let Some(page) = entry.page {
                        out["page"] = json!(page.id.as_str());
                    }
                    if let Some((component, _)) = entry.component {
                        out["component"] = json!(component.as_str());
                    }
                    if let Some(parent) = entry.parent {
                        out["parent"] = json!(parent.as_str());
                    }
                    nodes.push(out);
                }
            }
        }
        Ok(json!({"document": path, "nodes": nodes, "missing": missing, "truncated": budget.cut}))
    }
}

struct QueryNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct QueryInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// Only look inside this page, component or node.
    scope: Option<String>,
    /// Only nodes of this type.
    r#type: Option<NodeKind>,
    /// Only nodes whose name contains this, ignoring case.
    name: Option<String>,
    /// Only text, notes and prompts whose characters contain this, ignoring case.
    text: Option<String>,
    /// Only instances of this component, by component id.
    component: Option<String>,
    /// Only nodes that use this variable, by variable id or exact name.
    variable: Option<String>,
    /// The most nodes to list (default 50, at most 500). `total` is the full count.
    limit: Option<u32>,
}

fn variable_id(doc: &Document, wanted: &str) -> Result<vellum_types::VariableId, ToolError> {
    doc.variables
        .iter()
        .find(|(id, variable)| id.as_str() == wanted || variable.name() == wanted)
        .map(|(id, _)| id.clone())
        .ok_or_else(|| {
            ToolError::not_found(format!(
                "The document has no variable with the id or name {wanted:?}. get_variables lists them."
            ))
        })
}

impl Tool for QueryNodes {
    fn name(&self) -> &'static str {
        "query_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Find nodes by name, type, text, component or variable use, optionally inside one page or node. Returns \
         short summaries in reading order; use get_nodes for the detail. All filters given must match."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<QueryInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let query: QueryInput = parse_input(input)?;
        let limit = query.limit.map_or(50, |n| n as usize);
        if !(1..=500).contains(&limit) {
            return Err(ToolError::invalid("limit", "limit is between 1 and 500."));
        }
        let (path, doc) = ctx.document(query.document.as_deref())?;
        let index = Index::build(&doc);
        let range = match &query.scope {
            None => 0..index.entries.len(),
            Some(scope) => index.scope(scope).ok_or_else(|| {
                ToolError::not_found(format!("There is no page, component or node {scope:?}."))
            })?,
        };
        let variable = query
            .variable
            .as_deref()
            .map(|v| variable_id(&doc, v))
            .transpose()?;
        let name = query.name.as_deref().map(str::to_lowercase);
        let text = query.text.as_deref().map(str::to_lowercase);

        let mut nodes = Vec::new();
        let mut total = 0;
        for at in range {
            let node = index.entries[at].node;
            let hit = query.r#type.is_none_or(|kind| node.kind() == kind)
                && name
                    .as_deref()
                    .is_none_or(|n| node.name().to_lowercase().contains(n))
                && text.as_deref().is_none_or(|t| {
                    node_text(node).is_some_and(|content| content.to_lowercase().contains(t))
                })
                && query.component.as_deref().is_none_or(
                    |c| matches!(node, Node::Instance(i) if i.component_id.as_str() == c),
                )
                && variable
                    .as_ref()
                    .is_none_or(|v| referenced_variables(&node.without_children()).contains(v));
            if hit {
                total += 1;
                if nodes.len() < limit {
                    nodes.push(index.summary(at));
                }
            }
        }
        Ok(json!({"document": path, "nodes": nodes, "total": total, "truncated": total > limit}))
    }
}

struct FindProperties;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FindInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The property, in the file's spelling, with dots into nested objects. Lists are looked through, so
    /// `content.size` is every text run's size and `fills.color` is every fill colour. Examples: `cornerRadius`,
    /// `gap`, `content.font`, `stroke.width`.
    property: String,
    /// Only look inside this page, component or node.
    scope: Option<String>,
    /// Only nodes of this type.
    r#type: Option<NodeKind>,
    /// The most distinct values to list (default 100, at most 500).
    limit: Option<u32>,
}

fn collect(value: &Value, path: &[&str], out: &mut Vec<Value>) {
    match path.split_first() {
        None => out.push(value.clone()),
        Some((head, rest)) => match value {
            Value::Object(map) => {
                if let Some(next) = map.get(*head) {
                    collect(next, rest, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| collect(item, path, out)),
            _ => {}
        },
    }
}

fn order(a: &Value, b: &Value) -> std::cmp::Ordering {
    let rank = |v: &Value| match v {
        Value::Number(_) => 0,
        Value::String(_) => 1,
        Value::Bool(_) => 2,
        _ => 3,
    };
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x
            .as_f64()
            .partial_cmp(&y.as_f64())
            .unwrap_or(std::cmp::Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        _ => rank(a)
            .cmp(&rank(b))
            .then_with(|| a.to_string().cmp(&b.to_string())),
    }
}

impl Tool for FindProperties {
    fn name(&self) -> &'static str {
        "find_properties"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "List every distinct value a property takes in a scope, with how many nodes have each and a few example \
         ids. Use it to see what a design already uses (font sizes, colours, radii) before adding more, or to \
         find values that should be a variable. Variable references show as {\"$var\": id}."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<FindInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let find: FindInput = parse_input(input)?;
        let limit = find.limit.map_or(100, |n| n as usize);
        if !(1..=500).contains(&limit) {
            return Err(ToolError::invalid("limit", "limit is between 1 and 500."));
        }
        let steps: Vec<&str> = find.property.split('.').collect();
        if steps.iter().any(|step| step.is_empty()) {
            return Err(ToolError::invalid(
                "property",
                "Name a property such as `cornerRadius`, or a path such as `content.size`.",
            ));
        }
        let (path, doc) = ctx.document(find.document.as_deref())?;
        let index = Index::build(&doc);
        let range = match &find.scope {
            None => 0..index.entries.len(),
            Some(scope) => index.scope(scope).ok_or_else(|| {
                ToolError::not_found(format!("There is no page, component or node {scope:?}."))
            })?,
        };

        let (mut checked, mut with_property) = (0, 0);
        // value as text → (value, nodes that have it)
        let mut found: BTreeMap<String, (Value, Vec<String>)> = BTreeMap::new();
        for at in range {
            let node = index.entries[at].node;
            if find.r#type.is_some_and(|kind| node.kind() != kind) {
                continue;
            }
            checked += 1;
            let json = serde_json::to_value(node.without_children()).expect("a node serialises");
            let mut values = Vec::new();
            collect(&json, &steps, &mut values);
            if values.is_empty() {
                continue;
            }
            with_property += 1;
            for value in values {
                let nodes = &mut found
                    .entry(value.to_string())
                    .or_insert_with(|| (value, Vec::new()))
                    .1;
                let id = node.id().as_str();
                if nodes.last().map(String::as_str) != Some(id) {
                    nodes.push(id.to_owned());
                }
            }
        }
        let mut values: Vec<(Value, Vec<String>)> = found.into_values().collect();
        values.sort_by(|a, b| order(&a.0, &b.0));
        let truncated = values.len() > limit;
        let values: Vec<Value> = values
            .into_iter()
            .take(limit)
            .map(|(value, nodes)| {
                json!({
                    "value": value,
                    "count": nodes.len(),
                    "nodes": nodes.iter().take(EXAMPLES).collect::<Vec<_>>(),
                })
            })
            .collect();
        Ok(json!({
            "document": path,
            "property": find.property,
            "values": values,
            "nodesWithProperty": with_property,
            "nodesChecked": checked,
            "truncated": truncated,
        }))
    }
}
