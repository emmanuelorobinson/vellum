//! Writing the design system: `replace_properties`, `set_variables`, `create_component`, `create_instance`,
//! `make_slot` (ARCHITECTURE §7.3).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use vellum_core::ops::Op;
use vellum_types::{ComponentId, Document, NodeKind, NodePatch, Variable, VariableId};

use super::batch::{Batch, Label, fresh, ids_in_use, new_node, node_id, place, sentence};
use super::support::{Index, parse_input, schema_of};
use super::write::check_items;
use super::{Tool, ToolCtx, ToolError, Verb};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(ReplaceProperties));
    tools.push(Box::new(SetVariables));
    tools.push(Box::new(CreateComponent));
    tools.push(Box::new(CreateInstance));
    tools.push(Box::new(MakeSlot));
}

/// A variable named by id or by its exact name, as `{"$var": id}`.
fn variable_ref(doc: &Document, wanted: &str, at: &str) -> Result<Value, ToolError> {
    doc.variables
        .iter()
        .find(|(id, v)| id.as_str() == wanted || v.name() == wanted)
        .map(|(id, _)| json!({"$var": id.as_str()}))
        .ok_or_else(|| {
            ToolError::invalid(
                at,
                format!("There is no variable {wanted:?}. get_variables lists them."),
            )
        })
}

/// `#RGB` as `#RRGGBB`, so a short colour matches what the file writes.
fn long_colour(text: &str) -> Option<String> {
    let digits = text.strip_prefix('#')?;
    if digits.len() == 3 && digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        Some(
            format!(
                "#{}",
                digits.chars().flat_map(|c| [c, c]).collect::<String>()
            )
            .to_uppercase(),
        )
    } else {
        None
    }
}

/// A value the agent wrote, as the file writes it: `$name` is a variable, `#abc` is `#AABBCC`.
fn file_value(doc: &Document, value: Value, at: &str) -> Result<Value, ToolError> {
    match &value {
        Value::String(text) if text.starts_with('$') && text.len() > 1 => {
            variable_ref(doc, &text[1..], at)
        }
        Value::String(text) => Ok(long_colour(text).map_or(value, Value::String)),
        _ => Ok(value),
    }
}

/// Whether a value in the file is the one being looked for. Colours compare ignoring case and an opaque alpha;
/// numbers compare as numbers.
fn same(found: &Value, wanted: &Value) -> bool {
    match (found, wanted) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::String(a), Value::String(b)) if a.starts_with('#') && b.starts_with('#') => {
            let norm = |s: &str| {
                let s = s.to_uppercase();
                s.strip_suffix("FF")
                    .filter(|t| t.len() == 7)
                    .map_or(s.clone(), str::to_owned)
            };
            norm(a) == norm(b)
        }
        _ => found == wanted,
    }
}

/// Replaces every value at `path` (lists looked through) equal to `from` with `to`. Returns how many.
fn replace_at(value: &mut Value, path: &[&str], from: &Value, to: &Value) -> usize {
    match path.split_first() {
        None => {
            if same(value, from) {
                *value = to.clone();
                1
            } else {
                0
            }
        }
        Some((head, rest)) => match value {
            Value::Object(map) => map
                .get_mut(*head)
                .map_or(0, |next| replace_at(next, rest, from, to)),
            Value::Array(items) => items
                .iter_mut()
                .map(|item| replace_at(item, path, from, to))
                .sum(),
            _ => 0,
        },
    }
}

struct ReplaceProperties;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReplacePropertiesInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The property, in the file's spelling, dotted into nested objects; lists are looked through. Examples:
    /// `cornerRadius`, `content.size`, `fills.color`, `stroke.width`.
    property: String,
    /// The value to find. `#abc` matches `#AABBCC`; `$color/text` matches a use of that variable.
    from: Value,
    /// What to put instead. `$color/text` binds the variable of that name (or id).
    to: Value,
    /// Only inside this page, component or layer.
    scope: Option<String>,
    /// Only layers of this type.
    r#type: Option<NodeKind>,
}

