//! The document in Loro. Private: nothing here is visible outside the crate.
//!
//! Layout (ARCHITECTURE §4.2, ADR-004):
//!
//! - one movable tree, `nodes`, with fractional indexes so children keep their order. Its roots are the pages,
//!   in order, and one root per component. A page root's meta holds `page` (the page without its children);
//!   a component root's meta holds `component` (the id) and `props` (the definition without its node tree),
//!   and its one child is the component's root node;
//! - every node is one tree node, its meta holding each property of the node (children aside) as a JSON
//!   string, so edits to different properties of one node merge;
//! - the map `document` holds `shell`: what no edit changes (meta, imports, themes, styles, fonts);
//! - the map `variables` holds each variable as a JSON string.
//!
//! Every store opened from the same document writes it the same way, as the same peer (the genesis peer,
//! derived from the document's content), so their histories share that first change and sync without
//! duplicating it.

use std::collections::{BTreeMap, HashMap};

use loro::{
    ExportMode, LoroDoc, LoroMap, LoroTree, LoroValue, TreeID, TreeParentId, ValueOrContainer,
    VersionVector,
};
use serde_json::{Map, Value as Json};
use vellum_core::ops::{Edit, Parent};
use vellum_core::ports::SyncError;
use vellum_types::{Component, ComponentId, Document, Node, NodeId, Origin, Page, PageId};

/// Genesis peers have this bit set; a store's own peer must not.
pub(crate) const GENESIS_BIT: u64 = 1 << 63;

const TREE: &str = "nodes";
const SHELL: &str = "document";
const VARIABLES: &str = "variables";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Root {
    Page(PageId),
    Component(ComponentId),
}

pub(crate) struct Backend {
    doc: LoroDoc,
    tree: LoroTree,
    nodes: HashMap<NodeId, TreeID>,
    ids: HashMap<TreeID, NodeId>,
    roots: HashMap<Root, TreeID>,
    genesis: u64,
}

impl std::fmt::Debug for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backend")
            .field("nodes", &self.nodes.len())
            .field("genesis", &self.genesis)
            .finish_non_exhaustive()
    }
}

fn json_string(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("our types serialise")
}

/// The genesis peer for `document`: FNV-1a over its JSON, with the genesis bit set.
fn genesis_peer(document: &Document) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in serde_json::to_vec(document).expect("a document serialises") {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let peer = hash | GENESIS_BIT;
    // Loro reserves u64::MAX.
    if peer == u64::MAX { peer - 1 } else { peer }
}

/// A node's properties, children aside, as JSON strings by name.
fn properties(node: &Node) -> BTreeMap<String, String> {
    let Json::Object(map) = serde_json::to_value(node).expect("a node serialises") else {
        unreachable!("a node serialises to an object")
    };
    map.into_iter()
        .filter(|(key, _)| key != "children")
        .map(|(key, value)| (key, value.to_string()))
        .collect()
}

fn string(value: &LoroValue) -> Option<&str> {
    match value {
        LoroValue::String(s) => Some(s.as_str()),
        _ => None,
    }
}

fn read_map(map: &LoroMap) -> BTreeMap<String, String> {
    match map.get_value() {
        LoroValue::Map(entries) => entries
            .iter()
            .filter_map(|(k, v)| string(v).map(|v| (k.to_string(), v.to_string())))
            .collect(),
        _ => BTreeMap::new(),
    }
}

fn parse(text: &str) -> Json {
    serde_json::from_str(text).expect("the adapter wrote this JSON")
}

