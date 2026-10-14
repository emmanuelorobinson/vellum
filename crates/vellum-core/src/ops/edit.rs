//! Edits: the primitive changes every op lowers to and every store applies.
//!
//! An edit checks the shape of the tree itself (the target exists, the parent can hold the child, ids stay
//! unique, nothing moves into itself or across a page/component boundary) and returns its exact inverse, so
//! undo, redo and sync replay edits without going around those checks. What an op means (patches, variables,
//! wrappers) is checked one level up, in `plan`. Component references are also checked after replay.

use std::collections::BTreeSet;

use vellum_types::{
    Component, ComponentId, Document, Node, NodeId, NodeKind, Origin, Page, PageId, PageKind,
    Variable, VariableId,
};

use super::{OpErrorKind, Parent, Place, Shell};
use crate::ports::ChangeSet;

/// One primitive change.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// Adds a node and its subtree.
    Insert {
        /// Where it goes.
        to: Place,
        /// The node; no id in it may exist yet.
        node: Box<Node>,
    },
    /// Removes a node and its subtree.
    Remove {
        /// The node.
        id: NodeId,
    },
    /// Moves a node to `to.index` among the destination's children, counted after it has left its old place.
    Move {
        /// The node.
        id: NodeId,
        /// Where it ends up.
        to: Place,
    },
    /// Replaces a node's properties. Same id, same kind; the node keeps the children it has.
    Set {
        /// The node as it should read.
        node: Box<Node>,
    },
    /// Sets or removes a component definition. Node ids are checked against the rest of the document.
    SetComponent {
        /// The component id.
        id: ComponentId,
        /// Definition, or removal.
        component: Option<Box<Component>>,
    },
    /// Sets a variable, or removes it with `None`.
    SetVariable {
        /// The variable.
        id: VariableId,
        /// Its new definition.
        variable: Option<Box<Variable>>,
    },
    /// Adds a page with its nodes at `index` among the pages.
    InsertPage {
        /// Where among the pages.
        index: usize,
        /// The page; no id in it may exist yet.
        page: Box<Page>,
    },
    /// Removes a page and its nodes.
    RemovePage {
        /// The page.
        id: PageId,
    },
    /// Sets a page's name and kind. Same id; the page keeps the nodes it has.
    SetPage {
        /// The page as it should read; its children are ignored.
        page: Box<Page>,
    },
    /// Moves a page to `index` among the pages, counted after it has left its old place.
    MovePage {
        /// The page.
        id: PageId,
        /// Where it ends up.
        index: usize,
    },
    /// Sets the meta block, imports, themes, styles and fonts.
    SetShell {
        /// The shell as it should read.
        shell: Box<Shell>,
    },
}

/// A page without its nodes.
pub(crate) fn bare(page: &Page) -> Page {
    Page {
        id: page.id.clone(),
        name: page.name.clone(),
        kind: page.kind,
        children: Vec::new(),
    }
}

fn page_index(doc: &Document, id: &PageId) -> Result<usize, OpErrorKind> {
    doc.pages
        .iter()
        .position(|p| &p.id == id)
        .ok_or_else(|| OpErrorKind::UnknownPage(id.clone()))
}

/// What holds a node: a page of some kind, or a node of some kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// The top level of a page.
    Page(PageKind),
    /// A container node.
    Node(NodeKind),
}

/// Whether `container` may hold a node of `kind`. Frames, groups and canvas pages hold anything; a boolean
/// holds what can be turned into a path; a slides page holds slides (frames) and annotations. A merge's
/// conflict node may stand wherever the node it replaces stood, and holds whatever that node held.
pub fn can_hold(container: Container, kind: NodeKind) -> bool {
    use NodeKind as K;
    match container {
        _ if kind == K::Conflict => true,
        Container::Page(PageKind::Canvas) | Container::Node(K::Frame | K::Group | K::Conflict) => {
            true
        }
        Container::Page(PageKind::Slides) => {
            matches!(kind, K::Frame | K::Note | K::Context | K::Prompt)
        }
        Container::Node(K::Boolean) => matches!(
            kind,
            K::Rect
                | K::Ellipse
                | K::Polygon
                | K::Star
                | K::Line
                | K::Vector
                | K::Boolean
                | K::Text
                | K::Group
        ),
        Container::Node(_) => false,
    }
}

/// Which tree a node lives in: a page, or a component's definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tree {
    /// A page.
    Page(PageId),
    /// A component's node tree.
    Component(ComponentId),
}

