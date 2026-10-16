//! Components, variants, overrides and slots expanded into addressed trees. Each occurrence has an
//! instance path and a source document, so repeated definitions and library-local ids cannot collide.
mod instantiate;
pub use instantiate::{ComponentError, instantiate};
#[cfg(test)]
pub(crate) mod tests;
use super::{
    libraries::{Libraries, normalize},
    variables::{self, Theme},
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use vellum_types::{
    ComponentId, Document, InstanceNode, Node, NodeIdPath, NodePatch, Override, Slot, Variable,
    VariableId,
};

/// A component expansion failure or a variable/theme resolution failure.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// A cached derived script result no longer matches its inputs or source path.
    Script { message: String },
    /// The definition, prop, override, slot or library cannot be used.
    Component(ComponentError),
    /// An own property could not resolve under its source document's theme.
    Value(variables::Problem),
}
/// Source information for an expanded instance, retained for inspection and reset commands.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceInfo {
    /// Definition id in `source`.
    pub component: ComponentId,
    /// Definition's source file.
    pub source: String,
    /// Original instance including choices and overrides.
    pub instance: InstanceNode,
}
/// Resolved own fields, source and occurrence address, with the expanded children.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedNode {
    /// Resolved own values. Children live in this type's `children` only.
    pub value: Arc<variables::ResolvedNode>,
    /// Address from the page root through every instance occurrence.
    pub address: NodeIdPath,
    /// File providing this node's variable and component scope.
    pub source: String,
    /// Component link when this node is an expanded instance root.
    pub instance: Option<InstanceInfo>,
    /// Slot suggestions, also on filled slots; advisory, never restrictive.
    pub slot: Option<Slot>,
    /// Missing library paths used by this subtree (including cached dependencies).
    pub missing_libraries: Vec<String>,
    /// Failures on this own node.
    pub problems: Vec<Problem>,
    /// Expanded children, back to front.
    pub children: Vec<Arc<ResolvedNode>>,
}
impl ResolvedNode {
    /// Every problem in tree order, with an unambiguous occurrence address.
    pub fn all_problems(&self) -> Vec<(&NodeIdPath, &Problem)> {
        let mut out = self
            .problems
            .iter()
            .map(|p| (&self.address, p))
            .collect::<Vec<_>>();
        for c in &self.children {
            out.extend(c.all_problems());
        }
        out
    }
}
struct ScopeState {
    themes: BTreeMap<String, Vec<String>>,
    variables: BTreeMap<VariableId, Variable>,
    version: u64,
}
struct Entry {
    input: Node,
    theme: Theme,
    version: u64,
    value: Arc<variables::ResolvedNode>,
    result: Arc<ResolvedNode>,
}
/// A memoized resolver. Raw node comparisons detect component/override changes; source revisions track
/// variables and theme axes. It is safe to reuse after edits or library refresh without manual invalidation.
#[derive(Default)]
pub struct Resolver {
    script_outputs: BTreeMap<(String, vellum_types::NodeId), crate::script_nodes::Output>,
    entries: BTreeMap<(String, NodeIdPath), Entry>,
    scopes: BTreeMap<String, ScopeState>,
    seen: BTreeSet<(String, NodeIdPath)>,
    seen_scopes: BTreeSet<String>,
}
impl Resolver {
    /// Replace successful derived outputs for this pass. Missing/failed scripts have no entry, so
    /// stale output cannot survive an error. Paths distinguish library scripts from local scripts.
    pub fn set_script_outputs(
        &mut self,
        outputs: BTreeMap<(String, vellum_types::NodeId), crate::script_nodes::Output>,
    ) {
        self.script_outputs = outputs
            .into_iter()
            .map(|((path, id), output)| ((normalize(&path), id), output))
            .collect();
    }
    /// An empty resolver.
    pub fn new() -> Self {
        Self::default()
    }
    /// Discard every cached value.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.scopes.clear();
        self.seen.clear();
        self.seen_scopes.clear();
    }
    /// Resolve all page roots.
    pub fn resolve_page(
        &mut self,
        doc: &Document,
        path: &str,
        page: &vellum_types::Page,
        libraries: &mut Libraries<'_>,
    ) -> Vec<Arc<ResolvedNode>> {
        self.resolve_nodes(doc, path, &page.children, libraries)
    }
    /// Resolve multiple roots with one consistent refresh of every library.
    pub fn resolve_nodes(
        &mut self,
        doc: &Document,
        path: &str,
        nodes: &[Node],
        libraries: &mut Libraries<'_>,
    ) -> Vec<Arc<ResolvedNode>> {
        libraries.begin();
        self.seen.clear();
        self.seen_scopes.clear();
        let path = normalize(path);
        let theme = Theme::defaults(doc);
        let mut walk = Walk {
            resolver: self,
            libraries,
            stack: Vec::new(),
            origins: Vec::new(),
        };
        let result = nodes
            .iter()
            .map(|n| walk.node(doc, &path, n, NodeIdPath::from(n.id().clone()), &theme, 0))
            .collect();
        self.entries.retain(|k, _| self.seen.contains(k));
        result
    }
    /// Resolve one root.
    pub fn resolve_node(
        &mut self,
        doc: &Document,
        path: &str,
        node: &Node,
        libraries: &mut Libraries<'_>,
    ) -> Arc<ResolvedNode> {
        self.resolve_nodes(doc, path, std::slice::from_ref(node), libraries)
            .remove(0)
    }
    fn value(
        &mut self,
        doc: &Document,
        path: &str,
        node: &Node,
        address: &NodeIdPath,
        theme: &Theme,
    ) -> Arc<variables::ResolvedNode> {
        let state = self
            .scopes
            .entry(path.to_owned())
            .or_insert_with(|| ScopeState {
                themes: doc.themes.clone(),
                variables: doc.variables.clone(),
                version: 0,
            });
        if self.seen_scopes.insert(path.to_owned())
            && (state.themes != doc.themes || state.variables != doc.variables)
        {
            state.themes = doc.themes.clone();
            state.variables = doc.variables.clone();
            state.version += 1;
        }
        let version = state.version;
        let key = (path.to_owned(), address.clone());
        self.seen.insert(key.clone());
        if let Some(e) = self.entries.get(&key)
            && e.version == version
            && e.input == *node
            && e.theme == *theme
        {
            return e.value.clone();
        }
        variables::Resolver::new().resolve_node(doc, node, theme)
    }
    fn intern(&mut self, input: Node, theme: Theme, result: ResolvedNode) -> Arc<ResolvedNode> {
        let key = (result.source.clone(), result.address.clone());
        let previous = self
            .entries
            .get(&key)
            .filter(|e| {
                let old = &e.result;
                old.value == result.value
                    && old.instance == result.instance
                    && old.slot == result.slot
                    && old.missing_libraries == result.missing_libraries
                    && old.problems == result.problems
                    && old.children.len() == result.children.len()
                    && old
                        .children
                        .iter()
                        .zip(&result.children)
                        .all(|(a, b)| Arc::ptr_eq(a, b))
            })
            .map(|e| e.result.clone());
        let result = previous.unwrap_or_else(|| Arc::new(result));
        let version = self.scopes[&result.source].version;
        self.entries.insert(
            key,
            Entry {
                input,
                theme,
                version,
                value: result.value.clone(),
                result: result.clone(),
            },
        );
        result
    }
}
struct Walk<'a, 'b, 'c> {
    resolver: &'a mut Resolver,
    libraries: &'b mut Libraries<'c>,
    stack: Vec<(String, ComponentId)>,
    origins: Vec<OwnedOrigin>,
}
#[derive(Clone)]
struct OwnedOrigin {
    address: NodeIdPath,
    doc: Arc<Document>,
    path: String,
}
fn cross_theme(doc: &Document, theme: &Theme) -> Theme {
    let choices = theme
        .iter()
        .filter(|(a, v)| {
            doc.themes
                .get(*a)
                .is_some_and(|values| values.iter().any(|s| s == v))
        })
        .map(|(a, v)| (a.into(), v.into()))
        .collect();
    Theme::defaults(doc)
        .with(doc, &choices)
        .expect("filtered choices")
}
impl Walk<'_, '_, '_> {
    /// Find the typed target in the effective tree, crossing nested instance boundaries without losing
    /// its rendered address or inherited theme. Definition roots inside instances are path segments only.
    #[allow(clippy::too_many_arguments)]
    fn snapshot(
        &mut self,
        doc: &Document,
        source: &str,
        node: &Node,
        parts: &[&vellum_types::NodeId],
        theme: &Theme,
        suffix: Vec<vellum_types::NodeId>,
        depth: usize,
    ) -> Result<(Node, Theme, Vec<vellum_types::NodeId>), String> {
        if depth > 128 {
            return Err("instance depth exceeds 128".into());
        }
        let mut errors = Vec::new();
        let theme = node
            .theme()
            .map(|choices| theme.with_lenient(doc, choices, &mut errors))
            .unwrap_or_else(|| theme.clone());
        if parts.is_empty() {
            return Ok((node.clone(), theme, suffix));
        }
        if let Node::Instance(instance) = node {
            let library = if let Some(alias) = &instance.library {
                let relative = doc.imports.get(alias).ok_or("unknown library alias")?;
                Some(self.libraries.get(source, relative)?)
            } else {
                None
            };
            let (doc, source) = match &library {
                Some(library) => (
                    library.document.as_deref().ok_or("missing library")?,
                    library.path.as_str(),
                ),
                None => (doc, source),
            };
            let component = doc
                .components
                .get(&instance.component_id)
                .ok_or("missing component")?;
            let root = instantiate(component, instance).map_err(|e| e.to_string())?;
            if parts[0] != root.id() {
                return Err("nested path must name the definition root".into());
            }
            return self.snapshot(
                doc,
                source,
                &root,
                &parts[1..],
                &cross_theme(doc, &theme),
                suffix,
                depth + 1,
            );
        }
        let child = node
            .children()
            .iter()
            .find(|n| n.id() == parts[0])
            .ok_or("no node at this path")?;
        let mut suffix = suffix;
        suffix.push(child.id().clone());
        self.snapshot(doc, source, child, &parts[1..], &theme, suffix, depth + 1)
    }
    fn origin(&self, address: &NodeIdPath, source: &str) -> Option<OwnedOrigin> {
        self.origins
            .iter()
            .rev()
            .find(|origin| origin.address == *address)
            .filter(|origin| origin.path != source)
            .cloned()
    }
    fn node(
        &mut self,
        doc: &Document,
        path: &str,
        node: &Node,
        address: NodeIdPath,
        theme: &Theme,
        depth: usize,
    ) -> Arc<ResolvedNode> {
        if let Some(origin) = self.origin(&address, path) {
            let inherited = std::mem::take(&mut self.stack);
            let result = self.node(
                &origin.doc,
                &origin.path,
                node,
                address,
                &cross_theme(&origin.doc, theme),
                depth,
            );
            self.stack = inherited;
            return result;
        }
        if depth > 128 {
            return self.finish(
                doc,
                path,
                node,
                address,
                theme,
                None,
                vec![Problem::Component(instantiate::error(
                    "componentId",
                    "instance depth exceeds 128",
                ))],
                vec![],
                vec![],
            );
        }
        if let Node::Instance(instance) = node {
            return self.instance(doc, path, instance, address, theme, depth);
        }
        if matches!(node, Node::Script(_))
            && let Some(output) = self
                .resolver
                .script_outputs
                .get(&(path.to_owned(), node.id().clone()))
                .cloned()
        {
            if !crate::script_nodes::is_current(&output, doc, node, theme) {
                return self.finish(
                    doc,
                    path,
                    node,
                    address,
                    theme,
                    None,
                    vec![Problem::Script {
                        message: "Script inputs changed; run the script again".into(),
                    }],
                    vec![],
                    vec![],
                );
            }
            let mut frame = node.as_frame(output.nodes);
            if let Node::Frame(n) = &mut frame {
                n.clip = matches!(node,Node::Script(script) if script.clip);
            }
            return self.node(doc, path, &frame, address, theme, depth + 1);
        }
        let value = self
            .resolver
            .value(doc, path, &node.without_children(), &address, theme);
        let children = node
            .children()
            .iter()
            .map(|n| {
                self.node(
                    doc,
                    path,
                    n,
                    address.clone().join(n.id().clone()),
                    &value.theme,
                    depth + 1,
                )
            })
            .collect();
        self.finish(
            doc,
            path,
            node,
            address,
            theme,
            None,
            vec![],
            children,
            vec![],
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn finish(
        &mut self,
        doc: &Document,
        path: &str,
        node: &Node,
        address: NodeIdPath,
        theme: &Theme,
        instance: Option<InstanceInfo>,
        mut problems: Vec<Problem>,
        children: Vec<Arc<ResolvedNode>>,
        missing: Vec<String>,
    ) -> Arc<ResolvedNode> {
        let own = node.without_children();
        let value = self.resolver.value(doc, path, &own, &address, theme);
        problems.extend(value.problems.iter().cloned().map(Problem::Value));
        let mut missing: BTreeSet<_> = missing.into_iter().collect();
        for c in &children {
            missing.extend(c.missing_libraries.clone());
        }
        let slot = if let Node::Frame(f) = node {
            f.slot.clone()
        } else {
            None
        };
        self.resolver.intern(
            own,
            theme.clone(),
            ResolvedNode {
                value,
                address,
                source: path.into(),
                instance,
                slot,
                missing_libraries: missing.into_iter().collect(),
                problems,
                children,
            },
        )
    }
    fn instance(
        &mut self,
        owner: &Document,
        owner_path: &str,
        instance: &InstanceNode,
        address: NodeIdPath,
        theme: &Theme,
        depth: usize,
    ) -> Arc<ResolvedNode> {
        let original = Node::Instance(instance.clone());
        let mut missing = vec![];
        let library = match &instance.library {
            None => None,
            Some(alias) => match owner
                .imports
                .get(alias)
                .ok_or_else(|| format!("unknown library alias {alias}"))
                .and_then(|p| self.libraries.get(owner_path, p))
            {
                Ok(lib) => {
                    if lib.missing {
                        missing.push(lib.path.clone());
                    }
                    Some(lib)
                }
                Err(e) => {
                    return self.finish(
                        owner,
                        owner_path,
                        &original,
                        address,
                        theme,
                        None,
                        vec![Problem::Component(instantiate::error("library", e))],
                        vec![],
                        missing,
                    );
                }
            },
        };
        let (doc, path) = match &library {
            Some(lib) => match &lib.document {
                Some(doc) => (doc.as_ref(), lib.path.as_str()),
                None => {
                    return self.finish(
                        owner,
                        owner_path,
                        &original,
                        address,
                        theme,
                        None,
                        vec![Problem::Component(instantiate::error(
                            "library",
                            lib.problem.as_deref().unwrap_or("missing library"),
                        ))],
                        vec![],
                        missing,
                    );
                }
            },
            None => (owner, owner_path),
        };
        let key = (path.to_owned(), instance.component_id.clone());
        if self.stack.contains(&key) {
            return self.finish(
                owner,
                owner_path,
                &original,
                address,
                theme,
                None,
                vec![Problem::Component(instantiate::error(
                    "componentId",
                    "recursive component instance",
                ))],
                vec![],
                missing,
            );
        }
        let Some(component) = doc.components.get(&instance.component_id) else {
            return self.finish(
                owner,
                owner_path,
                &original,
                address,
                theme,
                None,
                vec![Problem::Component(instantiate::error(
                    "componentId",
                    "component does not exist",
                ))],
                vec![],
                missing,
            );
        };
        let local_theme = cross_theme(doc, theme);
        let mut prepared = instance.clone();
        let mut problems = vec![];
        let origin_start = self.origins.len();
        // Imported definitions keep library bindings; patches and common fields belong to the importer.
        if library.is_some() {
            let effective =
                instantiate(component, instance).unwrap_or_else(|_| component.root.clone());
            let owner_copy = Arc::new(owner.clone());
            for (target_path, over) in &mut prepared.overrides {
                let segments: Vec<_> = target_path.segments().collect();
                let snapshot = self.snapshot(
                    doc,
                    path,
                    &effective,
                    &segments[1..],
                    &local_theme,
                    Vec::new(),
                    0,
                );
                if let Ok((_, _, suffix)) = &snapshot {
                    let mut target_address = address.clone();
                    for id in suffix {
                        target_address = target_address.join(id.clone());
                    }
                    match over {
                        Override::Replace(_) => self.origins.push(OwnedOrigin {
                            address: target_address,
                            doc: owner_copy.clone(),
                            path: owner_path.into(),
                        }),
                        Override::Children(nodes) => {
                            for node in nodes {
                                self.origins.push(OwnedOrigin {
                                    address: target_address.clone().join(node.id().clone()),
                                    doc: owner_copy.clone(),
                                    path: owner_path.into(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                if let Override::Props(patch) = over
                    && let Ok((target, target_theme, _)) = snapshot
                    && let Ok(patched) = target.patched(patch)
                {
                    let value = variables::Resolver::new().resolve_node(
                        owner,
                        &patched.without_children(),
                        &cross_theme(owner, &target_theme),
                    );
                    let json = serde_json::to_value(&value.node).expect("node");
                    let names = patch.properties();
                    problems.extend(
                        value
                            .problems
                            .iter()
                            .filter(|p| {
                                names.keys().any(|name| {
                                    p.at == *name
                                        || p.at.starts_with(&format!("{name}."))
                                        || p.at.starts_with(&format!("{name}["))
                                })
                            })
                            .cloned()
                            .map(Problem::Value),
                    );
                    *patch = NodePatch::new(
                        names
                            .keys()
                            .map(|name| {
                                (
                                    name.clone(),
                                    json.get(name).cloned().unwrap_or(serde_json::Value::Null),
                                )
                            })
                            .collect(),
                    )
                    .expect("patch keys");
                }
            }
            let resolved = variables::Resolver::new().resolve_node(owner, &original, theme);
            let sparse = original.common_sparse_patch();
            let json = serde_json::to_value(&resolved.node).expect("node");
            let patch = NodePatch::new(
                sparse
                    .properties()
                    .keys()
                    .map(|k| {
                        (
                            k.clone(),
                            json.get(k).cloned().unwrap_or(serde_json::Value::Null),
                        )
                    })
                    .collect(),
            )
            .expect("common keys");
            let Node::Instance(n) = original
                .patched(&patch)
                .expect("resolved common properties")
            else {
                unreachable!()
            };
            prepared = InstanceNode {
                overrides: prepared.overrides,
                ..n
            };
        }
        let mut root = match instantiate(component, &prepared) {
            Ok(root) => root,
            Err(e) => {
                self.origins.truncate(origin_start);
                return self.finish(
                    owner,
                    owner_path,
                    &original,
                    address,
                    theme,
                    None,
                    vec![Problem::Component(e)],
                    vec![],
                    missing,
                );
            }
        };
        instantiate::set_id(&mut root, &instance.id);
        let info = InstanceInfo {
            component: instance.component_id.clone(),
            source: path.into(),
            instance: instance.clone(),
        };
        if instance
            .overrides
            .iter()
            .any(|(p, o)| p.len() == 1 && matches!(o, Override::Replace(_)))
        {
            self.origins.truncate(origin_start);
            let tree = self.node(owner, owner_path, &root, address, theme, depth + 1);
            let mut result = (*tree).clone();
            result.instance = Some(info);
            result.problems.extend(problems);
            let mut all_missing: BTreeSet<_> = result.missing_libraries.into_iter().collect();
            all_missing.extend(missing);
            result.missing_libraries = all_missing.into_iter().collect();
            return Arc::new(result);
        }
        self.stack.push(key);
        let value =
            self.resolver
                .value(doc, path, &root.without_children(), &address, &local_theme);
        // Structural payloads are owned by the importing document, even when they replace library nodes.
        let mut owned = BTreeSet::new();
        for over in instance.overrides.values() {
            match over {
                Override::Replace(n) => {
                    owned.insert(n.id().clone());
                }
                Override::Children(nodes) => {
                    owned.extend(nodes.iter().map(|n| n.id().clone()));
                }
                _ => {}
            }
        }
        for nodes in instance.slot_content.values() {
            owned.extend(nodes.iter().map(|n| n.id().clone()));
        }
        let mut replacement_paths = BTreeSet::new();
        for (p, o) in &instance.overrides {
            if matches!(o, Override::Replace(_)) {
                replacement_paths.insert(p.last().clone());
            }
        }
        let children = self.children(
            doc,
            path,
            owner,
            owner_path,
            &root,
            &address,
            &value.theme,
            theme,
            &owned,
            &replacement_paths,
            depth + 1,
        );
        self.stack.pop();
        let result = self.finish(
            doc,
            path,
            &root,
            address,
            &local_theme,
            Some(info),
            problems,
            children,
            missing,
        );
        self.origins.truncate(origin_start);
        result
    }
    #[allow(clippy::too_many_arguments)]
    fn children(
        &mut self,
        doc: &Document,
        path: &str,
        owner: &Document,
        owner_path: &str,
        root: &Node,
        address: &NodeIdPath,
        theme: &Theme,
        owner_theme: &Theme,
        owned: &BTreeSet<vellum_types::NodeId>,
        replaced: &BTreeSet<vellum_types::NodeId>,
        depth: usize,
    ) -> Vec<Arc<ResolvedNode>> {
        root.children()
            .iter()
            .map(|n| {
                let address = address.clone().join(n.id().clone());
                if self.origin(&address, path).is_some() {
                    return self.node(doc, path, n, address, theme, depth);
                }
                if owned.contains(n.id()) || replaced.contains(n.id()) {
                    let inherited = std::mem::take(&mut self.stack);
                    let result = self.node(owner, owner_path, n, address, owner_theme, depth);
                    self.stack = inherited;
                    return result;
                }
                if matches!(n, Node::Instance(_)) {
                    return self.node(doc, path, n, address, theme, depth);
                }
                let value = self
                    .resolver
                    .value(doc, path, &n.without_children(), &address, theme);
                let children = self.children(
                    doc,
                    path,
                    owner,
                    owner_path,
                    n,
                    &address,
                    &value.theme,
                    owner_theme,
                    owned,
                    replaced,
                    depth + 1,
                );
                self.finish(doc, path, n, address, theme, None, vec![], children, vec![])
            })
            .collect()
    }
}