impl Tool for ReplaceProperties {
    fn name(&self) -> &'static str {
        "replace_properties"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Change every use of one value of a property to another, in a scope, as one change: every #111111 text \
         colour to $color/text, every 16 font size to 18. Use find_properties first to see the values in use. \
         Reports how many layers changed; none is not an error."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ReplacePropertiesInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Updated
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let find: ReplacePropertiesInput = parse_input(input)?;
        let steps: Vec<&str> = find.property.split('.').collect();
        if steps.iter().any(|s| s.is_empty()) {
            return Err(ToolError::invalid(
                "property",
                "Name a property such as `cornerRadius`, or a path such as `content.size`.",
            ));
        }
        if matches!(steps[0], "id" | "type" | "children") {
            return Err(ToolError::invalid(
                "property",
                format!("{} cannot be replaced this way.", steps[0]),
            ));
        }
        let (path, doc) = ctx.document(find.document.as_deref())?;
        let from = file_value(&doc, find.from, "from")?;
        let to = file_value(&doc, find.to, "to")?;
        let index = Index::build(&doc);
        let range = match &find.scope {
            None => 0..index.entries.len(),
            Some(scope) => index.scope(scope).ok_or_else(|| {
                ToolError::invalid(
                    "scope",
                    format!("There is no page, component or layer {scope:?}."),
                )
            })?,
        };
        let mut batch = Batch::default();
        let mut matched = 0;
        for at in range {
            let node = index.entries[at].node;
            if find.r#type.is_some_and(|kind| node.kind() != kind) {
                continue;
            }
            let mut json =
                serde_json::to_value(node.without_children()).expect("a node serialises");
            let hits = replace_at(&mut json, &steps, &from, &to);
            if hits == 0 {
                continue;
            }
            matched += hits;
            let top = json.get(steps[0]).cloned().unwrap_or(Value::Null);
            let patch = NodePatch::new(BTreeMap::from([(steps[0].to_owned(), top)]))
                .map_err(|e| ToolError::invalid("property", sentence(&e.to_string())))?;
            batch.push(
                Op::Update {
                    id: node.id().clone(),
                    patch,
                },
                Label::whole("to")
                    .rename("patch", "to")
                    .rename("id", "scope"),
            );
        }
        if batch.is_empty() {
            ctx.note_target(&doc, find.scope.as_deref().unwrap_or_default());
            return Ok(json!({"document": path, "changed": 0, "values": 0}));
        }
        let done = ctx.commit(&path, batch)?;
        Ok(
            json!({"document": path, "changed": done.applied.changes.updated.len(), "values": matched}),
        )
    }
}

struct SetVariables;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetVariablesInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// Variables to add or change, in the file format: `{"name": "color/text", "type": "color", "values":
    /// [{"value": "#111111"}, {"value": "#EEEEEE", "when": {"Mode": "dark"}}]}`. One with the `id` or the
    /// `name` of an existing variable replaces it; otherwise it is new.
    #[serde(default)]
    variables: Vec<Map<String, Value>>,
    /// Variables to remove, by id or name. A variable still in use cannot be removed.
    #[serde(default)]
    remove: Vec<String>,
}