/// Where a node is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Its tree.
    pub tree: Tree,
    /// Its parent; none at the top of a page, and for a component's root.
    pub parent: Option<NodeId>,
    /// Its index among its siblings (0 for a component's root).
    pub index: usize,
}

impl Location {
    /// Whether this is a component's root, which only component ops may move or remove.
    pub fn is_component_root(&self) -> bool {
        self.parent.is_none() && matches!(self.tree, Tree::Component(_))
    }

    /// The place that puts a node back here.
    pub fn place(&self) -> Place {
        let parent = match (&self.parent, &self.tree) {
            (Some(parent), _) => Parent::Node(parent.clone()),
            (None, Tree::Page(page)) => Parent::Page(page.clone()),
            (None, Tree::Component(_)) => unreachable!("a component root has no place"),
        };
        Place {
            parent,
            index: self.index,
        }
    }
}

fn walk(nodes: &[Node], parent: Option<&NodeId>, id: &NodeId) -> Option<(Option<NodeId>, usize)> {
    for (index, node) in nodes.iter().enumerate() {
        if node.id() == id {
            return Some((parent.cloned(), index));
        }
        if let Some(found) = walk(node.children(), Some(node.id()), id) {
            return Some(found);
        }
    }
    None
}

/// Where `id` is, in a page or a component.
pub fn locate(doc: &Document, id: &NodeId) -> Option<Location> {
    for page in &doc.pages {
        if let Some((parent, index)) = walk(&page.children, None, id) {
            return Some(Location {
                tree: Tree::Page(page.id.clone()),
                parent,
                index,
            });
        }
    }
    for (component_id, component) in &doc.components {
        let tree = Tree::Component(component_id.clone());
        if component.root.id() == id {
            return Some(Location {
                tree,
                parent: None,
                index: 0,
            });
        }
        if let Some((parent, index)) =
            walk(component.root.children(), Some(component.root.id()), id)
        {
            return Some(Location {
                tree,
                parent,
                index,
            });
        }
    }
    None
}

fn find_in<'a>(nodes: &'a [Node], id: &NodeId) -> Option<&'a Node> {
    nodes.iter().find_map(|node| {
        if node.id() == id {
            Some(node)
        } else {
            find_in(node.children(), id)
        }
    })
}

fn find_in_mut<'a>(nodes: &'a mut [Node], id: &NodeId) -> Option<&'a mut Node> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Some(children) = node.children_mut()
            && let Some(found) = find_in_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}

/// The node with `id`, in a page or a component.
pub fn find<'a>(doc: &'a Document, id: &NodeId) -> Option<&'a Node> {
    doc.pages
        .iter()
        .find_map(|page| find_in(&page.children, id))
        .or_else(|| {
            doc.components
                .values()
                .find_map(|c| find_in(std::slice::from_ref(&c.root), id))
        })
}

pub(crate) fn find_mut<'a>(doc: &'a mut Document, id: &NodeId) -> Option<&'a mut Node> {
    if let Some(page) = doc
        .pages
        .iter_mut()
        .find(|page| find_in(&page.children, id).is_some())
    {
        return find_in_mut(&mut page.children, id);
    }
    doc.components
        .values_mut()
        .find_map(|c| find_in_mut(std::slice::from_mut(&mut c.root), id))
}

pub(crate) fn subtree_ids(node: &Node, out: &mut Vec<NodeId>) {
    out.push(node.id().clone());
    node.children().iter().for_each(|c| subtree_ids(c, out));
    if let Node::Instance(n) = node {
        for value in n.overrides.values() {
            match value {
                vellum_types::Override::Replace(node) => subtree_ids(node, out),
                vellum_types::Override::Children(nodes) => {
                    nodes.iter().for_each(|n| subtree_ids(n, out))
                }
                vellum_types::Override::Props(_) => {}
            }
        }
        for children in n.slot_content.values() {
            children.iter().for_each(|n| subtree_ids(n, out));
        }
    }
}

/// Every node id in the document's pages and components, in tree order.
pub(crate) fn all_ids(doc: &Document) -> Vec<NodeId> {
    let mut ids = Vec::new();
    doc.pages
        .iter()
        .flat_map(|page| &page.children)
        .for_each(|node| subtree_ids(node, &mut ids));
    doc.components
        .values()
        .for_each(|c| subtree_ids(&c.root, &mut ids));
    ids
}

