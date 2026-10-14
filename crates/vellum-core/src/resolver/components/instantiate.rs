//! The shared, read-only instance recipe used by resolution and the op layer.
use std::collections::BTreeMap;
use vellum_types::{
    Component, InstanceNode, Node, NodeId, NodeIdPath, NodePatch, Override, PropDef, Scalar,
};

/// A definition or instance cannot be expanded as requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentError {
    /// A property, prop name or definition path.
    pub at: String,
    /// What failed. Includes format-reader diagnostics for invalid property values.
    pub message: String,
}
impl std::fmt::Display for ComponentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.at, self.message)
    }
}
impl std::error::Error for ComponentError {}
pub(super) fn error(at: impl ToString, message: impl ToString) -> ComponentError {
    ComponentError {
        at: at.to_string(),
        message: message.to_string(),
    }
}

pub(crate) fn target<'a>(
    root: &'a mut Node,
    path: &NodeIdPath,
) -> Result<&'a mut Node, ComponentError> {
    let mut parts = path.segments();
    if parts.next() != Some(root.id()) {
        return Err(error(path, "path must start at the definition root"));
    }
    let mut node = root;
    for id in parts {
        node = node
            .children_mut()
            .and_then(|c| c.iter_mut().find(|n| n.id() == id))
            .ok_or_else(|| error(path, "no node at this path"))?;
    }
    Ok(node)
}
pub(crate) fn set_id(node: &mut Node, id: &NodeId) {
    let mut v = serde_json::to_value(&*node).expect("node");
    v["id"] = serde_json::json!(id);
    *node = serde_json::from_value(v).expect("valid id");
}
pub(crate) fn apply_override(
    root: &mut Node,
    path: &NodeIdPath,
    value: &Override,
) -> Result<(), ComponentError> {
    let parts: Vec<_> = path.segments().collect();
    if parts[0] != root.id() {
        return Err(error(path, "path must start at the definition root"));
    }
    let mut node = root;
    for (index, id) in parts.iter().enumerate().skip(1) {
        if let Node::Instance(instance) = node {
            let rest: NodeIdPath = parts[index..]
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join("/")
                .parse()
                .expect("id path");
            match (instance.overrides.get_mut(&rest), value) {
                (Some(Override::Props(previous)), Override::Props(later)) => {
                    previous.overlay(later)
                }
                _ => {
                    instance.overrides.insert(rest, value.clone());
                }
            }
            return Ok(());
        }
        node = node
            .children_mut()
            .and_then(|children| children.iter_mut().find(|n| n.id() == *id))
            .ok_or_else(|| error(path, "no node at this path"))?;
    }
    match value {
        Override::Props(p) => *node = node.patched(p).map_err(|e| error(path, e))?,
        Override::Replace(replacement) => {
            let id = node.id().clone();
            *node = (**replacement).clone();
            set_id(node, &id);
        }
        Override::Children(children) => {
            *node
                .children_mut()
                .ok_or_else(|| error(path, "node cannot hold children"))? = children.clone()
        }
    }
    Ok(())
}
fn prop_value(p: &PropDef) -> (Scalar, &[vellum_types::PropBinding]) {
    match p {
        PropDef::Boolean(p) => (Scalar::Bool(p.default), &p.bindings),
        PropDef::Text(p) => (Scalar::Text(p.default.clone()), &p.bindings),
        PropDef::Number(p) => (Scalar::Number(p.default), &p.bindings),
    }
}
/// Applies default and chosen props, then variant overrides, instance overrides, slots and sparse common
/// fields. It leaves nested instances linked. No stored document is changed.
pub fn instantiate(component: &Component, instance: &InstanceNode) -> Result<Node, ComponentError> {
    let mut root = component.root.clone();
    for name in instance.props.keys() {
        if !component.props.contains_key(name) {
            return Err(error(format!("props.{name}"), "undeclared prop"));
        }
    }
    let mut driven = BTreeMap::new();
    for (name, definition) in &component.props {
        let (default, bindings) = prop_value(definition);
        let value = instance.props.get(name).unwrap_or(&default);
        if std::mem::discriminant(value) != std::mem::discriminant(&default) {
            return Err(error(
                format!("props.{name}"),
                "value has the wrong prop type",
            ));
        }
        for binding in bindings {
            if driven
                .insert((binding.node.clone(), binding.property.clone()), name)
                .is_some()
            {
                return Err(error(
                    format!("props.{name}"),
                    "two props drive the same property",
                ));
            }
            let node = target(&mut root, &binding.node)?;
            if binding.property == "content"
                && let Scalar::Text(text) = value
                && let Node::Text(t) = node
            {
                let Some(mut run) = t.content.first().cloned() else {
                    return Err(error(
                        format!("props.{name}"),
                        "text binding requires a styled run",
                    ));
                };
                run.text = text.clone();
                t.content = vec![run];
                continue;
            }
            let patch = NodePatch::new(BTreeMap::from([(
                binding.property.clone(),
                serde_json::to_value(value).expect("scalar"),
            )]))
            .map_err(|e| error(format!("props.{name}"), e))?;
            *node = node
                .patched(&patch)
                .map_err(|e| error(format!("props.{name}"), e))?;
        }
    }
    if let Some(variant) = component
        .variants
        .iter()
        .find(|v| v.properties == instance.variant)
    {
        for (path, value) in &variant.overrides {
            apply_override(&mut root, path, value)?;
        }
    } else if !instance.variant.is_empty() {
        return Err(error("variant", "no matching component variant"));
    }
    for (path, value) in &instance.overrides {
        apply_override(&mut root, path, value)?;
    }
    for (id, children) in &instance.slot_content {
        fn populate(node: &mut Node, id: &NodeId, children: &[Node]) -> bool {
            if node.id() == id {
                if let Node::Frame(f) = node
                    && f.slot.is_some()
                {
                    f.children = children.to_vec();
                    return true;
                }
                return false;
            }
            node.children_mut()
                .is_some_and(|nodes| nodes.iter_mut().any(|n| populate(n, id, children)))
        }
        if !populate(&mut root, id, children) {
            return Err(error(
                format!("slotContent.{id}"),
                "not a slot in this instance",
            ));
        }
    }
    root = root
        .patched(&Node::Instance(instance.clone()).common_sparse_patch())
        .map_err(|e| error("instance", e))?;
    Ok(root)
}
