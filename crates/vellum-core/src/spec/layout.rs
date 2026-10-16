//! `layout-spec-v1`.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Number, Value, json};
use vellum_types::{ComponentId, Document, Node, NodeId, StyleId, VariableId, VariableType};

use crate::ports::{SpecError, SpecFormat};
use crate::tokens::{Token, TokenValue, collect};

/// The framework-neutral description of a layer and everything in it.
pub struct LayoutSpecV1;

/// A number as JSON would write it by hand: `320`, not `320.0`.
fn number(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 1e15 {
        Value::Number(Number::from(n as i64))
    } else {
        Number::from_f64(n).map_or(Value::Null, Value::Number)
    }
}

fn tidy(value: &mut Value) {
    match value {
        Value::Number(n) => {
            if let Some(f) = n.as_f64()
                && !n.is_i64()
                && !n.is_u64()
            {
                *value = number(f);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(tidy),
        Value::Object(map) => map.values_mut().for_each(tidy),
        _ => {}
    }
}

fn page_of(doc: &Document, id: &NodeId) -> Option<String> {
    fn contains(nodes: &[Node], id: &NodeId) -> bool {
        nodes
            .iter()
            .any(|n| n.id() == id || contains(n.children(), id))
    }
    doc.pages
        .iter()
        .find(|page| contains(&page.children, id))
        .map(|page| page.id.as_str().to_owned())
}

struct Walk<'a> {
    doc: &'a Document,
    vars: BTreeSet<VariableId>,
    styles: BTreeSet<StyleId>,
    /// Components used, and how many instances in the spec's own layers use each.
    components: BTreeMap<ComponentId, usize>,
    /// Library components: alias, id, instances.
    library: BTreeMap<(String, String), usize>,
    in_definition: bool,
}

impl Walk<'_> {
    /// Replaces every `{"$var": id}` by `{"$token": name}`, remembering the variables.
    fn rewrite(&mut self, value: &mut Value) {
        match value {
            Value::Object(map) => {
                if map.len() == 1
                    && let Some(Value::String(id)) = map.get("$var")
                {
                    let found = VariableId::new(id.as_str()).ok().and_then(|id| {
                        self.doc
                            .variables
                            .get(&id)
                            .map(|v| (id, v.name().to_owned()))
                    });
                    *value = match found {
                        Some((id, name)) => {
                            self.vars.insert(id);
                            json!({"$token": name})
                        }
                        None => json!({"$unresolved": id}),
                    };
                    return;
                }
                map.values_mut().for_each(|v| self.rewrite(v));
            }
            Value::Array(items) => items.iter_mut().for_each(|v| self.rewrite(v)),
            _ => {}
        }
    }

    fn nodes(&mut self, nodes: &[Node]) -> Vec<Value> {
        nodes.iter().map(|n| self.node(n)).collect()
    }

    fn node(&mut self, node: &Node) -> Value {
        let mut value = serde_json::to_value(node.without_children()).unwrap_or(Value::Null);
        let Some(map) = value.as_object_mut() else {
            return value;
        };
        map.remove("locked");
        if let Some(notes) = map.remove("agentNotes") {
            map.insert("notes".into(), notes);
        }
        let slot_content = map.remove("slotContent");
        self.rewrite(&mut value);
        let Some(map) = value.as_object_mut() else {
            return value;
        };

        match node {
            Node::Text(text) => {
                if let Some(style) = &text.text_style
                    && let Some(found) = self.doc.styles.get(style)
                {
                    self.styles.insert(style.clone());
                    let name = match found {
                        vellum_types::Style::Text(s) => &s.name,
                        vellum_types::Style::Effect(s) => &s.name,
                    };
                    map.insert("textStyleName".into(), json!(name));
                }
            }
            Node::Instance(instance) => {
                match &instance.library {
                    None => {
                        if let Some(component) = self.doc.components.get(&instance.component_id) {
                            map.insert("component".into(), json!(component.name));
                        }
                        let uses = self
                            .components
                            .entry(instance.component_id.clone())
                            .or_default();
                        if !self.in_definition {
                            *uses += 1;
                        }
                    }
                    Some(alias) => {
                        let uses = self
                            .library
                            .entry((alias.clone(), instance.component_id.as_str().to_owned()))
                            .or_default();
                        if !self.in_definition {
                            *uses += 1;
                        }
                    }
                }
                if let Some(Value::Object(slots)) = slot_content {
                    let mut out = Map::new();
                    for (slot, filled) in &instance.slot_content {
                        if slots.contains_key(slot.as_str()) {
                            out.insert(slot.as_str().to_owned(), Value::Array(self.nodes(filled)));
                        }
                    }
                    if !out.is_empty() {
                        map.insert("slotContent".into(), Value::Object(out));
                    }
                }
            }
            _ => {}
        }
        let children = node.children();
        if !children.is_empty() {
            let children = self.nodes(children);
            if let Some(map) = value.as_object_mut() {
                map.insert("children".into(), Value::Array(children));
            }
        }
        value
    }
}

