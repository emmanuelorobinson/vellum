//! What a change touched, at the grain undo needs: node properties one by one, nodes made, removed and
//! moved, the containers whose children changed, variables and components.

use std::collections::{BTreeMap, BTreeSet};

use vellum_types::{ComponentId, Document, Node, NodeId, Origin, PageId, VariableId};

use crate::ops::Edit;
use crate::ops::edit::subtree_ids;
use crate::ops::{Parent, Place};
use crate::ports::ChangeSet;

/// What one change (a transaction, an undo, an import) touched.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Footprint {
    /// Node properties it set, by node, in the file's spelling.
    pub props: BTreeMap<NodeId, BTreeSet<String>>,
    /// Nodes it made.
    pub inserted: BTreeSet<NodeId>,
    /// Nodes it removed.
    pub removed: BTreeSet<NodeId>,
    /// Nodes it moved.
    pub moved: BTreeSet<NodeId>,
    /// Nodes whose children it added, removed or moved.
    pub parents: BTreeSet<NodeId>,
    /// Variables it set or removed.
    pub variables: BTreeSet<VariableId>,
    /// Components it set or removed.
    pub components: BTreeSet<ComponentId>,
}

/// The properties that differ between two versions of one node, children aside.
pub(crate) fn changed_keys(before: &Node, after: &Node) -> BTreeSet<String> {
    let (before, after) = (before.property_patch(), after.property_patch());
    let (before, after) = (before.properties(), after.properties());
    before
        .keys()
        .chain(after.keys())
        .filter(|key| before.get(*key) != after.get(*key))
        .cloned()
        .collect()
}

fn ids(node: &Node) -> Vec<NodeId> {
    let mut out = Vec::new();
    subtree_ids(node, &mut out);
    out
}

impl Footprint {
    /// The footprint of `edits`, given `inverse` as `apply_edits` returned it (the inverse of `edits[i]` is
    /// `inverse[len - 1 - i]`).
    pub fn of(edits: &[Edit], inverse: &[Edit]) -> Self {
        let mut out = Self::default();
        let paired = |i: usize| {
            inverse
                .len()
                .checked_sub(i + 1)
                .and_then(|j| inverse.get(j))
        };
        for (i, edit) in edits.iter().enumerate() {
            let inv = paired(i);
            match edit {
                Edit::Insert { to, node } => {
                    out.inserted.extend(ids(node));
                    out.parent(to);
                }
                Edit::Remove { id } => match inv {
                    Some(Edit::Insert { to, node }) => {
                        out.removed.extend(ids(node));
                        out.parent(to);
                    }
                    _ => {
                        out.removed.insert(id.clone());
                    }
                },
                Edit::Move { id, to } => {
                    out.moved.insert(id.clone());
                    out.parent(to);
                    if let Some(Edit::Move { to: from, .. }) = inv {
                        out.parent(from);
                    }
                }
                Edit::Set { node: after } => {
                    if let Some(Edit::Set { node: before }) = inv {
                        let keys = changed_keys(before, after);
                        if !keys.is_empty() {
                            out.props
                                .entry(after.id().clone())
                                .or_default()
                                .extend(keys);
                        }
                    }
                }
                Edit::SetComponent { id, component } => {
                    out.components.insert(id.clone());
                    let old = match inv {
                        Some(Edit::SetComponent { component, .. }) => component.as_deref(),
                        _ => None,
                    };
                    let new = component.as_deref();
                    if old.map(|c| &c.root) != new.map(|c| &c.root) {
                        if let Some(old) = old {
                            out.removed.extend(ids(&old.root));
                        }
                        if let Some(new) = new {
                            out.inserted.extend(ids(&new.root));
                        }
                    }
                }
                Edit::SetVariable { id, .. } => {
                    out.variables.insert(id.clone());
                }
            }
        }
        out
    }

    fn parent(&mut self, place: &Place) {
        if let Parent::Node(id) = &place.parent {
            self.parents.insert(id.clone());
        }
    }

    /// Adds what `other` touched.
    pub fn absorb(&mut self, other: &Footprint) {
        for (id, keys) in &other.props {
            self.props
                .entry(id.clone())
                .or_default()
                .extend(keys.iter().cloned());
        }
        self.inserted.extend(other.inserted.iter().cloned());
        self.removed.extend(other.removed.iter().cloned());
        self.moved.extend(other.moved.iter().cloned());
        self.parents.extend(other.parents.iter().cloned());
        self.variables.extend(other.variables.iter().cloned());
        self.components.extend(other.components.iter().cloned());
    }

    /// Whether the change did anything to the node: set a property, made, removed or moved it, or changed its
    /// children.
    pub fn touches_node(&self, id: &NodeId) -> bool {
        self.props.contains_key(id)
            || self.inserted.contains(id)
            || self.removed.contains(id)
            || self.moved.contains(id)
            || self.parents.contains(id)
    }