/// Checks that `node`, and every node under it, may sit where it is.
pub(crate) fn check_kinds(node: &Node, container: Container) -> Result<(), OpErrorKind> {
    if !can_hold(container, node.kind()) {
        return Err(OpErrorKind::NotAllowedIn {
            id: node.id().clone(),
            kind: node.kind(),
            parent: container,
        });
    }
    node.children()
        .iter()
        .try_for_each(|child| check_kinds(child, Container::Node(node.kind())))
}

struct Target {
    tree: Tree,
    container: Container,
    parent: Option<NodeId>,
}

fn resolve(doc: &Document, parent: &Parent) -> Result<Target, OpErrorKind> {
    match parent {
        Parent::Page(page) => {
            let found = doc
                .pages
                .iter()
                .find(|p| &p.id == page)
                .ok_or_else(|| OpErrorKind::UnknownPage(page.clone()))?;
            Ok(Target {
                tree: Tree::Page(page.clone()),
                container: Container::Page(found.kind),
                parent: None,
            })
        }
        Parent::Node(id) => {
            let location = locate(doc, id).ok_or_else(|| OpErrorKind::UnknownNode(id.clone()))?;
            let node = find(doc, id).expect("located");
            if !matches!(
                node.kind(),
                NodeKind::Frame | NodeKind::Group | NodeKind::Boolean | NodeKind::Conflict
            ) {
                return Err(OpErrorKind::NotAContainer(id.clone()));
            }
            Ok(Target {
                tree: location.tree,
                container: Container::Node(node.kind()),
                parent: Some(id.clone()),
            })
        }
    }
}

fn siblings_mut<'a>(
    doc: &'a mut Document,
    tree: &Tree,
    parent: Option<&NodeId>,
) -> &'a mut Vec<Node> {
    match (parent, tree) {
        (Some(parent), _) => find_mut(doc, parent)
            .and_then(Node::children_mut)
            .expect("the parent was checked"),
        (None, Tree::Page(page)) => {
            &mut doc
                .pages
                .iter_mut()
                .find(|p| &p.id == page)
                .expect("the page was checked")
                .children
        }
        (None, Tree::Component(_)) => unreachable!("nothing sits beside a component root"),
    }
}

fn siblings_len(doc: &Document, target: &Target) -> usize {
    match &target.parent {
        Some(parent) => find(doc, parent).map_or(0, |n| n.children().len()),
        None => match &target.tree {
            Tree::Page(page) => doc
                .pages
                .iter()
                .find(|p| &p.id == page)
                .map_or(0, |p| p.children.len()),
            Tree::Component(_) => 0,
        },
    }
}

/// The ids, variables and order of what a run of edits touched; becomes a [`ChangeSet`].
#[derive(Debug, Default)]
pub(crate) struct Touched {
    inserted: BTreeSet<NodeId>,
    updated: BTreeSet<NodeId>,
    removed: BTreeSet<NodeId>,
    variables: BTreeSet<VariableId>,
    pages: BTreeSet<PageId>,
    components: BTreeSet<ComponentId>,
    shell: bool,
}

impl Touched {
    fn insert(&mut self, id: NodeId) {
        if self.removed.remove(&id) {
            self.updated.insert(id);
        } else {
            self.inserted.insert(id);
        }
    }

    fn remove(&mut self, id: NodeId) {
        self.updated.remove(&id);
        if !self.inserted.remove(&id) {
            self.removed.insert(id);
        }
    }

    fn update(&mut self, id: NodeId) {
        if !self.inserted.contains(&id) {
            self.updated.insert(id);
        }
    }

    pub(crate) fn into_change_set(self, origin: Origin) -> ChangeSet {
        ChangeSet {
            origin,
            inserted: self.inserted.into_iter().collect(),
            updated: self.updated.into_iter().collect(),
            removed: self.removed.into_iter().collect(),
            variables: self.variables.into_iter().collect(),
            pages: self.pages.into_iter().collect(),
            components: self.components.into_iter().collect(),
            shell: self.shell,
        }
    }
}