fn token_value(value: &TokenValue) -> Value {
    match value {
        TokenValue::Color(c) => json!(c.to_string()),
        TokenValue::Number(n) => number(*n),
        TokenValue::String(s) => json!(s),
        TokenValue::Boolean(b) => json!(b),
        TokenValue::Alias(name) => json!({"$token": name}),
    }
}

fn token_entry(token: &Token) -> Value {
    let mut entry = json!({
        "name": token.name,
        "type": token.kind.as_str(),
        "value": token_value(&token.value),
    });
    if !token.modes.is_empty() {
        let modes: Map<String, Value> = token
            .modes
            .iter()
            .map(|m| (format!("{}: {}", m.axis, m.mode), token_value(&m.value)))
            .collect();
        entry["modes"] = Value::Object(modes);
    }
    entry
}

impl SpecFormat for LayoutSpecV1 {
    fn id(&self) -> &'static str {
        "layout-spec-v1"
    }

    fn build(&self, doc: &Document, frame: &NodeId) -> Result<Value, SpecError> {
        let node = crate::ops::find(doc, frame).ok_or_else(|| {
            SpecError::new(format!(
                "there is no layer {frame}. get_app_state and get_nodes list the layers."
            ))
        })?;
        let mut walk = Walk {
            doc,
            vars: BTreeSet::new(),
            styles: BTreeSet::new(),
            components: BTreeMap::new(),
            library: BTreeMap::new(),
            in_definition: false,
        };
        let root = walk.node(node);

        // Components, then what their definitions use; styles, then what they use.
        let mut components = Vec::new();
        let mut done = BTreeSet::new();
        walk.in_definition = true;
        loop {
            let next = walk
                .components
                .keys()
                .find(|id| !done.contains(*id))
                .cloned();
            let Some(id) = next else { break };
            done.insert(id.clone());
            let Some(component) = doc.components.get(&id) else {
                continue;
            };
            let mut props = serde_json::to_value(&component.props).unwrap_or(Value::Null);
            let mut variants = serde_json::to_value(&component.variants).unwrap_or(Value::Null);
            walk.rewrite(&mut props);
            walk.rewrite(&mut variants);
            let definition = walk.node(&component.root);
            components.push((
                component.name.clone(),
                json!({
                    "id": id.as_str(),
                    "name": component.name,
                    "instances": walk.components.get(&id).copied().unwrap_or(0),
                    "props": props,
                    "variants": variants,
                    "definition": definition,
                }),
            ));
        }
        components.sort_by(|a, b| a.0.cmp(&b.0));
        let mut components: Vec<Value> = components.into_iter().map(|(_, v)| v).collect();
        for ((alias, id), instances) in &walk.library {
            components.push(json!({"library": alias, "id": id, "instances": instances}));
        }

        let mut styles = Map::new();
        for id in walk.styles.clone() {
            if let Some(style) = doc.styles.get(&id) {
                let mut value = serde_json::to_value(style).unwrap_or(Value::Null);
                walk.rewrite(&mut value);
                styles.insert(id.as_str().to_owned(), value);
            }
        }

        // Tokens: what the layers use, and whatever those lead to through aliases.
        let (all, warnings) = collect(doc);
        let mut names: BTreeSet<String> = walk
            .vars
            .iter()
            .filter_map(|id| doc.variables.get(id).map(|v| v.name().to_owned()))
            .collect();
        loop {
            let reached: Vec<String> = all
                .tokens
                .iter()
                .filter(|t| names.contains(&t.name))
                .flat_map(|t| std::iter::once(&t.value).chain(t.modes.iter().map(|m| &m.value)))
                .filter_map(|v| match v {
                    TokenValue::Alias(target) => Some(target.clone()),
                    _ => None,
                })
                .collect();
            let before = names.len();
            names.extend(reached);
            if names.len() == before {
                break;
            }
        }
        let tokens: Vec<Value> = names
            .iter()
            .map(|name| match all.get(name) {
                Some(token) => token_entry(token),
                None => {
                    let kind = doc
                        .variables
                        .values()
                        .find(|v| v.name() == name)
                        .map_or(VariableType::String, |v| v.variable_type());
                    let problem = warnings
                        .iter()
                        .find(|w| w.token.as_deref() == Some(name))
                        .map_or("it could not be read".to_owned(), |w| w.message.clone());
                    json!({"name": name, "type": kind.as_str(), "problem": problem})
                }
            })
            .collect();

        let mut frame_info = json!({"id": node.id().as_str(), "name": node.name()});
        if let Some(page) = page_of(doc, frame) {
            frame_info["page"] = json!(page);
        }
        let mut spec = json!({
            "format": self.id(),
            "frame": frame_info,
            "themes": doc.themes,
            "tokens": tokens,
            "components": components,
            "styles": styles,
            "root": root,
        });
        tidy(&mut spec);
        Ok(spec)
    }
}
