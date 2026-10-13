//! Diff: the ops that turn one document into another, by node id and property (ARCHITECTURE §4.5).
//!
//! An external change (a `git checkout`, a merge, an agent writing the file) reaches the open document as these
//! ops, through the one write path like any other change (Invariant 1). Ids are stable, so a node with the same
//! id, kind and tree in both documents is the same node: its changed properties become one `Update`, a new
//! parent a `Move`, a new place among its siblings a `Reorder`. Only nodes that are new, gone, or changed kind or
//! tree (between the pages and a component, or between components) are inserted and deleted, and then as whole
//! subtrees.
//!
//! The ops come in an order every intermediate step of which the op layer accepts, which `plan` checks;
//! references (instances, override paths, slots) are checked once, at the end. The order:
//!
//! 1. the shell; new pages (empty, at the end); pages that become canvases (they hold anything);
//! 2. variables added or changed; component names, props and variants;
//! 3. property updates;
//! 4. nodes removed whose ids come back elsewhere (they must go before those ids can be inserted again);
//! 5. components removed or replaced, then components added or replaced;
//! 6. new subtrees, without the nodes that live on inside them;
//! 7. nodes that live on, moved to their new parents in the new document's order, parents first;
//! 8. the remaining removals, once what lives on has moved out;
//! 9. each parent's children put in order;
//! 10. pages that become slides, removed pages, page order; removed variables.
//!
//! The reference read for this (guide §3.2, Prompt 15) is Graphite's `document_diff.rs`; see ADR-010.

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;
use vellum_types::{ComponentId, Document, Node, NodeId, NodePatch, PageId, PageKind};

use crate::ops::edit::{all_ids, bare, subtree_ids};
use crate::ops::{Draft, Op, OpError, OpErrorKind, Parent, Place, Shell, Tree, find, locate};