impl Backend {
    /// A Loro document holding `document`, written by its genesis peer, then switched to `peer`.
    pub fn genesis(document: &Document, peer: u64) -> Self {
        assert!(
            peer & GENESIS_BIT == 0,
            "a store's peer id must be below 2^63; higher ones name a document's genesis"
        );
        let doc = LoroDoc::new();
        let genesis = genesis_peer(document);
        doc.set_peer_id(genesis).expect("a valid peer id");
        let tree = doc.get_tree(TREE);
        tree.enable_fractional_index(0);
        let mut backend = Self {
            doc,
            tree,
            nodes: HashMap::new(),
            ids: HashMap::new(),
            roots: HashMap::new(),
            genesis,
        };

        let mut shell = document.clone();
        shell.pages.clear();
        shell.variables.clear();
        shell.components.clear();
        backend
            .doc
            .get_map(SHELL)
            .insert("shell", json_string(&shell))
            .expect("write the shell");
        for page in &document.pages {
            let root = backend
                .tree
                .create(TreeParentId::Root)
                .expect("a page root");
            let mut bare = page.clone();
            bare.children.clear();
            backend
                .meta(root)
                .insert("page", json_string(&bare))
                .expect("write a page");
            backend.roots.insert(Root::Page(page.id.clone()), root);
            for (index, node) in page.children.iter().enumerate() {
                backend.create(root, index, node);
            }
        }
        for (id, component) in &document.components {
            backend.set_component(id, component);
        }
        let variables = backend.doc.get_map(VARIABLES);
        for (id, variable) in &document.variables {
            variables
                .insert(id.as_str(), json_string(variable))
                .expect("write a variable");
        }
        backend.doc.commit();
        backend.doc.set_peer_id(peer).expect("a valid peer id");
        backend
    }

    fn meta(&self, id: TreeID) -> LoroMap {
        self.tree.get_meta(id).expect("a live tree node has meta")
    }

    fn create(&mut self, parent: TreeID, index: usize, node: &Node) -> TreeID {
        let id = self
            .tree
            .create_at(parent, index)
            .expect("the index was checked by the op layer");
        let meta = self.meta(id);
        for (key, value) in properties(node) {
            meta.insert(&key, value).expect("write a property");
        }
        self.nodes.insert(node.id().clone(), id);
        self.ids.insert(id, node.id().clone());
        for (index, child) in node.children().iter().enumerate() {
            self.create(id, index, child);
        }
        id
    }

    fn forget(&mut self, id: TreeID) {
        if let Some(children) = self.tree.children(id) {
            for child in children {
                self.forget(child);
            }
        }
        if let Some(node) = self.ids.remove(&id) {
            self.nodes.remove(&node);
        }
    }

    fn delete(&mut self, id: TreeID) {
        self.forget(id);
        self.tree.delete(id).expect("a live tree node");
    }

    fn parent(&self, parent: &Parent) -> TreeID {
        match parent {
            Parent::Page(page) => self.roots[&Root::Page(page.clone())],
            Parent::Node(node) => self.nodes[node],
        }
    }

    fn set_component(&mut self, id: &ComponentId, component: &Component) {
        let root = self
            .tree
            .create(TreeParentId::Root)
            .expect("a component root");
        let meta = self.meta(root);
        meta.insert("component", id.as_str())
            .expect("write a component id");
        let Json::Object(mut props) = serde_json::to_value(component).expect("a component") else {
            unreachable!("a component serialises to an object")
        };
        props.remove("root");
        meta.insert("props", Json::Object(props).to_string())
            .expect("write a component");
        self.roots.insert(Root::Component(id.clone()), root);
        self.create(root, 0, &component.root);
    }

    fn set_properties(&mut self, node: &Node) {
        let meta = self.meta(self.nodes[node.id()]);
        let now = read_map(&meta);
        let next = properties(node);
        for key in now.keys() {
            if !next.contains_key(key) {
                meta.delete(key).expect("delete a property");
            }
        }
        for (key, value) in next {
            if now.get(&key) != Some(&value) {
                meta.insert(&key, value).expect("write a property");
            }
        }
    }

