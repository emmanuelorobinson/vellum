//! Component definitions and instances (ADR-034): what the editor's component menu does, as batches. Definitions
//! change their name, props and variants here; their layers are changed with `replace_nodes` on the definition.
//! Instances are detached, turned into components, or given an override.

use std::collections::BTreeMap;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use vellum_core::ops::Op;
use vellum_types::{ComponentId, NodeId, NodeIdPath, PropDef, Variant};

use super::batch::{Batch, Label, ids_in_use, new_node, sentence};
use super::support::{parse_input, schema_of};
use super::write::MAX_ITEMS;
use super::{Tool, ToolCtx, ToolError, Verb};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(ManageComponents));
    tools.push(Box::new(InstanceNodes));
}

// ---- manage_components ------------------------------------------------------------------------------------------

struct ManageComponents;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ComponentsInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// Definitions to change. What is left out stays as it is. The layers of a definition are not changed here.
    update: Option<Vec<ComponentChange>>,
    /// Definitions to remove. One that an instance still uses is refused: detach those instances first.
    remove: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ComponentChange {
    /// The component.
    id: String,
    /// Its new name.
    name: Option<String>,
    /// Its props, replacing all of them: each name maps to its definition, in the file format (see read_skill).
    props: Option<Map<String, Value>>,
    /// Its variants, replacing all of them, in the file format (see read_skill).
    variants: Option<Vec<Map<String, Value>>>,
}

impl Tool for ManageComponents {
    fn name(&self) -> &'static str {
        "manage_components"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Rename a component, replace its props or variants, or remove a component that no instance uses. The \
         layers inside a component are not changed here. One change for the batch."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ComponentsInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::MadeComponent
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ComponentsInput {
            document,
            update,
            remove,
        } = parse_input(input)?;
        let (update, remove) = (update.unwrap_or_default(), remove.unwrap_or_default());
        let (updated, removed) = (update.len(), remove.len());
        let total = updated + removed;
        if total == 0 {
            return Err(ToolError::invalid(
                "update",
                "Give at least one change: update or remove.",
            ));
        }
        if total > MAX_ITEMS {
            return Err(ToolError::invalid(
                "update",
                format!("At most {MAX_ITEMS} changes in one call; split the batch."),
            ));
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut batch = Batch::default();
        for (i, change) in update.into_iter().enumerate() {
            let at = format!("update[{i}]");
            let id = component_id(&change.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            let current = doc.components.get(&id).ok_or_else(|| {
                ToolError::not_found(format!(
                    "There is no component {:?} in this document.",
                    change.id
                ))
                .at_op(i)
                .at_path(format!("{at}.id"))
            })?;
            let props = match change.props {
                Some(props) => read_props(props, &at).map_err(|e| e.at_op(i))?,
                None => current.props.clone(),
            };
            let variants = match change.variants {
                Some(variants) => read_variants(variants, &at).map_err(|e| e.at_op(i))?,
                None => current.variants.clone(),
            };
            let patch = Op::UpdateComponent {
                id,
                name: change.name.unwrap_or_else(|| current.name.clone()),
                props,
                variants,
            };
            batch.push(patch, Label::new(i, at));
        }
        for (i, id) in remove.iter().enumerate() {
            let at = format!("remove[{i}]");
            let id = component_id(id, &at).map_err(|e| e.at_op(i))?;
            batch.push(
                Op::SetComponent {
                    id,
                    component: None,
                },
                Label::new(i, at.clone()).rename("id", at),
            );
        }
        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "updated": updated, "removed": removed}))
    }
}

// ---- instance_nodes ---------------------------------------------------------------------------------------------