impl Tool for SetVariables {
    fn name(&self) -> &'static str {
        "set_variables"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Add, change or remove variables (design tokens), as one change. A variable is matched by id or by name, \
         so sending `color/text` again changes it. Returns the id of each by name; bind one with {\"$var\": id}."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<SetVariablesInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::UpdatedVariables
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let SetVariablesInput {
            document,
            variables,
            remove,
        } = parse_input(input)?;
        if variables.is_empty() && remove.is_empty() {
            return Err(ToolError::invalid(
                "variables",
                "Give variables to set or names to remove.",
            ));
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut taken = ids_in_use(&doc);
        let mut set = BTreeMap::new();
        // id → where in the input it came from, to report errors there.
        let mut sources: BTreeMap<String, String> = BTreeMap::new();
        let mut ids = Map::new();
        for (i, mut body) in variables.into_iter().enumerate() {
            let at = format!("variables[{i}]");
            let name = body.get("name").and_then(Value::as_str).map(str::to_owned);
            let id = match body.remove("id") {
                Some(Value::String(id)) => id,
                Some(_) => {
                    return Err(ToolError::invalid(format!("{at}.id"), "An id is a string."));
                }
                None => match doc
                    .variables
                    .iter()
                    .find(|(_, v)| Some(v.name()) == name.as_deref())
                {
                    Some((id, _)) => id.as_str().to_owned(),
                    None => fresh(&mut ctx.docs().tool_ids(), &mut taken),
                },
            };
            let id = VariableId::new(&id)
                .map_err(|e| ToolError::invalid(format!("{at}.id"), format!("{id:?}: {e}.")))?;
            let variable: Variable = serde_path_to_error::deserialize(Value::Object(body))
                .map_err(|e| {
                    let inner = e.path().to_string();
                    let inner = if inner == "." { String::new() } else { inner };
                    ToolError::invalid(
                        super::batch::join(&at, &inner),
                        sentence(&e.inner().to_string()),
                    )
                })?;
            ids.insert(variable.name().to_owned(), json!(id.as_str()));
            sources.insert(id.as_str().to_owned(), at);
            set.insert(id, variable);
        }
        let mut gone = BTreeSet::new();
        for (i, wanted) in remove.iter().enumerate() {
            let at = format!("remove[{i}]");
            let id = doc
                .variables
                .iter()
                .find(|(id, v)| id.as_str() == wanted || v.name() == wanted)
                .map(|(id, _)| id.clone())
                .ok_or_else(|| {
                    ToolError::invalid(&at, format!("There is no variable {wanted:?}."))
                })?;
            sources.insert(id.as_str().to_owned(), at);
            gone.insert(id);
        }
        let mut label = Label::whole("variables");
        for (id, at) in &sources {
            label = label
                .rename(format!("set.{id}"), at.clone())
                .rename(format!("remove.{id}"), at.clone());
        }
        let mut batch = Batch::default();
        batch.push(Op::SetVariables { set, remove: gone }, label);
        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "ids": ids, "removed": remove.len()}))
    }
}

struct CreateComponent;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateComponentInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The layers to make into components. Each stays where it is, as the first instance of its component.
    ids: Vec<String>,
}

impl Tool for CreateComponent {
    fn name(&self) -> &'static str {
        "create_component"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Make layers into components: each layer's tree becomes a component definition and the layer becomes its \
         first instance. Returns the component ids in order; use create_instance to place more."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<CreateComponentInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::MadeComponent
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let CreateComponentInput { document, ids } = parse_input(input)?;
        check_items(&ids, "ids")?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut taken = ids_in_use(&doc);
        let mut batch = Batch::default();
        let mut components = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            let at = format!("ids[{i}]");
            let id = node_id(id, &at).map_err(|e| e.at_op(i))?;
            let component_id = ComponentId::new(fresh(&mut ctx.docs().tool_ids(), &mut taken))
                .expect("a node id is a component id");
            components.push(component_id.as_str().to_owned());
            batch.push(
                Op::CreateComponent { id, component_id },
                Label::new(i, at.clone()).rename("id", at),
            );
        }
        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "components": components}))
    }
}

struct CreateInstance;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InstanceSpec {
    /// The page or layer it goes into, by id.
    parent: String,
    /// The local component, by id (list_components shows them).
    component: String,
    /// Where among the parent's children, back to front. Defaults to the top (the end).
    index: Option<u32>,
    /// Instance properties in the file format: `name`, `position`, `width`, `props` (the component's props by
    /// name), `overrides`. Size and name default to the component's.
    #[serde(default)]
    set: Map<String, Value>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateInstanceInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The instances to place.
    instances: Vec<InstanceSpec>,
}