    /// What changed from `before` to `after`, found by comparing them: for changes that arrive as a new state
    /// (sync) rather than as edits.
    pub fn between(before: &Document, after: &Document, origin: Origin) -> (Self, ChangeSet) {
        let (old, new) = (Index::of(before), Index::of(after));
        let (old_rank, new_rank) = (old.ranks(&new), new.ranks(&old));
        let mut out = Self::default();
        let mut updated = BTreeSet::new();
        for (id, was) in &old.at {
            match new.at.get(id) {
                Some(now) if now.node.kind() == was.node.kind() => {
                    if was.node != now.node {
                        let keys = changed_keys(was.node, now.node);
                        if !keys.is_empty() {
                            updated.insert(id.clone());
                            out.props.insert(id.clone(), keys);
                        }
                    }
                    if was.holder != now.holder || old_rank.get(id) != new_rank.get(id) {
                        updated.insert(id.clone());
                        out.moved.insert(id.clone());
                        out.holder(&was.holder);
                        out.holder(&now.holder);
                    }
                }
                Some(now) => {
                    // A node replaced by one of another kind: removed and made again.
                    out.removed.insert(id.clone());
                    out.inserted.insert(id.clone());
                    updated.insert(id.clone());
                    out.holder(&was.holder);
                    out.holder(&now.holder);
                }
                None => {
                    out.removed.insert(id.clone());
                    out.holder(&was.holder);
                }
            }
        }
        for (id, now) in &new.at {
            if !old.at.contains_key(id) {
                out.inserted.insert(id.clone());
                out.holder(&now.holder);
            }
        }
        let keys: BTreeSet<_> = before
            .variables
            .keys()
            .chain(after.variables.keys())
            .collect();
        out.variables = keys
            .into_iter()
            .filter(|id| before.variables.get(*id) != after.variables.get(*id))
            .cloned()
            .collect();
        let keys: BTreeSet<_> = before
            .components
            .keys()
            .chain(after.components.keys())
            .collect();
        out.components = keys
            .into_iter()
            .filter(|id| before.components.get(*id) != after.components.get(*id))
            .cloned()
            .collect();
        let inserted: Vec<_> = out
            .inserted
            .iter()
            .filter(|id| !old.at.contains_key(*id))
            .cloned()
            .collect();
        let removed: Vec<_> = out
            .removed
            .iter()
            .filter(|id| !new.at.contains_key(*id))
            .cloned()
            .collect();
        let changes = ChangeSet {
            origin,
            inserted,
            updated: updated.into_iter().collect(),
            removed,
            variables: out.variables.iter().cloned().collect(),
        };
        (out, changes)
    }

    fn holder(&mut self, holder: &Holder) {
        if let Holder::Node(id) = holder {
            self.parents.insert(id.clone());
        }
    }
}

/// What holds a node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Holder {
    Page(PageId),
    Node(NodeId),
    Component(ComponentId),
}

struct At<'a> {
    holder: Holder,
    node: &'a Node,
}

/// Every node of a document by id, with what holds it, and each holder's children in order.
struct Index<'a> {
    at: BTreeMap<NodeId, At<'a>>,
    children: BTreeMap<Holder, Vec<&'a NodeId>>,
}

impl<'a> Index<'a> {
    fn of(doc: &'a Document) -> Self {
        let mut index = Self {
            at: BTreeMap::new(),
            children: BTreeMap::new(),
        };
        for page in &doc.pages {
            index.add(Holder::Page(page.id.clone()), &page.children);
        }
        for (id, component) in &doc.components {
            index.add(
                Holder::Component(id.clone()),
                std::slice::from_ref(&component.root),
            );
        }
        index
    }

    fn add(&mut self, holder: Holder, nodes: &'a [Node]) {
        self.children
            .insert(holder.clone(), nodes.iter().map(Node::id).collect());
        for node in nodes {
            self.at.insert(
                node.id().clone(),
                At {
                    holder: holder.clone(),
                    node,
                },
            );
            if !node.children().is_empty() {
                self.add(Holder::Node(node.id().clone()), node.children());
            }
        }
    }

    /// Each node's place among the siblings it has under the same holder in `other` too, so that a sibling
    /// coming or going does not count as a move.
    fn ranks(&self, other: &Index<'_>) -> BTreeMap<NodeId, usize> {
        let mut out = BTreeMap::new();
        for (holder, kids) in &self.children {
            let staying = kids
                .iter()
                .filter(|id| other.at.get(**id).is_some_and(|at| &at.holder == holder));
            for (rank, id) in staying.enumerate() {
                out.insert((*id).clone(), rank);
            }
        }
        out
    }
}