    /// Writes `edits` (with `inverse` as `apply_edits` returned it) as one Loro change carrying `origin`.
    pub fn write(&mut self, edits: &[Edit], inverse: &[Edit], origin: &Origin) {
        if edits.is_empty() {
            return;
        }
        for (i, edit) in edits.iter().enumerate() {
            match edit {
                Edit::Insert { to, node } => {
                    let parent = self.parent(&to.parent);
                    self.create(parent, to.index, node);
                }
                Edit::Remove { id } => {
                    let tree_id = self.nodes[id];
                    self.delete(tree_id);
                }
                Edit::Move { id, to } => {
                    let parent = self.parent(&to.parent);
                    self.tree
                        .mov_to(self.nodes[id], parent, to.index)
                        .expect("the move was checked by the op layer");
                }
                Edit::Set { node } => self.set_properties(node),
                Edit::SetComponent { id, component } => {
                    let old = inverse
                        .len()
                        .checked_sub(i + 1)
                        .and_then(|j| inverse.get(j))
                        .and_then(|e| match e {
                            Edit::SetComponent { component, .. } => component.as_deref(),
                            _ => None,
                        });
                    match (old, component.as_deref()) {
                        (Some(old), Some(new)) if old.root == new.root => {
                            let root = self.roots[&Root::Component(id.clone())];
                            let Json::Object(mut props) =
                                serde_json::to_value(new).expect("a component")
                            else {
                                unreachable!("a component serialises to an object")
                            };
                            props.remove("root");
                            self.meta(root)
                                .insert("props", Json::Object(props).to_string())
                                .expect("write a component");
                        }
                        (_, new) => {
                            if let Some(root) = self.roots.remove(&Root::Component(id.clone())) {
                                self.delete(root);
                            }
                            if let Some(new) = new {
                                self.set_component(id, new);
                            }
                        }
                    }
                }
                Edit::SetVariable { id, variable } => {
                    let map = self.doc.get_map(VARIABLES);
                    match variable {
                        Some(v) => map.insert(id.as_str(), json_string(v)),
                        None => map.delete(id.as_str()),
                    }
                    .expect("write a variable");
                }
            }
        }
        self.doc.set_next_commit_message(&json_string(origin));
        self.doc.commit();
    }

    fn read_node(&self, id: TreeID) -> Node {
        let mut object = Map::new();
        for (key, value) in read_map(&self.meta(id)) {
            object.insert(key, parse(&value));
        }
        let mut node: Node =
            serde_json::from_value(Json::Object(object)).expect("the adapter wrote this node");
        let children = self.tree.children(id).unwrap_or_default();
        if let Some(slot) = node.children_mut() {
            *slot = children.into_iter().map(|c| self.read_node(c)).collect();
        }
        node
    }

    /// The document as Loro holds it.
    pub fn read(&self) -> Document {
        let shell = self.doc.get_map(SHELL);
        let shell = match shell.get("shell") {
            Some(ValueOrContainer::Value(value)) => string(&value).map(parse),
            _ => None,
        }
        .expect("the genesis wrote the shell");
        let mut document: Document =
            serde_json::from_value(shell).expect("the adapter wrote this shell");
        for root in self.tree.children(TreeParentId::Root).unwrap_or_default() {
            let meta = read_map(&self.meta(root));
            let kids = self.tree.children(root).unwrap_or_default();
            if let Some(page) = meta.get("page") {
                let mut page: Page =
                    serde_json::from_str(page).expect("the adapter wrote this page");
                page.children = kids.into_iter().map(|c| self.read_node(c)).collect();
                document.pages.push(page);
            } else if let (Some(id), Some(props), Some(first)) =
                (meta.get("component"), meta.get("props"), kids.first())
            {
                let id = ComponentId::new(id.clone()).expect("the adapter wrote this id");
                let mut props = parse(props);
                props["root"] = serde_json::to_value(self.read_node(*first)).expect("a node");
                let component: Component =
                    serde_json::from_value(props).expect("the adapter wrote this component");
                document.components.insert(id, component);
            }
        }
        for (id, variable) in read_map(&self.doc.get_map(VARIABLES)) {
            let id = vellum_types::VariableId::new(id).expect("the adapter wrote this id");
            let variable =
                serde_json::from_str(&variable).expect("the adapter wrote this variable");
            document.variables.insert(id, variable);
        }
        document
    }