struct InstanceNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InstancesInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// Instances to expand into their own layers. They then no longer follow their component.
    detach: Option<Vec<String>>,
    /// Layers to make into components, or instances to turn back into layers. A layer needs `componentId`, the id
    /// the new component takes. An instance keeps its own component's id; when it is that component's only
    /// instance, the component is removed.
    toggle: Option<Vec<ToggleItem>>,
    /// Layers inside an instance to replace with an override. The component itself is not changed.
    replace: Option<Vec<ReplaceItem>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ToggleItem {
    /// The layer, or the instance.
    id: String,
    /// The id the new component takes. Required for a layer; an instance keeps its own.
    #[serde(rename = "componentId")]
    component_id: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReplaceItem {
    /// The instance whose layer is overridden.
    instance: String,
    /// The layer inside the component, by id from the component's root, joined with "/": for example
    /// `buttonRoot/buttonText`.
    path: String,
    /// The new layer, in the file format. Its `id` and `name` may be left out.
    node: Map<String, Value>,
}

impl Tool for InstanceNodes {
    fn name(&self) -> &'static str {
        "instance_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Detach instances, make a layer into a component, turn an instance back into layers, or override a layer \
         inside an instance. The component itself is changed only by manage_components. One change for the batch."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<InstancesInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::MadeComponent
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let InstancesInput {
            document,
            detach,
            toggle,
            replace,
        } = parse_input(input)?;
        let (detach, toggle, replace) = (
            detach.unwrap_or_default(),
            toggle.unwrap_or_default(),
            replace.unwrap_or_default(),
        );
        let total = detach.len() + toggle.len() + replace.len();
        if total == 0 {
            return Err(ToolError::invalid(
                "detach",
                "Give at least one change: detach, toggle or replace.",
            ));
        }
        if total > MAX_ITEMS {
            return Err(ToolError::invalid(
                "detach",
                format!("At most {MAX_ITEMS} changes in one call; split the batch."),
            ));
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut ids = ctx.docs().tool_ids();
        let mut taken = ids_in_use(&doc);
        let mut batch = Batch::default();
        let mut components = Vec::new();

        for (i, id) in detach.iter().enumerate() {
            let at = format!("detach[{i}]");
            let id = node_id(id, &at).map_err(|e| e.at_op(i))?;
            batch.push(
                Op::Detach { id },
                Label::new(i, at.clone()).rename("id", at),
            );
        }
        for (i, item) in toggle.into_iter().enumerate() {
            let at = format!("toggle[{i}]");
            let id = node_id(&item.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            let layer = vellum_core::ops::find(&doc, &id).ok_or_else(|| {
                ToolError::not_found(format!("There is no layer {:?} in this document.", item.id))
                    .at_op(i)
                    .at_path(format!("{at}.id"))
            })?;
            // An instance keeps its own component's id; a layer needs one from the agent.
            let json = serde_json::to_value(layer).unwrap_or(Value::Null);
            let own = (json["type"] == "instance")
                .then(|| json["componentId"].as_str().map(str::to_owned))
                .flatten();
            let given = match (item.component_id, own) {
                (Some(given), _) => given,
                (None, Some(own)) => own,
                (None, None) => {
                    return Err(ToolError::invalid(
                        format!("{at}.componentId"),
                        "A layer that is not an instance needs a componentId: the id the new component takes.",
                    )
                    .at_op(i));
                }
            };
            let target =
                component_id(&given, &format!("{at}.componentId")).map_err(|e| e.at_op(i))?;
            components.push(target.as_str().to_owned());
            batch.push(
                Op::ToggleComponent {
                    id,
                    component_id: target,
                },
                Label::new(i, at.clone()).rename("id", at),
            );
        }
        for (i, item) in replace.into_iter().enumerate() {
            let at = format!("replace[{i}]");
            let instance =
                node_id(&item.instance, &format!("{at}.instance")).map_err(|e| e.at_op(i))?;
            let path_ids = NodeIdPath::from_str(&item.path).map_err(|e| {
                ToolError::invalid(
                    format!("{at}.path"),
                    format!(
                        "{:?} is not a path of layer ids joined with \"/\": {e}.",
                        item.path
                    ),
                )
                .at_op(i)
            })?;
            let node = new_node(
                Value::Object(item.node),
                &doc,
                &mut ids,
                &mut taken,
                &format!("{at}.node"),
            )
            .map_err(|e| e.at_op(i))?;
            batch.push(
                Op::ReplaceInInstance {
                    id: instance,
                    path: path_ids,
                    node: Box::new(node),
                },
                Label::new(i, at.clone())
                    .rename("node", format!("{at}.node"))
                    .rename("path", format!("{at}.path")),
            );
        }
        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "changed": total, "components": components}))
    }
}

/// A component's props as the file format reads them, at input path `at`.
fn read_props(props: Map<String, Value>, at: &str) -> Result<BTreeMap<String, PropDef>, ToolError> {
    serde_json::from_value(Value::Object(props))
        .map_err(|e| ToolError::invalid(format!("{at}.props"), sentence(&e.to_string())))
}

/// A component's variants as the file format reads them, at input path `at`.
fn read_variants(variants: Vec<Map<String, Value>>, at: &str) -> Result<Vec<Variant>, ToolError> {
    serde_json::from_value(Value::Array(
        variants.into_iter().map(Value::Object).collect(),
    ))
    .map_err(|e| ToolError::invalid(format!("{at}.variants"), sentence(&e.to_string())))
}

/// A node id the agent gave, checked, at input path `at`.
fn node_id(id: &str, at: &str) -> Result<NodeId, ToolError> {
    NodeId::new(id).map_err(|e| ToolError::invalid(at, format!("{id:?} is not a layer id: {e}.")))
}

/// A component id the agent gave, checked, at input path `at`.
fn component_id(id: &str, at: &str) -> Result<ComponentId, ToolError> {
    ComponentId::new(id)
        .map_err(|e| ToolError::invalid(at, format!("{id:?} is not a component id: {e}.")))
}
