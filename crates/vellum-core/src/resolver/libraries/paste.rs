//! Paste is a read-only plan of ordinary ops. The caller supplies the origin and commits the plan as one
//! transaction, so undo, attribution, leases and validation stay on the existing write path.
use super::{import_path, relative_path};
use crate::{
    ops::{Op, Parent},
    ports::IdGen,
};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use vellum_types::{ComponentId, Document, DocumentKind, Node, NodeId, VariableId};

/// Whether pasted library components remain linked or become local independent definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteMode {
    /// Import the source library under this alias.
    Link {
        /// Import alias chosen by the caller.
        alias: String,
    },
    /// Copy components and variables, keeping local nested instances linked to the copied definitions.
    Copies,
}
/// The decision for a variable whose name exists in the destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableChoice {
    /// Bind to the destination variable of the same name and type.
    UseExisting,
    /// Carry the source values under this new name.
    AddRenamed(String),
}
/// Inputs to a paste operation. Library mode is ignored for ordinary source documents.
pub struct PasteRequest<'a> {
    /// Decoded source document.
    pub source: &'a Document,
    /// Source document path, for rebasing imports.
    pub source_path: &'a str,
    /// Destination document path.
    pub target_path: &'a str,
    /// Selected source subtrees.
    pub nodes: &'a [Node],
    /// Destination parent.
    pub to: Parent,
    /// Insertion index.
    pub index: usize,
    /// Link or copies.
    pub mode: PasteMode,
    /// Per-variable decisions keyed by source id.
    pub variables: BTreeMap<VariableId, VariableChoice>,
}
/// A paste needs a decision or cannot preserve the content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteError {
    /// Ask Use existing / Add renamed for this variable.
    VariableClash {
        /// Source id.
        variable: VariableId,
        /// Clashing name.
        name: String,
    },
    /// An explicit choice, reference, path or destination is invalid.
    Invalid(String),
}
/// Reviewable ops and any dependencies the host must copy beside the target file.
pub struct PastePlan {
    /// Apply together with an Origin through the op layer.
    pub ops: Vec<Op>,
    /// Files not moved by this pure plan (assets, fonts and scripts). The host must carry or report them.
    pub dependencies: Vec<String>,
}
#[derive(Default)]
struct Maps {
    nodes: BTreeMap<String, String>,
    components: BTreeMap<String, String>,
    variables: BTreeMap<String, String>,
    styles: BTreeMap<String, String>,
    imports: BTreeMap<String, String>,
}
fn mapped(s: &str, m: &BTreeMap<String, String>) -> String {
    m.get(s).cloned().unwrap_or_else(|| s.into())
}
fn path(s: &str, m: &BTreeMap<String, String>) -> String {
    s.split('/')
        .map(|p| mapped(p, m))
        .collect::<Vec<_>>()
        .join("/")
}
fn rewrite(value: &mut Json, m: &Maps) {
    match value {
        Json::Array(a) => {
            for v in a {
                rewrite(v, m)
            }
        }
        Json::Object(map) => {
            let is_node = map.get("type").and_then(Json::as_str).is_some_and(|k| {
                vellum_types::NodeKind::ALL
                    .iter()
                    .any(|kind| kind.as_str() == k)
            });
            let is_instance = map.get("type").and_then(Json::as_str) == Some("instance");
            let external = is_instance && map.contains_key("library");
            let is_binding = map.contains_key("property") && map.contains_key("node");
            let is_prototype = map.contains_key("target") && map.contains_key("transition");
            for (key, v) in map.iter_mut() {
                if key == "metadata" || (key == "props" && is_instance) {
                    continue;
                }
                let reference = match key.as_str() {
                    "id" if is_node => Some(&m.nodes),
                    "componentId" if !external => Some(&m.components),
                    "$var" | "$alias" => Some(&m.variables),
                    "textStyle" => Some(&m.styles),
                    "library" if is_instance => Some(&m.imports),
                    "target" if is_prototype => Some(&m.nodes),
                    _ => None,
                };
                if let (Some(ids), Json::String(s)) = (reference, &mut *v) {
                    *s = mapped(s, ids);
                    continue;
                }
                if key == "node"
                    && is_binding
                    && let Json::String(s) = v
                {
                    *s = path(s, &m.nodes);
                    continue;
                }
                if key == "suggested"
                    && let Json::Array(ids) = v
                {
                    for id in ids {
                        if let Json::String(s) = id {
                            *s = mapped(s, &m.components)
                        }
                    }
                    continue;
                }
                if matches!(key.as_str(), "inputs" | "uniforms")
                    && let Json::Object(values) = v
                {
                    for value in values.values_mut() {
                        rewrite(value, m);
                    }
                    continue;
                }
                if (key == "overrides" || key == "slotContent")
                    && let Json::Object(entries) = v
                {
                    let old = std::mem::take(entries);
                    for (p, mut value) in old {
                        rewrite(&mut value, m);
                        entries.insert(path(&p, &m.nodes), value);
                    }
                    continue;
                }
                rewrite(v, m);
            }
        }
        _ => {}
    }
}
fn component_refs(value: &Json, out: &mut BTreeSet<String>) {
    match value {
        Json::Array(values) => {
            for value in values {
                component_refs(value, out);
            }
        }
        Json::Object(map) => {
            if !map.contains_key("library")
                && let Some(Json::String(id)) = map.get("componentId")
            {
                out.insert(id.clone());
            }
            if let Some(Json::Array(ids)) = map.get("suggested") {
                out.extend(ids.iter().filter_map(Json::as_str).map(str::to_owned));
            }
            for (key, value) in map {
                if key != "metadata" && key != "inputs" && key != "uniforms" {
                    component_refs(value, out);
                }
            }
        }
        _ => {}
    }
}
fn ids_in(value: &Json, field: &str, out: &mut BTreeSet<String>) {
    match value {
        Json::Array(a) => {
            for v in a {
                ids_in(v, field, out)
            }
        }
        Json::Object(m) => {
            for (k, v) in m {
                if k == "metadata" {
                    continue;
                }
                if k == field
                    && let Json::String(s) = v
                {
                    out.insert(s.clone());
                } else {
                    ids_in(v, field, out)
                }
            }
        }
        _ => {}
    }
}
fn fresh(ids: &mut dyn IdGen, used: &mut BTreeSet<String>) -> Result<String, PasteError> {
    for _ in 0..1024 {
        let id = ids.node_id().to_string();
        if used.insert(id.clone()) {
            return Ok(id);
        }
    }
    Err(PasteError::Invalid(
        "id generator repeatedly returned used ids".into(),
    ))
}
/// Build a deterministic paste plan. No data is written, and no origin is fabricated.
pub fn plan_paste(
    target: &Document,
    request: &PasteRequest<'_>,
    ids: &mut dyn IdGen,
) -> Result<PastePlan, PasteError> {
    let fail = |s: String| PasteError::Invalid(s);
    let source = request.source;
    let linked =
        source.meta.kind == DocumentKind::Library && matches!(request.mode, PasteMode::Link { .. });
    let mut next = target.clone();
    let mut maps = Maps::default();
    let mut used = BTreeSet::new();
    ids_in(
        &serde_json::to_value(target).expect("document"),
        "id",
        &mut used,
    );
    used.extend(target.components.keys().map(ToString::to_string));
    used.extend(target.variables.keys().map(ToString::to_string));
    used.extend(target.styles.keys().map(ToString::to_string));
    let mut selected = serde_json::to_value(request.nodes).expect("nodes");
    let mut component_ids = BTreeSet::new();
    component_refs(&selected, &mut component_ids);
    let mut components = BTreeMap::new();
    let mut variables = BTreeSet::new();
    ids_in(&selected, "$var", &mut variables);
    if !linked {
        let mut todo = component_ids.clone();
        while let Some(id) = todo.pop_first() {
            if components.contains_key(&id) {
                continue;
            }
            // External instances already have an import alias and are rebound below; they are not local definitions.
            let Some(component) = source
                .components
                .get(&ComponentId::new(&id).map_err(|e| fail(e.to_string()))?)
            else {
                continue;
            };
            let json = serde_json::to_value(component).expect("component");
            let mut refs = BTreeSet::new();
            component_refs(&json, &mut refs);
            todo.extend(refs);
            ids_in(&json, "$var", &mut variables);
            maps.components.insert(id.clone(), fresh(ids, &mut used)?);
            components.insert(id, json);
        }
    }
    let mut styles = BTreeMap::new();
    let mut style_ids = BTreeSet::new();
    ids_in(&selected, "textStyle", &mut style_ids);
    for component in components.values() {
        ids_in(component, "textStyle", &mut style_ids);
    }
    for id in style_ids {
        let key = vellum_types::StyleId::new(&id).map_err(|e| fail(e.to_string()))?;
        let style = source
            .styles
            .get(&key)
            .ok_or_else(|| fail(format!("source style {id} is missing")))?;
        let json = serde_json::to_value(style).expect("style");
        ids_in(&json, "$var", &mut variables);
        maps.styles.insert(id.clone(), fresh(ids, &mut used)?);
        styles.insert(id, json);
    }
    let mut todo = variables.clone();
    while let Some(id) = todo.pop_first() {
        let key = VariableId::new(&id).map_err(|e| fail(e.to_string()))?;
        let v = source
            .variables
            .get(&key)
            .ok_or_else(|| fail(format!("source variable {id} is missing")))?;
        for alias in v.aliases() {
            if variables.insert(alias.to_string()) {
                todo.insert(alias.to_string());
            }
        }
    }
    let mut new_variables = BTreeMap::new();
    let mut names: BTreeSet<String> = target.variables.values().map(|v| v.name().into()).collect();
    for id in variables {
        let key = VariableId::new(&id).map_err(|e| fail(e.to_string()))?;
        let v = &source.variables[&key];
        let existing = target.variables.iter().find(|(_, e)| e.name() == v.name());
        let mut name = v.name().to_owned();
        if let Some((eid, ev)) = existing {
            match request.variables.get(&key) {
                Some(VariableChoice::UseExisting) => {
                    if ev.variable_type() != v.variable_type() {
                        return Err(fail(format!("variable {} has a different type", v.name())));
                    }
                    maps.variables.insert(id, eid.to_string());
                    continue;
                }
                Some(VariableChoice::AddRenamed(n)) => {
                    if n.is_empty() || names.contains(n) {
                        return Err(fail("renamed variable needs an unused name".into()));
                    }
                    name = n.clone();
                }
                None => {
                    return Err(PasteError::VariableClash {
                        variable: key,
                        name,
                    });
                }
            }
        }
        names.insert(name.clone());
        let new_id = fresh(ids, &mut used)?;
        maps.variables.insert(id, new_id.clone());
        let mut json = serde_json::to_value(v).expect("variable");
        json["name"] = Json::String(name);
        new_variables.insert(new_id, json);
    }
    for (axis, values) in &source.themes {
        let dest = next.themes.entry(axis.clone()).or_default();
        for v in values {
            if !dest.contains(v) {
                dest.push(v.clone())
            }
        }
    }
    // Imports used by copied nested links are rebased, preserving their origin when folders differ.
    for (alias, relative) in &source.imports {
        let absolute = import_path(request.source_path, relative).map_err(fail)?;
        let relative = relative_path(request.target_path, &absolute);
        let imported = if let Some((alias, _)) = next.imports.iter().find(|(_, p)| *p == &relative)
        {
            alias.clone()
        } else {
            let mut candidate = alias.clone();
            let mut count = 2;
            while next.imports.contains_key(&candidate) {
                candidate = format!("{alias}{count}");
                count += 1;
            }
            next.imports.insert(candidate.clone(), relative);
            candidate
        };
        maps.imports.insert(alias.clone(), imported);
    }
    if linked && let PasteMode::Link { alias } = &request.mode {
        if alias.is_empty() {
            return Err(fail("import alias cannot be empty".into()));
        }
        let relative = relative_path(request.target_path, request.source_path);
        import_path(request.target_path, &relative).map_err(fail)?;
        if next.imports.get(alias).is_some_and(|p| p != &relative) {
            return Err(fail(
                "import alias already points to another library".into(),
            ));
        }
        next.imports.insert(alias.clone(), relative);
        fn link(v: &mut Json, alias: &str) {
            match v {
                Json::Array(a) => {
                    for v in a {
                        link(v, alias)
                    }
                }
                Json::Object(m) => {
                    if m.get("type").and_then(Json::as_str) == Some("instance")
                        && !m.contains_key("library")
                    {
                        m.insert("library".into(), Json::String(alias.into()));
                    }
                    for (k, v) in m {
                        if k != "metadata" {
                            link(v, alias)
                        }
                    }
                }
                _ => {}
            }
        }
        // Rewrite existing external aliases first; the new link alias is in target scope.
        rewrite(&mut selected, &maps);
        link(&mut selected, alias);
    }
    // Every owned node id and override path is remapped as a group; definition and payload ids stay stable relative to each other.
    let mut node_ids = BTreeSet::new();
    ids_in(&selected, "id", &mut node_ids);
    for v in components.values() {
        ids_in(v, "id", &mut node_ids)
    }
    for id in node_ids {
        maps.nodes.insert(id, fresh(ids, &mut used)?);
    }
    if linked {
        let node_only = Maps {
            nodes: maps.nodes.clone(),
            ..Maps::default()
        };
        rewrite(&mut selected, &node_only);
    } else {
        rewrite(&mut selected, &maps);
    }
    let mut dependencies = BTreeSet::new();
    for value in std::iter::once(&selected).chain(components.values()) {
        ids_in(value, "asset", &mut dependencies);
        ids_in(value, "src", &mut dependencies);
    }
    for (id, mut json) in components {
        rewrite(&mut json, &maps);
        next.components.insert(
            ComponentId::new(&maps.components[&id]).expect("generated"),
            serde_json::from_value(json).map_err(|e| fail(e.to_string()))?,
        );
    }
    for (id, mut json) in new_variables {
        rewrite(&mut json, &maps);
        next.variables.insert(
            VariableId::new(id).expect("generated"),
            serde_json::from_value(json).map_err(|e| fail(e.to_string()))?,
        );
    }
    // Styles/fonts travel as definitions; bytes remain an explicit host dependency.
    for (id, mut v) in styles {
        rewrite(&mut v, &maps);
        next.styles.insert(
            vellum_types::StyleId::new(&maps.styles[&id]).expect("generated"),
            serde_json::from_value(v).map_err(|e| fail(e.to_string()))?,
        );
    }
    next.fonts.extend(source.fonts.clone());
    let nodes: Vec<Node> = serde_json::from_value(selected).map_err(|e| fail(e.to_string()))?;
    let children = match &request.to {
        Parent::Page(id) => {
            &mut next
                .pages
                .iter_mut()
                .find(|p| p.id == *id)
                .ok_or_else(|| fail("destination page is missing".into()))?
                .children
        }
        Parent::Node(id) => {
            fn find<'a>(nodes: &'a mut [Node], id: &NodeId) -> Option<&'a mut Node> {
                for n in nodes {
                    if n.id() == id {
                        return Some(n);
                    }
                    if let Some(children) = n.children_mut()
                        && let Some(n) = find(children, id)
                    {
                        return Some(n);
                    }
                }
                None
            }
            let mut found = None;
            for p in &mut next.pages {
                if let Some(n) = find(&mut p.children, id) {
                    found = Some(n);
                    break;
                }
            }
            found
                .ok_or_else(|| fail("destination node is missing".into()))?
                .children_mut()
                .ok_or_else(|| fail("destination cannot hold children".into()))?
        }
    };
    if request.index > children.len() {
        return Err(fail("paste index is out of range".into()));
    }
    children.splice(request.index..request.index, nodes);
    dependencies.extend(source.fonts.keys().map(|id| format!("font:{id}")));
    let problems = crate::ops::check_document(&next);
    if !problems.is_empty() {
        return Err(fail(problems.join("; ")));
    }
    let ops = crate::diff::diff(target, &next).map_err(|e| fail(e.to_string()))?;
    Ok(PastePlan {
        ops,
        dependencies: dependencies.into_iter().collect(),
    })
}