/// Applies one edit in place and returns its inverse. Every check runs before anything changes.
pub(crate) fn apply_one(
    doc: &mut Document,
    edit: &Edit,
    touched: &mut Touched,
) -> Result<Edit, OpErrorKind> {
    match edit {
        Edit::Insert { to, node } => {
            let target = resolve(doc, &to.parent)?;
            let len = siblings_len(doc, &target);
            if to.index > len {
                return Err(OpErrorKind::IndexOutOfRange {
                    index: to.index,
                    len,
                });
            }
            let mut new_ids = Vec::new();
            subtree_ids(node, &mut new_ids);
            let mut taken: BTreeSet<NodeId> = all_ids(doc).into_iter().collect();
            for id in &new_ids {
                if !taken.insert(id.clone()) {
                    return Err(OpErrorKind::DuplicateId(id.clone()));
                }
            }
            check_kinds(node, target.container)?;
            siblings_mut(doc, &target.tree, target.parent.as_ref())
                .insert(to.index, (**node).clone());
            new_ids.into_iter().for_each(|id| touched.insert(id));
            Ok(Edit::Remove {
                id: node.id().clone(),
            })
        }
        Edit::Remove { id } => {
            let location = locate(doc, id).ok_or_else(|| OpErrorKind::UnknownNode(id.clone()))?;
            if location.is_component_root() {
                return Err(OpErrorKind::ComponentRoot(id.clone()));
            }
            let node =
                siblings_mut(doc, &location.tree, location.parent.as_ref()).remove(location.index);
            let mut gone = Vec::new();
            subtree_ids(&node, &mut gone);
            gone.into_iter().for_each(|id| touched.remove(id));
            Ok(Edit::Insert {
                to: location.place(),
                node: Box::new(node),
            })
        }
        Edit::Move { id, to } => {
            let from = locate(doc, id).ok_or_else(|| OpErrorKind::UnknownNode(id.clone()))?;
            if from.is_component_root() {
                return Err(OpErrorKind::ComponentRoot(id.clone()));
            }
            let target = resolve(doc, &to.parent)?;
            let node = find(doc, id).expect("located");
            if let Parent::Node(parent) = &to.parent
                && find_in(std::slice::from_ref(node), parent).is_some()
            {
                return Err(OpErrorKind::Cycle(id.clone()));
            }
            let same_tree = match (&from.tree, &target.tree) {
                (Tree::Page(_), Tree::Page(_)) => true,
                (a, b) => a == b,
            };
            if !same_tree {
                return Err(OpErrorKind::CrossesBoundary(id.clone()));
            }
            if !can_hold(target.container, node.kind()) {
                return Err(OpErrorKind::NotAllowedIn {
                    id: id.clone(),
                    kind: node.kind(),
                    parent: target.container,
                });
            }
            let same_parent = from.tree == target.tree && from.parent == target.parent;
            let len = siblings_len(doc, &target) - usize::from(same_parent);
            if to.index > len {
                return Err(OpErrorKind::IndexOutOfRange {
                    index: to.index,
                    len,
                });
            }
            let node = siblings_mut(doc, &from.tree, from.parent.as_ref()).remove(from.index);
            siblings_mut(doc, &target.tree, target.parent.as_ref()).insert(to.index, node);
            touched.update(id.clone());
            Ok(Edit::Move {
                id: id.clone(),
                to: from.place(),
            })
        }
        Edit::Set { node } => {
            let current = find_mut(doc, node.id())
                .ok_or_else(|| OpErrorKind::UnknownNode(node.id().clone()))?;
            if current.kind() != node.kind() {
                return Err(OpErrorKind::WrongKind {
                    expected: current.kind(),
                    found: node.kind(),
                });
            }
            let mut next = (**node).clone();
            if let (Some(slot), Some(kids)) = (next.children_mut(), current.children_mut()) {
                *slot = std::mem::take(kids);
            }
            let before = std::mem::replace(current, next);
            touched.update(node.id().clone());
            Ok(Edit::Set {
                node: Box::new(before),
            })
        }
        Edit::SetComponent { id, component } => {
            if let Some(component) = component {
                if component.root.kind() != NodeKind::Frame {
                    return Err(OpErrorKind::WrongKind {
                        expected: NodeKind::Frame,
                        found: component.root.kind(),
                    });
                }
                let mut without = doc.clone();
                without.components.remove(id);
                let mut taken: BTreeSet<_> = all_ids(&without).into_iter().collect();
                let mut added = Vec::new();
                subtree_ids(&component.root, &mut added);
                for node_id in added {
                    if !taken.insert(node_id.clone()) {
                        return Err(OpErrorKind::DuplicateId(node_id));
                    }
                }
                check_kinds(&component.root, Container::Node(NodeKind::Frame))?;
            }
            let before = match component {
                Some(c) => doc.components.insert(id.clone(), (**c).clone()),
                None => Some(
                    doc.components
                        .remove(id)
                        .ok_or_else(|| OpErrorKind::UnknownComponent(id.clone()))?,
                ),
            };
            if let Some(c) = &before {
                let mut removed = Vec::new();
                subtree_ids(&c.root, &mut removed);
                for id in removed {
                    touched.remove(id);
                }
            }
            if let Some(c) = component {
                let mut added = Vec::new();
                subtree_ids(&c.root, &mut added);
                for id in added {
                    touched.insert(id);
                }
            }
            touched.components.insert(id.clone());
            Ok(Edit::SetComponent {
                id: id.clone(),
                component: before.map(Box::new),
            })
        }
        Edit::SetVariable { id, variable } => {
            let before = match variable {
                Some(variable) => doc.variables.insert(id.clone(), (**variable).clone()),
                None => Some(
                    doc.variables
                        .remove(id)
                        .ok_or_else(|| OpErrorKind::UnknownVariable(id.clone()))?,
                ),
            };
            touched.variables.insert(id.clone());
            Ok(Edit::SetVariable {
                id: id.clone(),
                variable: before.map(Box::new),
            })
        }
        Edit::InsertPage { index, page } => {
            if doc.pages.iter().any(|p| p.id == page.id) {
                return Err(OpErrorKind::DuplicatePage(page.id.clone()));
            }
            if *index > doc.pages.len() {
                return Err(OpErrorKind::IndexOutOfRange {
                    index: *index,
                    len: doc.pages.len(),
                });
            }
            let mut new_ids = Vec::new();
            page.children
                .iter()
                .for_each(|n| subtree_ids(n, &mut new_ids));
            let mut taken: BTreeSet<NodeId> = all_ids(doc).into_iter().collect();
            for id in &new_ids {
                if !taken.insert(id.clone()) {
                    return Err(OpErrorKind::DuplicateId(id.clone()));
                }
            }
            for node in &page.children {
                check_kinds(node, Container::Page(page.kind))?;
            }
            doc.pages.insert(*index, (**page).clone());
            new_ids.into_iter().for_each(|id| touched.insert(id));
            touched.pages.insert(page.id.clone());
            Ok(Edit::RemovePage {
                id: page.id.clone(),
            })
        }
        Edit::RemovePage { id } => {
            let index = page_index(doc, id)?;
            let page = doc.pages.remove(index);
            let mut gone = Vec::new();
            page.children.iter().for_each(|n| subtree_ids(n, &mut gone));
            gone.into_iter().for_each(|id| touched.remove(id));
            touched.pages.insert(id.clone());
            Ok(Edit::InsertPage {
                index,
                page: Box::new(page),
            })
        }
        Edit::SetPage { page } => {
            let index = page_index(doc, &page.id)?;
            for node in &doc.pages[index].children {
                check_kinds(node, Container::Page(page.kind))?;
            }
            let current = &mut doc.pages[index];
            let before = bare(current);
            current.name = page.name.clone();
            current.kind = page.kind;
            touched.pages.insert(page.id.clone());
            Ok(Edit::SetPage {
                page: Box::new(before),
            })
        }
        Edit::MovePage { id, index } => {
            let from = page_index(doc, id)?;
            let len = doc.pages.len() - 1;
            if *index > len {
                return Err(OpErrorKind::IndexOutOfRange { index: *index, len });
            }
            let page = doc.pages.remove(from);
            doc.pages.insert(*index, page);
            touched.pages.insert(id.clone());
            Ok(Edit::MovePage {
                id: id.clone(),
                index: from,
            })
        }
        Edit::SetShell { shell } => {
            let before = (**shell).clone().swap_into(doc);
            touched.shell = true;
            Ok(Edit::SetShell {
                shell: Box::new(before),
            })
        }
    }
}

/// Applies `edits` in order, all or none, and returns their inverse (to apply in the order given) and what
/// changed, attributed to `origin`. On an error, `doc` is unchanged and the error names the edit's index.
///
/// Stores use this for commits, undo, redo and sync; the edits a commit carries were already checked against
/// the store's document, so for a commit it does not fail.
pub fn apply_edits(
    doc: &mut Document,
    edits: &[Edit],
    origin: Origin,
) -> Result<(Vec<Edit>, ChangeSet), (usize, OpErrorKind)> {
    let mut work = doc.clone();
    let mut touched = Touched::default();
    let mut inverse = Vec::with_capacity(edits.len());
    for (index, edit) in edits.iter().enumerate() {
        inverse.push(apply_one(&mut work, edit, &mut touched).map_err(|kind| (index, kind))?);
    }
    super::advanced::validate_references(&work)
        .map_err(|kind| (edits.len().saturating_sub(1), kind))?;
    super::advanced::validate_instance_bindings(&work)
        .map_err(|kind| (edits.len().saturating_sub(1), kind))?;
    inverse.reverse();
    *doc = work;
    Ok((inverse, touched.into_change_set(origin)))
}