impl Tool for CreateInstance {
    fn name(&self) -> &'static str {
        "create_instance"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Place instances of local components, with their props and overrides. Each is sized like its component \
         unless `set` says otherwise. Returns the new ids in order, as one change."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<CreateInstanceInput>()
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
        let CreateInstanceInput {
            document,
            instances,
        } = parse_input(input)?;
        check_items(&instances, "instances")?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut taken = ids_in_use(&doc);
        let mut batch = Batch::default();
        let mut created = Vec::new();
        let mut placed: BTreeMap<String, usize> = BTreeMap::new();
        for (i, spec) in instances.into_iter().enumerate() {
            let at = format!("instances[{i}]");
            if !doc.components.keys().any(|c| c.as_str() == spec.component) {
                return Err(ToolError::invalid(
                    format!("{at}.component"),
                    format!(
                        "There is no local component {:?}. list_components lists them.",
                        spec.component
                    ),
                )
                .at_op(i));
            }
            for key in ["type", "componentId", "children"] {
                if spec.set.contains_key(key) {
                    return Err(ToolError::invalid(
                        format!("{at}.set.{key}"),
                        format!("`{key}` comes from `component`; leave it out of `set`."),
                    )
                    .at_op(i));
                }
            }
            let mut node = spec.set;
            node.insert("type".into(), json!("instance"));
            node.insert("componentId".into(), json!(spec.component));
            let node = new_node(
                Value::Object(node),
                &doc,
                &mut ctx.docs().tool_ids(),
                &mut taken,
                &format!("{at}.set"),
            )
            .map_err(|e| e.at_op(i))?;
            let mut to = place(
                &doc,
                &spec.parent,
                spec.index,
                None,
                &format!("{at}.parent"),
            )
            .map_err(|e| e.at_op(i))?;
            // Appends to one parent land after each other, in order.
            if spec.index.is_none() {
                let earlier = placed.entry(spec.parent.clone()).or_default();
                to.index += *earlier;
                *earlier += 1;
            }
            created.push(node.id().as_str().to_owned());
            batch.push(
                Op::Insert {
                    to,
                    node: Box::new(node),
                },
                Label::new(i, at.clone())
                    .rename("node", format!("{at}.set"))
                    .rename("to.parent", format!("{at}.parent"))
                    .rename("to.index", format!("{at}.index")),
            );
        }
        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "created": created}))
    }
}

struct MakeSlot;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SlotSpec {
    /// An empty frame inside a component definition (not its root).
    id: String,
    /// Components suggested for the slot, by id. Advisory: anything can fill it.
    #[serde(default)]
    suggested: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MakeSlotInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The frames to make into slots.
    slots: Vec<SlotSpec>,
}

impl Tool for MakeSlot {
    fn name(&self) -> &'static str {
        "make_slot"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Mark empty frames inside a component as slots, which each instance fills with its own content. Suggest \
         the components that usually go there. One change for the batch."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<MakeSlotInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::MadeSlot
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let MakeSlotInput { document, slots } = parse_input(input)?;
        check_items(&slots, "slots")?;
        let (path, _) = ctx.document(document.as_deref())?;
        let mut batch = Batch::default();
        for (i, slot) in slots.iter().enumerate() {
            let at = format!("slots[{i}]");
            let id = node_id(&slot.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            let suggested = slot
                .suggested
                .iter()
                .enumerate()
                .map(|(k, c)| {
                    ComponentId::new(c).map_err(|e| {
                        ToolError::invalid(format!("{at}.suggested[{k}]"), format!("{c:?}: {e}."))
                            .at_op(i)
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            batch.push(Op::MakeSlot { id, suggested }, Label::new(i, at.clone()));
        }
        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "slots": slots.len()}))
    }
}