/// The ops that turn `old` into `new`, in an order `plan` accepts on `old`. Empty when they are equal.
///
/// The ops are checked one by one as they are made; an error means `new` is not a document the op layer could
/// reach (an id used twice, a node where its parent cannot hold it), and names the op that could not be made. References are
/// not checked here: planning the ops checks them, and refuses a `new` whose instances name a missing component.
pub fn diff(old: &Document, new: &Document) -> Result<Vec<Op>, OpError> {
    // Ids are what the diff goes by; a new document that uses one twice has no answer.
    let mut seen = BTreeSet::new();
    if let Some(twice) = all_ids(new).into_iter().find(|id| !seen.insert(id.clone())) {
        return Err(OpError {
            op_index: 0,
            path: String::new(),
            kind: OpErrorKind::DuplicateId(twice),
        });
    }
    let mut draft = Draft::new(old);

    // Components whose tree is diffed node by node: in both, with the same root. The rest are set whole.
    let kept_components: BTreeSet<ComponentId> = old
        .components
        .iter()
        .filter(|(id, c)| {
            new.components
                .get(*id)
                .is_some_and(|n| n.root.id() == c.root.id() && n.root.kind() == c.root.kind())
        })
        .map(|(id, _)| id.clone())
        .collect();
    let was = Index::of(old, &kept_components);
    let now = Index::of(new, &kept_components);
    let new_ids: BTreeSet<NodeId> = all_ids(new).into_iter().collect();

    // Kept: the same id, kind and tree in both. A node in a subtree removed early is removed with it, and
    // made again.
    let mut kept: BTreeSet<NodeId> = was
        .at
        .iter()
        .filter(|(id, a)| {
            now.at
                .get(*id)
                .is_some_and(|b| b.node.kind() == a.node.kind() && b.home == a.home)
        })
        .map(|(id, _)| id.clone())
        .collect();
    let mut early = BTreeSet::new();
    let mut doomed = BTreeSet::new();
    for id in &was.order {
        let at = &was.at[id];
        let parent_doomed = matches!(&at.holder, Holder::Node(p) if doomed.contains(p));
        if parent_doomed {
            kept.remove(id);
        }
        if !kept.contains(id) && owned(at.node).iter().any(|i| new_ids.contains(i)) {
            early.insert(id.clone());
        }
        if parent_doomed || early.contains(id) {
            doomed.insert(id.clone());
        }
    }

    // 1. The shell, new pages, pages that become canvases.
    let shell = Shell::of(new);
    if Shell::of(old) != shell {
        draft.push(Op::SetShell {
            shell: Box::new(shell),
        })?;
    }
    for page in &new.pages {
        match old.pages.iter().find(|p| p.id == page.id) {
            None => draft.push(Op::InsertPage {
                index: draft.doc().pages.len(),
                page: Box::new(bare(page)),
            })?,
            Some(was) if page_changed(was, page) && page.kind == PageKind::Canvas => {
                draft.push(update_page(page))?
            }
            Some(_) => {}
        }
    }

    // 2. Variables added or changed; component definitions.
    let set: BTreeMap<_, _> = new
        .variables
        .iter()
        .filter(|(id, v)| old.variables.get(*id) != Some(*v))
        .map(|(id, v)| (id.clone(), v.clone()))
        .collect();
    if !set.is_empty() {
        draft.push(Op::SetVariables {
            set,
            remove: BTreeSet::new(),
        })?;
    }
    for id in &kept_components {
        let (a, b) = (&old.components[id], &new.components[id]);
        if (&a.name, &a.props, &a.variants) != (&b.name, &b.props, &b.variants) {
            draft.push(Op::UpdateComponent {
                id: id.clone(),
                name: b.name.clone(),
                props: b.props.clone(),
                variants: b.variants.clone(),
            })?;
        }
    }

    // 3. Properties.
    for id in &now.order {
        if kept.contains(id)
            && let Some(patch) = patch_between(was.at[id].node, now.at[id].node)
        {
            draft.push(Op::Update {
                id: id.clone(),
                patch,
            })?;
        }
    }

    // 4. Removals whose ids come back.
    for id in &was.order {
        let parent_doomed = matches!(&was.at[id].holder, Holder::Node(p) if doomed.contains(p));
        if early.contains(id) && !parent_doomed {
            draft.push(Op::Delete { id: id.clone() })?;
        }
    }

    // 5. Whole components.
    for id in old.components.keys() {
        if !kept_components.contains(id) {
            draft.push(Op::SetComponent {
                id: id.clone(),
                component: None,
            })?;
        }
    }
    for (id, component) in &new.components {
        if !kept_components.contains(id) {
            draft.push(Op::SetComponent {
                id: id.clone(),
                component: Some(Box::new(component.clone())),
            })?;
        }
    }

    // 6. New subtrees, under a parent that is already there.
    for id in &now.order {
        let at = &now.at[id];
        let parent_new = matches!(&at.holder, Holder::Node(p) if !kept.contains(p));
        if kept.contains(id) || parent_new {
            continue;
        }
        let parent = at.holder.parent();
        let index = after_sibling(draft.doc(), &parent, &now.kids[&at.holder][..at.index]);
        draft.push(Op::Insert {
            to: Place { parent, index },
            node: Box::new(without(at.node, &kept)),
        })?;
    }

    // 7. What lives on, to its new parent. In the new document's order, so a parent is in place before its
    // children arrive, and nothing moves into its own subtree.
    for id in &now.order {
        let at = &now.at[id];
        if !kept.contains(id) || matches!(at.holder, Holder::Component(_)) {
            continue;
        }
        let parent = at.holder.parent();
        let here = locate(draft.doc(), id).expect("a kept node is in the draft");
        if here.place().parent != parent {
            let index = child_ids(draft.doc(), &parent).len();
            draft.push(Op::Move {
                id: id.clone(),
                to: Place { parent, index },
            })?;
        }
    }

    // 8. The other removals, but not of what a removed page takes with it.
    let removed_pages: BTreeSet<&PageId> = old
        .pages
        .iter()
        .filter(|p| !new.pages.iter().any(|n| n.id == p.id))
        .map(|p| &p.id)
        .collect();
    for id in &was.order {
        if kept.contains(id) || doomed.contains(id) {
            continue;
        }
        let Some(here) = locate(draft.doc(), id) else {
            continue;
        };
        if matches!(&here.tree, Tree::Page(page) if removed_pages.contains(page)) {
            continue;
        }
        draft.push(Op::Delete { id: id.clone() })?;
    }

    // 9. Order among siblings.
    for (holder, want) in &now.kids {
        if want.len() < 2 || matches!(holder, Holder::Component(_)) {
            continue;
        }
        let mut have = child_ids(draft.doc(), &holder.parent());
        debug_assert_eq!(
            have.iter().collect::<BTreeSet<_>>(),
            want.iter().collect::<BTreeSet<_>>(),
            "the children of {holder:?} are not the ones the new document has"
        );
        for (index, id) in want.iter().enumerate() {
            if &have[index] != id {
                draft.push(Op::Reorder {
                    id: id.clone(),
                    index,
                })?;
                let from = have.iter().position(|h| h == id).expect("a child");
                let moved = have.remove(from);
                have.insert(index, moved);
            }
        }
    }

    // 10. Pages that become slides, removed pages, page order, removed variables.
    for page in &new.pages {
        if let Some(was) = old.pages.iter().find(|p| p.id == page.id)
            && page_changed(was, page)
            && page.kind != PageKind::Canvas
        {
            draft.push(update_page(page))?;
        }
    }
    for id in removed_pages {
        draft.push(Op::DeletePage { id: id.clone() })?;
    }
    for (index, page) in new.pages.iter().enumerate() {
        if draft.doc().pages[index].id != page.id {
            draft.push(Op::MovePage {
                id: page.id.clone(),
                index,
            })?;
        }
    }
    let remove: BTreeSet<_> = old
        .variables
        .keys()
        .filter(|id| !new.variables.contains_key(*id))
        .cloned()
        .collect();
    if !remove.is_empty() {
        draft.push(Op::SetVariables {
            set: BTreeMap::new(),
            remove,
        })?;
    }

    debug_assert_eq!(draft.doc(), new, "the diff does not reach the new document");
    Ok(draft.into_ops())
}