    /// Rebuilds the maps from our ids to tree nodes, after changes from elsewhere.
    pub fn index(&mut self) {
        self.nodes.clear();
        self.ids.clear();
        self.roots.clear();
        for root in self.tree.children(TreeParentId::Root).unwrap_or_default() {
            let meta = read_map(&self.meta(root));
            if let Some(page) = meta.get("page") {
                let page: Page = serde_json::from_str(page).expect("the adapter wrote this page");
                self.roots.insert(Root::Page(page.id), root);
            } else if let Some(id) = meta.get("component") {
                let id = ComponentId::new(id.clone()).expect("the adapter wrote this id");
                self.roots.insert(Root::Component(id), root);
            }
        }
        let mut stack = self.tree.children(TreeParentId::Root).unwrap_or_default();
        while let Some(id) = stack.pop() {
            if let Some(node) = self.meta(id).get("id").and_then(|v| match v {
                ValueOrContainer::Value(value) => string(&value).map(parse),
                _ => None,
            }) {
                let node = NodeId::new(node.as_str().expect("a string id").to_string())
                    .expect("the adapter wrote this id");
                self.nodes.insert(node.clone(), id);
                self.ids.insert(id, node);
            }
            stack.extend(self.tree.children(id).unwrap_or_default());
        }
    }

    /// Every live tree node that holds a document node, by our id. For the adapter's own tests.
    #[cfg(test)]
    pub fn mapped(&self) -> Vec<(NodeId, String)> {
        let mut out: Vec<_> = self
            .nodes
            .iter()
            .map(|(node, tree)| {
                let id = self.meta(*tree).get("id").and_then(|v| match v {
                    ValueOrContainer::Value(value) => string(&value).map(str::to_string),
                    _ => None,
                });
                (node.clone(), id.unwrap_or_default())
            })
            .collect();
        out.sort();
        out
    }

    /// How many live tree nodes there are. For the adapter's own tests.
    #[cfg(test)]
    pub fn tree_len(&self) -> usize {
        self.tree
            .nodes()
            .into_iter()
            .filter(|id| !self.tree.is_node_deleted(id).unwrap_or(true))
            .count()
    }

    pub fn cursor(&self) -> Vec<u8> {
        self.doc.oplog_vv().encode()
    }

    pub fn export(&self, since: &[u8]) -> Vec<u8> {
        let from = if since.is_empty() {
            VersionVector::default()
        } else {
            VersionVector::decode(since).unwrap_or_default()
        };
        self.doc
            .export(ExportMode::updates(&from))
            .expect("export updates")
    }

    /// Imports updates and returns the origin of the newest change they carried, if any arrived.
    pub fn import(&mut self, bytes: &[u8]) -> Result<Option<Origin>, SyncError> {
        let meta = LoroDoc::decode_import_blob_meta(bytes, true)
            .map_err(|e| SyncError::Malformed(e.to_string()))?;
        if meta
            .partial_end_vv
            .iter()
            .any(|(peer, _)| peer & GENESIS_BIT != 0 && *peer != self.genesis)
        {
            return Err(SyncError::DifferentDocument);
        }
        let status = self
            .doc
            .import(bytes)
            .map_err(|e| SyncError::Malformed(e.to_string()))?;
        let newest = status
            .success
            .iter()
            .filter(|(_, (start, end))| end > start)
            .filter_map(|(peer, (_, end))| self.doc.get_change(loro::ID::new(*peer, end - 1)))
            .filter(|change| change.message.is_some())
            .max_by_key(|change| (change.lamport, change.id.peer));
        Ok(newest.and_then(|change| serde_json::from_str(change.message.as_deref()?).ok()))
    }
}