/// What holds a node in one document.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Holder {
    Page(PageId),
    Node(NodeId),
    /// The node is a component's root.
    Component(ComponentId),
}

impl Holder {
    /// The place a node goes under this holder. Never asked of a component root, which does not move.
    fn parent(&self) -> Parent {
        match self {
            Self::Page(page) => Parent::Page(page.clone()),
            Self::Node(node) => Parent::Node(node.clone()),
            Self::Component(_) => unreachable!("a component root has no parent to move to"),
        }
    }
}

/// Which tree a node is in: nodes move freely between pages, but not into or out of a component.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Home {
    Pages,
    Component(ComponentId),
}

struct At<'a> {
    home: Home,
    holder: Holder,
    index: usize,
    node: &'a Node,
}

/// The nodes of the pages and of the components diffed node by node: where each is, in tree order, and each
/// holder's children.
struct Index<'a> {
    at: BTreeMap<NodeId, At<'a>>,
    order: Vec<NodeId>,
    kids: BTreeMap<Holder, Vec<NodeId>>,
}

impl<'a> Index<'a> {
    fn of(doc: &'a Document, components: &BTreeSet<ComponentId>) -> Self {
        let mut index = Self {
            at: BTreeMap::new(),
            order: Vec::new(),
            kids: BTreeMap::new(),
        };
        for page in &doc.pages {
            index.add(&Home::Pages, Holder::Page(page.id.clone()), &page.children);
        }
        for id in components {
            let root = &doc.components[id].root;
            index.add(
                &Home::Component(id.clone()),
                Holder::Component(id.clone()),
                std::slice::from_ref(root),
            );
        }
        index
    }

    fn add(&mut self, home: &Home, holder: Holder, nodes: &'a [Node]) {
        self.kids.insert(
            holder.clone(),
            nodes.iter().map(|n| n.id().clone()).collect(),
        );
        for (index, node) in nodes.iter().enumerate() {
            self.at.insert(
                node.id().clone(),
                At {
                    home: home.clone(),
                    holder: holder.clone(),
                    index,
                    node,
                },
            );
            self.order.push(node.id().clone());
            if !node.children().is_empty() {
                self.add(home, Holder::Node(node.id().clone()), node.children());
            }
        }
    }
}

/// The ids a node holds itself: its own, and those in an instance's overrides and slot content (not its
/// children, which are nodes of their own).
fn owned(node: &Node) -> Vec<NodeId> {
    let mut all = Vec::new();
    subtree_ids(node, &mut all);
    let mut children = Vec::new();
    node.children()
        .iter()
        .for_each(|c| subtree_ids(c, &mut children));
    let children: BTreeSet<_> = children.into_iter().collect();
    all.into_iter().filter(|i| !children.contains(i)).collect()
}

/// `node` with only the children that are new too, all the way down.
fn without(node: &Node, kept: &BTreeSet<NodeId>) -> Node {
    let mut copy = node.clone();
    if let Some(children) = copy.children_mut() {
        *children = node
            .children()
            .iter()
            .filter(|c| !kept.contains(c.id()))
            .map(|c| without(c, kept))
            .collect();
    }
    copy
}

/// The properties that differ, with `null` for one back at its default. None when none differ.
fn patch_between(before: &Node, after: &Node) -> Option<NodePatch> {
    let (before, after) = (before.property_patch(), after.property_patch());
    let (before, after) = (before.properties(), after.properties());
    let changed: BTreeMap<String, Json> = before
        .keys()
        .chain(after.keys())
        .filter(|key| before.get(*key) != after.get(*key))
        .map(|key| (key.clone(), after.get(key).cloned().unwrap_or(Json::Null)))
        .collect();
    (!changed.is_empty()).then(|| NodePatch::new(changed).expect("properties of a node"))
}

fn page_changed(a: &vellum_types::Page, b: &vellum_types::Page) -> bool {
    a.name != b.name || a.kind != b.kind
}

fn update_page(page: &vellum_types::Page) -> Op {
    Op::UpdatePage {
        id: page.id.clone(),
        name: page.name.clone(),
        kind: page.kind,
    }
}

/// The index just after the last of `before` (siblings that come first in the new document) that is already
/// under `parent`, or 0. Saves a reorder when the siblings are already in order.
fn after_sibling(doc: &Document, parent: &Parent, before: &[NodeId]) -> usize {
    let have = child_ids(doc, parent);
    before
        .iter()
        .rev()
        .find_map(|id| have.iter().position(|h| h == id))
        .map_or(0, |at| at + 1)
}

/// The ids of the children under `parent` in `doc`, in order.
fn child_ids(doc: &Document, parent: &Parent) -> Vec<NodeId> {
    let children = match parent {
        Parent::Page(page) => doc
            .pages
            .iter()
            .find(|p| &p.id == page)
            .map(|p| p.children.as_slice()),
        Parent::Node(node) => find(doc, node).map(Node::children),
    };
    children
        .unwrap_or_default()
        .iter()
        .map(|n| n.id().clone())
        .collect()
}
