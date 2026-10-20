//! Merge: a three-way merge of documents by node id and property (ARCHITECTURE §4.5, DESIGN C22). It works on
//! documents, not on a store, so the Git merge driver and the editor use the same code.
//!
//! What one side changed and the other did not is taken. What both changed the same way is taken once. The rest
//! are conflicts:
//!
//! - both sides changed the same property of a node: a **conflict node** takes the node's place and id, holding
//!   both versions (each with every change that did not conflict) and the node's merged children;
//! - one side removed a node and the other changed it, or anything in it: the change wins, and the topmost node
//!   that was removed becomes a conflict node with no version for the side that removed it;
//! - both moved a node to different places, or both reordered the same children differently, or both changed
//!   one entry of the document (a variable, a style, the name, a page) differently: ours is kept and the
//!   conflict is reported, since there is no node to hold both. Git keeps theirs (`MERGE_HEAD`).
//!
//! Where both sides insert at the same place, their nodes are ordered by id, so the merge does not depend on
//! which side is ours unless something conflicts. A variable or component one side removed and the other
//! still uses is kept. The result is checked like any document; a merge that cannot be made valid is an error,
//! and the caller keeps ours.

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value as Json};
use vellum_types::{
    Component, ComponentId, Document, DocumentMeta, Node, NodeId, NodeKind, Page, PageId, PageKind,
    referenced_variables,
};

use crate::ops::check_document;
use crate::ops::edit::{Container, can_hold};

/// Which side of a merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    /// The branch being merged into (Git's `%A`).
    Ours,
    /// The branch being merged in (Git's `%B`).
    Theirs,
}

/// Something both sides changed in ways that do not combine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Conflict {
    /// Both sides changed these properties of a node; a conflict node holds both versions.
    Properties {
        /// The node, now a conflict node.
        id: NodeId,
        /// The properties, in the file's spelling; `type` when the kind is one of them.
        properties: Vec<String>,
    },
    /// One side removed a node the other changed, or changed something in; a conflict node keeps the change.
    Removed {
        /// The node, now a conflict node.
        id: NodeId,
        /// The side that removed it.
        by: Side,
    },
    /// Both sides moved the node, to different places; it is where ours put it.
    Moved {
        /// The node.
        id: NodeId,
    },
    /// Both sides reordered the same children differently; ours' order is kept.
    Order {
        /// The parent: a node id, a page id, or `pages` for the page list.
        parent: String,
    },
    /// Both sides changed one entry of the document differently, or one removed what the other changed or
    /// uses. The path names it: `meta.name`, `variables.<id>`, `styles.<id>`, `pages.<id>`,
    /// `components.<id>`, `components.<id>.root`.
    Entry {
        /// Where.
        path: String,
    },
}

/// A merged document and what conflicted.
#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    /// The document; conflict nodes mark node conflicts.
    pub document: Document,
    /// Every conflict, those with a conflict node and those without.
    pub conflicts: Vec<Conflict>,
}

/// A merge whose result is not a valid document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeError {
    /// What is wrong with it, as `check_document` says.
    pub problems: Vec<String>,
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the merged document is not valid: {}",
            self.problems.join("; ")
        )
    }
}

impl std::error::Error for MergeError {}

/// Merges `ours` and `theirs`, two versions of `base`.
pub fn merge(base: &Document, ours: &Document, theirs: &Document) -> Result<Merged, MergeError> {
    let mut m = Merger::default();

    let docs = [base, ours, theirs];
    let meta = DocumentMeta {
        format_version: field(&mut m, "meta.formatVersion", docs, |d| {
            &d.meta.format_version
        }),
        name: field(&mut m, "meta.name", docs, |d| &d.meta.name),
        created_with: field(&mut m, "meta.createdWith", docs, |d| &d.meta.created_with),
        kind: field(&mut m, "meta.kind", docs, |d| &d.meta.kind),
    };
    let mut document = Document {
        meta,
        imports: m.map("imports", &base.imports, &ours.imports, &theirs.imports),
        themes: m.map("themes", &base.themes, &ours.themes, &theirs.themes),
        variables: m.map(
            "variables",
            &base.variables,
            &ours.variables,
            &theirs.variables,
        ),
        styles: m.map("styles", &base.styles, &ours.styles, &theirs.styles),
        fonts: m.map("fonts", &base.fonts, &ours.fonts, &theirs.fonts),
        comments: m.map("comments", &base.comments, &ours.comments, &theirs.comments),
        components: BTreeMap::new(),
        pages: Vec::new(),
    };

    // Components: merged node by node when the root is the same everywhere, otherwise taken whole.
    let mut trees = BTreeSet::new();
    let ids: BTreeSet<&ComponentId> = base
        .components
        .keys()
        .chain(ours.components.keys())
        .chain(theirs.components.keys())
        .collect();
    for id in ids {
        let (b, o, t) = (
            base.components.get(id),
            ours.components.get(id),
            theirs.components.get(id),
        );
        let roots: BTreeSet<_> = [b, o, t]
            .into_iter()
            .flatten()
            .map(|c| (c.root.id(), c.root.kind()))
            .collect();
        let path = format!("components.{id}");
        if roots.len() == 1 && (o.is_some() && t.is_some() || b.is_none()) {
            trees.insert(id.clone());
            let name = m.value(
                &format!("{path}.name"),
                b.map(|c| &c.name),
                o.map(|c| &c.name),
                t.map(|c| &c.name),
            );
            let props = m.value(
                &format!("{path}.props"),
                b.map(|c| &c.props),
                o.map(|c| &c.props),
                t.map(|c| &c.props),
            );
            let variants = m.value(
                &format!("{path}.variants"),
                b.map(|c| &c.variants),
                o.map(|c| &c.variants),
                t.map(|c| &c.variants),
            );
            let shown = o.or(t).expect("one side has it");
            document.components.insert(
                id.clone(),
                Component {
                    name: name.cloned().unwrap_or_default(),
                    props: props.cloned().unwrap_or_default(),
                    root: shown.root.clone(),
                    variants: variants.cloned().unwrap_or_default(),
                },
            );
        } else if let Some(whole) = m.value(&path, b, o, t) {
            document.components.insert(id.clone(), whole.clone());
        }
    }

    let (vb, vo, vt) = (
        Version::of(base, &trees),
        Version::of(ours, &trees),
        Version::of(theirs, &trees),
    );
    let tree = Trees::merge(&mut m, &vb, &vo, &vt);

    // Component roots: a conflict cannot be a root.
    for id in &trees {
        let Some(component) = document.components.get_mut(id) else {
            continue;
        };
        let root = component.root.id().clone();
        if !tree.out.contains_key(&root) {
            document.components.remove(id);
            continue;
        }
        let mut node = tree.build_node(&root, &mut m);
        if let Node::Conflict(c) = &node {
            m.conflicts.retain(|k| !matches!(k, Conflict::Properties { id: n, .. } | Conflict::Removed { id: n, .. } if n == &root));
            m.conflict(Conflict::Entry {
                path: format!("components.{id}.root"),
            });
            let mut version = *c
                .ours
                .clone()
                .or_else(|| c.theirs.clone())
                .expect("a version");
            if let Some(slot) = version.children_mut() {
                *slot = c.children.clone();
            }
            node = version;
        }
        component.root = node;
    }

    // Pages.
    let page_ids: BTreeSet<&PageId> = vb
        .pages
        .keys()
        .chain(vo.pages.keys())
        .chain(vt.pages.keys())
        .collect();
    let mut pages: BTreeMap<PageId, Page> = BTreeMap::new();
    for id in page_ids {
        let (b, o, t) = (vb.pages.get(id), vo.pages.get(id), vt.pages.get(id));
        let path = format!("pages.{id}");
        let kept = match (b, o, t) {
            (Some(_), None, Some(t)) if tree.page_lives(id, Side::Ours) => {
                m.conflict(Conflict::Entry { path: path.clone() });
                Some(bare_page(t))
            }
            (Some(_), Some(o), None) if tree.page_lives(id, Side::Theirs) => {
                m.conflict(Conflict::Entry { path: path.clone() });
                Some(bare_page(o))
            }
            (Some(_), None, _) | (Some(_), _, None) => None,
            _ => {
                let (b, o, t) = (
                    b.map(|p| bare_page(p)),
                    o.map(|p| bare_page(p)),
                    t.map(|p| bare_page(p)),
                );
                m.value(&path, b.as_ref(), o.as_ref(), t.as_ref()).cloned()
            }
        };
        let kept = kept.or_else(|| {
            // Something lives on this page, though the page was removed.
            tree.holds(&Holder::Page(id.clone())).then(|| {
                m.conflict(Conflict::Entry { path: path.clone() });
                bare_page(o.or(t).or(b).expect("one side has it"))
            })
        });
        if let Some(page) = kept {
            pages.insert(id.clone(), page);
        }
    }
    let alive: BTreeSet<PageId> = pages.keys().cloned().collect();
    let mut order_conflict = false;
    let order = merge_order(
        &alive,
        &vb.page_order,
        Some(&vo.page_order),
        Some(&vt.page_order),
        &mut order_conflict,
    );
    if order_conflict {
        m.conflict(Conflict::Order {
            parent: "pages".into(),
        });
    }
    for id in order {
        let mut page = pages.remove(&id).expect("an alive page");
        page.children = tree.build_children(&Holder::Page(id.clone()), &mut m);
        if page.kind != PageKind::Canvas
            && page
                .children
                .iter()
                .any(|n| !can_hold(Container::Page(page.kind), n.kind()))
        {
            page.kind = PageKind::Canvas;
            m.conflict(Conflict::Entry {
                path: format!("pages.{id}.kind"),
            });
        }
        document.pages.push(page);
    }

    keep_what_is_used(&mut m, &mut document, [ours, theirs, base]);
    hold_broken_instances(&mut m, &mut document, ours, theirs);

    let problems = check_document(&document);
    if !problems.is_empty() {
        return Err(MergeError { problems });
    }
    let mut conflicts = m.conflicts;
    conflicts.dedup();
    Ok(Merged {
        document,
        conflicts,
    })
}

/// One field every version has, merged.
fn field<V: PartialEq + Clone>(
    m: &mut Merger,
    path: &str,
    [b, o, t]: [&Document; 3],
    get: impl Fn(&Document) -> &V,
) -> V {
    m.value(path, Some(get(b)), Some(get(o)), Some(get(t)))
        .expect("every version has it")
        .clone()
}

/// Variables and components one side removed while the other uses them are kept, from whichever side has
/// them, as are the variables they alias.
fn keep_what_is_used(m: &mut Merger, doc: &mut Document, sources: [&Document; 3]) {
    loop {
        let mut used = referenced_variables(&doc.pages);
        used.extend(referenced_variables(&doc.components));
        used.extend(referenced_variables(&doc.styles));
        for variable in doc.variables.values() {
            used.extend(variable.aliases().into_iter().cloned());
        }
        let missing: Vec<_> = used
            .into_iter()
            .filter(|id| !doc.variables.contains_key(id))
            .collect();
        let mut instances = BTreeSet::new();
        doc.pages
            .iter()
            .flat_map(|p| &p.children)
            .chain(doc.components.values().map(|c| &c.root))
            .for_each(|n| instances_in(n, &mut instances));
        let gone: Vec<_> = instances
            .into_iter()
            .filter(|id| !doc.components.contains_key(id))
            .collect();
        let mut added = false;
        for id in missing {
            if let Some(v) = sources.iter().find_map(|d| d.variables.get(&id)) {
                doc.variables.insert(id.clone(), v.clone());
                m.conflict(Conflict::Entry {
                    path: format!("variables.{id}"),
                });
                added = true;
            }
        }
        for id in gone {
            if let Some(c) = sources.iter().find_map(|d| d.components.get(&id)) {
                doc.components.insert(id.clone(), c.clone());
                m.conflict(Conflict::Entry {
                    path: format!("components.{id}"),
                });
                added = true;
            }
        }
        if !added {
            return;
        }
    }
}

/// An instance whose overrides or slot content no longer resolve (one side changed the component, the other
/// the instance) becomes a conflict node holding both versions of it.
fn hold_broken_instances(m: &mut Merger, doc: &mut Document, ours: &Document, theirs: &Document) {
    fn broken(doc: &Document, nodes: &[Node], out: &mut Vec<NodeId>) {
        for node in nodes {
            if let Node::Instance(n) = node
                && !crate::ops::advanced::resolves(doc, n)
            {
                out.push(n.id.clone());
            }
            broken(doc, node.children(), out);
        }
    }
    fn wrap(nodes: &mut [Node], id: &NodeId, with: &mut Option<Node>) {
        for node in nodes {
            if node.id() == id {
                if let Some(with) = with.take() {
                    *node = with;
                }
                return;
            }
            if let Some(children) = node.children_mut() {
                wrap(children, id, with);
            }
        }
    }
    let mut ids = Vec::new();
    for page in &doc.pages {
        broken(doc, &page.children, &mut ids);
    }
    for c in doc.components.values() {
        broken(doc, std::slice::from_ref(&c.root), &mut ids);
    }
    for id in ids {
        let version = |d: &Document| crate::ops::find(d, &id).map(bare);
        let properties = vec!["overrides".to_string()];
        m.conflict(Conflict::Properties {
            id: id.clone(),
            properties: properties.clone(),
        });
        let mut with = Some(conflict_node(
            &id,
            version(ours).as_ref(),
            version(theirs).as_ref(),
            &properties,
            Vec::new(),
        ));
        for page in &mut doc.pages {
            wrap(&mut page.children, &id, &mut with);
        }
        for c in doc.components.values_mut() {
            wrap(std::slice::from_mut(&mut c.root), &id, &mut with);
        }
    }
}

fn instances_in(node: &Node, out: &mut BTreeSet<ComponentId>) {
    if let Node::Instance(n) = node {
        out.insert(n.component_id.clone());
        for value in n.overrides.values() {
            match value {
                vellum_types::Override::Replace(node) => instances_in(node, out),
                vellum_types::Override::Children(nodes) => {
                    nodes.iter().for_each(|n| instances_in(n, out))
                }
                vellum_types::Override::Props(_) => {}
            }
        }
        n.slot_content
            .values()
            .flatten()
            .for_each(|n| instances_in(n, out));
    }
    node.children().iter().for_each(|c| instances_in(c, out));
}

#[derive(Default)]
struct Merger {
    conflicts: Vec<Conflict>,
}

impl Merger {
    fn conflict(&mut self, conflict: Conflict) {
        if !self.conflicts.contains(&conflict) {
            self.conflicts.push(conflict);
        }
    }

    /// The three-way merge of one value; `None` is "not there". A change beats no change; a change beats a
    /// removal; two different changes keep ours. The last two are conflicts at `path`.
    fn value<'a, V: PartialEq>(
        &mut self,
        path: &str,
        b: Option<&'a V>,
        o: Option<&'a V>,
        t: Option<&'a V>,
    ) -> Option<&'a V> {
        if o == t {
            return o;
        }
        if o == b {
            return t;
        }
        if t == b {
            return o;
        }
        self.conflict(Conflict::Entry { path: path.into() });
        match (o, t) {
            (None, Some(t)) => Some(t),
            _ => o,
        }
    }

    fn map<K: Ord + Clone + std::fmt::Display, V: PartialEq + Clone>(
        &mut self,
        name: &str,
        b: &BTreeMap<K, V>,
        o: &BTreeMap<K, V>,
        t: &BTreeMap<K, V>,
    ) -> BTreeMap<K, V> {
        let keys: BTreeSet<&K> = b.keys().chain(o.keys()).chain(t.keys()).collect();
        keys.into_iter()
            .filter_map(|k| {
                self.value(&format!("{name}.{k}"), b.get(k), o.get(k), t.get(k))
                    .map(|v| (k.clone(), v.clone()))
            })
            .collect()
    }
}

/// What holds a node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Holder {
    Page(PageId),
    Node(NodeId),
    Component(ComponentId),
}

struct Spot<'a> {
    holder: Holder,
    node: &'a Node,
}

/// One side's nodes (in the pages and in the components merged node by node), pages and orders.
struct Version<'a> {
    at: BTreeMap<NodeId, Spot<'a>>,
    order: Vec<NodeId>,
    kids: BTreeMap<Holder, Vec<NodeId>>,
    pages: BTreeMap<PageId, &'a Page>,
    page_order: Vec<PageId>,
}

impl<'a> Version<'a> {
    fn of(doc: &'a Document, trees: &BTreeSet<ComponentId>) -> Self {
        let mut v = Self {
            at: BTreeMap::new(),
            order: Vec::new(),
            kids: BTreeMap::new(),
            pages: doc.pages.iter().map(|p| (p.id.clone(), p)).collect(),
            page_order: doc.pages.iter().map(|p| p.id.clone()).collect(),
        };
        for page in &doc.pages {
            v.add(Holder::Page(page.id.clone()), &page.children);
        }
        for id in trees {
            if let Some(c) = doc.components.get(id) {
                v.add(Holder::Component(id.clone()), std::slice::from_ref(&c.root));
            }
        }
        v
    }

    fn add(&mut self, holder: Holder, nodes: &'a [Node]) {
        self.kids.insert(
            holder.clone(),
            nodes.iter().map(|n| n.id().clone()).collect(),
        );
        for node in nodes {
            self.at.insert(
                node.id().clone(),
                Spot {
                    holder: holder.clone(),
                    node,
                },
            );
            self.order.push(node.id().clone());
            self.add(Holder::Node(node.id().clone()), node.children());
        }
    }

    fn kids(&self, holder: &Holder) -> &[NodeId] {
        self.kids.get(holder).map_or(&[], Vec::as_slice)
    }
}

/// A node with its children taken off.
fn bare(node: &Node) -> Node {
    let mut copy = node.clone();
    if let Some(children) = copy.children_mut() {
        children.clear();
    }
    copy
}

fn bare_page(page: &Page) -> Page {
    Page {
        children: Vec::new(),
        ..page.clone()
    }
}

/// What one side changed, by node: the node itself (new, changed, moved, its children changed) and, deep,
/// anything under it too.
struct Touched {
    deep: BTreeSet<NodeId>,
    pages: BTreeSet<PageId>,
}

impl Touched {
    fn of(b: &Version<'_>, s: &Version<'_>) -> Self {
        let mut deep = BTreeSet::new();
        for id in s.order.iter().rev() {
            let at = &s.at[id];
            let holder = Holder::Node(id.clone());
            let own = match b.at.get(id) {
                None => true,
                Some(was) => {
                    was.holder != at.holder
                        || bare(was.node) != bare(at.node)
                        || b.kids(&holder) != s.kids(&holder)
                }
            };
            if own || s.kids(&holder).iter().any(|c| deep.contains(c)) {
                deep.insert(id.clone());
            }
        }
        let pages = s
            .pages
            .iter()
            .filter(|(id, page)| {
                let holder = Holder::Page((*id).clone());
                b.pages
                    .get(*id)
                    .is_none_or(|was| bare_page(was) != bare_page(page))
                    || b.kids(&holder) != s.kids(&holder)
                    || s.kids(&holder).iter().any(|c| deep.contains(c))
            })
            .map(|(id, _)| id.clone())
            .collect();
        Self { deep, pages }
    }
}

/// A merged node, before its children are placed.
enum Out {
    Plain(Box<Node>),
    Conflict {
        ours: Option<Box<Node>>,
        theirs: Option<Box<Node>>,
        properties: Vec<String>,
    },
}

/// The merged node trees: each surviving node, and each parent's children in order.
struct Trees {
    out: BTreeMap<NodeId, Out>,
    kids: BTreeMap<Holder, Vec<NodeId>>,
    /// Pages a side removed that live on, because the other side changed them.
    pages_kept: BTreeMap<Side, BTreeSet<PageId>>,
}

impl Trees {
    fn merge(m: &mut Merger, b: &Version<'_>, o: &Version<'_>, t: &Version<'_>) -> Self {
        let (to, tt) = (Touched::of(b, o), Touched::of(b, t));
        // What lives on of what one side removed: decided at the topmost node it removed (or the page), from
        // whether the other side changed anything there.
        let lives_o = Self::removal(b, o, t, &tt);
        let lives_t = Self::removal(b, t, o, &to);

        let mut out = BTreeMap::new();
        let mut holder = BTreeMap::new();
        let ids: BTreeSet<&NodeId> = b.at.keys().chain(o.at.keys()).chain(t.at.keys()).collect();
        for id in ids {
            let (sb, so, st) = (b.at.get(id), o.at.get(id), t.at.get(id));
            let node = match (sb, so, st) {
                (b, Some(o), Some(t)) => Some(merge_node(m, id, b.map(|s| s.node), o.node, t.node)),
                (Some(_), None, Some(t)) => lives_o.get(id).and_then(|(lives, root)| {
                    lives.then(|| removed(m, id, Side::Ours, t.node, *root))
                }),
                (Some(_), Some(o), None) => lives_t.get(id).and_then(|(lives, root)| {
                    lives.then(|| removed(m, id, Side::Theirs, o.node, *root))
                }),
                (None, Some(o), None) => Some(Out::Plain(Box::new(bare(o.node)))),
                (None, None, Some(t)) => Some(Out::Plain(Box::new(bare(t.node)))),
                _ => None,
            };
            let Some(node) = node else { continue };
            let (hb, ho, ht) = (
                sb.map(|s| &s.holder),
                so.map(|s| &s.holder),
                st.map(|s| &s.holder),
            );
            let place = match (ho, ht) {
                (Some(o), Some(t)) if o == t => o,
                (Some(o), Some(t)) if Some(o) == hb => t,
                (Some(o), Some(t)) if Some(t) == hb => o,
                (Some(o), Some(_)) => {
                    m.conflict(Conflict::Moved { id: id.clone() });
                    o
                }
                (Some(h), None) | (None, Some(h)) => h,
                (None, None) => unreachable!("a surviving node is on one side"),
            };
            holder.insert(id.clone(), place.clone());
            out.insert(id.clone(), node);
        }

        // A parent that does not live on: take the other side's place, or ours.
        let ids: Vec<NodeId> = holder.keys().cloned().collect();
        for id in &ids {
            if let Holder::Node(p) = &holder[id]
                && !out.contains_key(p)
            {
                let fallback = [o, t]
                    .into_iter()
                    .filter_map(|v| v.at.get(id))
                    .map(|s| s.holder.clone())
                    .find(|h| !matches!(h, Holder::Node(p) if !out.contains_key(p)));
                match fallback {
                    Some(h) => {
                        holder.insert(id.clone(), h);
                    }
                    None => {
                        holder.remove(id);
                        out.remove(id);
                    }
                }
            }
        }

        // Moves on both sides can make a cycle (A into B, B into A). Put its nodes where ours has them.
        // Each round puts at least one node back where ours has it; ours is a tree, so this ends.
        while let Some(cycle) = find_cycle(&holder) {
            let mut changed = false;
            for id in cycle {
                if let Some(h) = o.at.get(&id).map(|s| s.holder.clone())
                    && h != holder[&id]
                {
                    holder.insert(id.clone(), h);
                    m.conflict(Conflict::Moved { id });
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // Children in order.
        let mut by_holder: BTreeMap<Holder, BTreeSet<NodeId>> = BTreeMap::new();
        for (id, h) in &holder {
            by_holder.entry(h.clone()).or_default().insert(id.clone());
        }
        let mut kids = BTreeMap::new();
        for (h, alive) in by_holder {
            let mut clash = false;
            let order = merge_order(
                &alive,
                b.kids(&h),
                o.kids.get(&h).map(Vec::as_slice),
                t.kids.get(&h).map(Vec::as_slice),
                &mut clash,
            );
            if clash {
                m.conflict(Conflict::Order {
                    parent: match &h {
                        Holder::Page(p) => p.to_string(),
                        Holder::Node(n) => n.to_string(),
                        Holder::Component(c) => c.to_string(),
                    },
                });
            }
            kids.insert(h, order);
        }

        // A node whose merged kind cannot hold its merged children (one side changed its kind, the other drew
        // into it) becomes a conflict.
        let kinds: BTreeMap<NodeId, NodeKind> = out
            .iter()
            .map(|(id, node)| {
                let kind = match node {
                    Out::Plain(n) => n.kind(),
                    Out::Conflict { .. } => NodeKind::Conflict,
                };
                (id.clone(), kind)
            })
            .collect();
        for (id, node) in out.iter_mut() {
            let Out::Plain(plain) = node else { continue };
            let children = kids
                .get(&Holder::Node(id.clone()))
                .map_or(&[][..], Vec::as_slice);
            let container = matches!(
                plain.kind(),
                NodeKind::Frame | NodeKind::Group | NodeKind::Boolean | NodeKind::Conflict
            );
            let fits = container
                && children
                    .iter()
                    .all(|c| can_hold(Container::Node(plain.kind()), kinds[c]));
            if !children.is_empty() && !fits {
                let (ours, theirs) = (
                    o.at.get(id).map(|s| bare(s.node)),
                    t.at.get(id).map(|s| bare(s.node)),
                );
                m.conflict(Conflict::Properties {
                    id: id.clone(),
                    properties: vec!["type".into()],
                });
                *node = Out::Conflict {
                    ours: ours.map(Box::new),
                    theirs: theirs.map(Box::new),
                    properties: vec!["type".into()],
                };
            }
        }

        let mut pages_kept = BTreeMap::new();
        pages_kept.insert(
            Side::Ours,
            b.pages
                .keys()
                .filter(|p| !o.pages.contains_key(*p) && tt.pages.contains(*p))
                .cloned()
                .collect(),
        );
        pages_kept.insert(
            Side::Theirs,
            b.pages
                .keys()
                .filter(|p| !t.pages.contains_key(*p) && to.pages.contains(*p))
                .cloned()
                .collect(),
        );
        Self {
            out,
            kids,
            pages_kept,
        }
    }

    /// For each base node that `gone` removed and `other` kept: whether it lives on, and whether it is where
    /// the removal starts (the topmost node removed, which becomes the conflict node).
    fn removal(
        b: &Version<'_>,
        gone: &Version<'_>,
        other: &Version<'_>,
        touched: &Touched,
    ) -> BTreeMap<NodeId, (bool, bool)> {
        let mut decided: BTreeMap<NodeId, (bool, bool)> = BTreeMap::new();
        for id in &b.order {
            if gone.at.contains_key(id) || !other.at.contains_key(id) {
                continue;
            }
            let inherited = match &b.at[id].holder {
                Holder::Node(p) => decided.get(p).map(|(lives, _)| *lives),
                Holder::Page(p) if !gone.pages.contains_key(p) && other.pages.contains_key(p) => {
                    Some(touched.pages.contains(p))
                }
                _ => None,
            };
            let decision = match inherited {
                Some(lives) => (lives, false),
                None => (touched.deep.contains(id), true),
            };
            decided.insert(id.clone(), decision);
        }
        decided
    }

    fn page_lives(&self, page: &PageId, removed_by: Side) -> bool {
        self.pages_kept[&removed_by].contains(page)
    }

    fn holds(&self, holder: &Holder) -> bool {
        self.kids.get(holder).is_some_and(|k| !k.is_empty())
    }

    fn build_children(&self, holder: &Holder, m: &mut Merger) -> Vec<Node> {
        self.kids
            .get(holder)
            .map(|ids| ids.iter().map(|id| self.build_node(id, m)).collect())
            .unwrap_or_default()
    }

    fn build_node(&self, id: &NodeId, m: &mut Merger) -> Node {
        let children = self.build_children(&Holder::Node(id.clone()), m);
        match &self.out[id] {
            Out::Plain(node) => {
                let mut node = (**node).clone();
                if let Some(slot) = node.children_mut() {
                    *slot = children;
                }
                node
            }
            Out::Conflict {
                ours,
                theirs,
                properties,
            } => conflict_node(id, ours.as_deref(), theirs.as_deref(), properties, children),
        }
    }
}

/// A node one side removed and the other kept: the conflict node where the removal starts, the kept version
/// under it.
fn removed(m: &mut Merger, id: &NodeId, by: Side, kept: &Node, root: bool) -> Out {
    if !root {
        return Out::Plain(Box::new(bare(kept)));
    }
    m.conflict(Conflict::Removed { id: id.clone(), by });
    let kept = Some(Box::new(bare(kept)));
    match by {
        Side::Ours => Out::Conflict {
            ours: None,
            theirs: kept,
            properties: Vec::new(),
        },
        Side::Theirs => Out::Conflict {
            ours: kept,
            theirs: None,
            properties: Vec::new(),
        },
    }
}

/// The three-way merge of one node's own properties (its kind among them).
fn merge_node(m: &mut Merger, id: &NodeId, b: Option<&Node>, o: &Node, t: &Node) -> Out {
    let (bo, bt) = (bare(o), bare(t));
    let bb = b.map(bare);
    if bo == bt || bb.as_ref() == Some(&bt) {
        return Out::Plain(Box::new(bo));
    }
    if bb.as_ref() == Some(&bo) {
        return Out::Plain(Box::new(bt));
    }
    let (po, pt) = (properties(&bo), properties(&bt));
    let pb = bb.as_ref().map(properties);
    let differing = |a: &BTreeMap<String, Json>, c: &BTreeMap<String, Json>| -> Vec<String> {
        let mut keys: BTreeSet<String> = a
            .keys()
            .chain(c.keys())
            .filter(|k| a.get(*k) != c.get(*k))
            .cloned()
            .collect();
        if bo.kind() != bt.kind() {
            keys.insert("type".into());
        }
        keys.into_iter().collect()
    };
    let conflict = |m: &mut Merger, ours: Node, theirs: Node, properties: Vec<String>| {
        m.conflict(Conflict::Properties {
            id: id.clone(),
            properties: properties.clone(),
        });
        Out::Conflict {
            ours: Some(Box::new(ours)),
            theirs: Some(Box::new(theirs)),
            properties,
        }
    };
    // Added on both sides, or a kind changed on one side while the other changed anything: whole versions.
    let Some(pb) = pb.filter(|_| bo.kind() == bt.kind()) else {
        return conflict(m, bo.clone(), bt.clone(), differing(&po, &pt));
    };
    let keys: BTreeSet<&String> = pb.keys().chain(po.keys()).chain(pt.keys()).collect();
    let (mut with_ours, mut with_theirs) = (Map::new(), Map::new());
    let mut clashes = Vec::new();
    for key in keys {
        let (vb, vo, vt) = (pb.get(key), po.get(key), pt.get(key));
        let (a, c) = if vo == vt || vt == vb {
            (vo, vo)
        } else if vo == vb {
            (vt, vt)
        } else {
            clashes.push(key.clone());
            (vo, vt)
        };
        if let Some(a) = a {
            with_ours.insert(key.clone(), a.clone());
        }
        if let Some(c) = c {
            with_theirs.insert(key.clone(), c.clone());
        }
    }
    let build = |mut map: Map<String, Json>| -> Option<Node> {
        map.insert("type".into(), Json::String(bo.kind().as_str().into()));
        map.insert("id".into(), Json::String(id.to_string()));
        serde_json::from_value(Json::Object(map)).ok()
    };
    match (build(with_ours), build(with_theirs)) {
        (Some(a), Some(_)) if clashes.is_empty() => Out::Plain(Box::new(a)),
        (Some(a), Some(c)) => conflict(m, a, c, clashes),
        _ => conflict(m, bo.clone(), bt.clone(), differing(&po, &pt)),
    }
}

/// A node's own properties in the file's spelling, without id, type or children.
fn properties(node: &Node) -> BTreeMap<String, Json> {
    node.property_patch().properties().clone()
}

/// A conflict node at `id`, shown like one of its versions.
fn conflict_node(
    id: &NodeId,
    ours: Option<&Node>,
    theirs: Option<&Node>,
    properties: &[String],
    children: Vec<Node>,
) -> Node {
    let shown = ours.or(theirs).expect("a conflict has a version");
    let mut map: Map<String, Json> = shown
        .common_patch()
        .properties()
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    map.insert("type".into(), Json::String("conflict".into()));
    map.insert("id".into(), Json::String(id.to_string()));
    if let Some(ours) = ours {
        map.insert("ours".into(), serde_json::to_value(ours).expect("a node"));
    }
    if let Some(theirs) = theirs {
        map.insert(
            "theirs".into(),
            serde_json::to_value(theirs).expect("a node"),
        );
    }
    map.insert(
        "properties".into(),
        serde_json::to_value(properties).expect("names"),
    );
    map.insert(
        "children".into(),
        serde_json::to_value(children).expect("nodes"),
    );
    serde_json::from_value(Json::Object(map))
        .expect("a conflict node from a node's common properties")
}

/// Nodes on a loop of parents, if any.
fn find_cycle(holder: &BTreeMap<NodeId, Holder>) -> Option<Vec<NodeId>> {
    let mut done: BTreeSet<&NodeId> = BTreeSet::new();
    for start in holder.keys() {
        let mut path: Vec<&NodeId> = Vec::new();
        let mut on_path = BTreeSet::new();
        let mut at = start;
        loop {
            if done.contains(at) {
                break;
            }
            if !on_path.insert(at) {
                let from = path.iter().position(|p| *p == at).expect("on the path");
                return Some(path[from..].iter().map(|n| (*n).clone()).collect());
            }
            path.push(at);
            match holder.get(at) {
                Some(Holder::Node(p)) if holder.contains_key(p) => at = p,
                _ => break,
            }
        }
        done.extend(path);
    }
    None
}

/// The merged order of the children `alive` under one parent, from the base's order and each side's. The
/// children all three have keep the order that changed (ours when both did: `clash`); the others go after the
/// child they follow on their side, both sides' runs at one place ordered by their first id.
fn merge_order<T: Ord + Clone>(
    alive: &BTreeSet<T>,
    b: &[T],
    o: Option<&[T]>,
    t: Option<&[T]>,
    clash: &mut bool,
) -> Vec<T> {
    let (o, t) = (o.unwrap_or_default(), t.unwrap_or_default());
    let common: BTreeSet<&T> = alive
        .iter()
        .filter(|id| b.contains(id) && o.contains(id) && t.contains(id))
        .collect();
    let only = |list: &[T]| -> Vec<T> {
        list.iter()
            .filter(|i| common.contains(i))
            .cloned()
            .collect()
    };
    let (ob, oo, ot) = (only(b), only(o), only(t));
    let order = if oo == ot || ot == ob {
        oo
    } else if oo == ob {
        ot
    } else {
        *clash = true;
        oo
    };
    let runs = |list: &[T]| -> BTreeMap<Option<T>, Vec<T>> {
        let mut out: BTreeMap<Option<T>, Vec<T>> = BTreeMap::new();
        let mut anchor = None;
        for id in list {
            if common.contains(id) {
                anchor = Some(id.clone());
            } else if alive.contains(id) {
                out.entry(anchor.clone()).or_default().push(id.clone());
            }
        }
        out
    };
    let (ro, rt) = (runs(o), runs(t));
    let mut gap = |anchor: Option<T>| -> Vec<T> {
        let (a, c) = (
            ro.get(&anchor).cloned().unwrap_or_default(),
            rt.get(&anchor).cloned().unwrap_or_default(),
        );
        merge_runs(a, c, clash)
    };
    let mut result = gap(None);
    for id in order {
        result.push(id.clone());
        result.extend(gap(Some(id)));
    }
    let mut seen = BTreeSet::new();
    result.retain(|id| seen.insert(id.clone()));
    // Anything not placed yet (on neither side's list here) goes at the end, by id.
    result.extend(alive.iter().filter(|id| !seen.contains(*id)).cloned());
    result
}

/// Two sides' runs of children at one place. Children in both runs line them up, when both have them in the same
/// order (otherwise ours wins: `clash`); between those, the run whose first id is smaller goes first, so the
/// result does not depend on which side is ours.
fn merge_runs<T: Ord + Clone>(a: Vec<T>, c: Vec<T>, clash: &mut bool) -> Vec<T> {
    let in_c: BTreeSet<&T> = c.iter().collect();
    let in_a: BTreeSet<&T> = a.iter().collect();
    let shared_a: Vec<&T> = a.iter().filter(|x| in_c.contains(x)).collect();
    let shared_c: Vec<&T> = c.iter().filter(|x| in_a.contains(x)).collect();
    if shared_a != shared_c {
        *clash = true;
        return a
            .iter()
            .chain(c.iter().filter(|x| !in_a.contains(x)))
            .cloned()
            .collect();
    }
    let segments = |run: &[T]| -> Vec<Vec<T>> {
        let mut out = vec![Vec::new()];
        for x in run {
            if in_a.contains(x) && in_c.contains(x) {
                out.push(Vec::new());
            } else {
                out.last_mut().expect("one segment").push(x.clone());
            }
        }
        out
    };
    let (sa, sc) = (segments(&a), segments(&c));
    let mut result = Vec::new();
    for (i, (x, y)) in sa.into_iter().zip(sc).enumerate() {
        match (x.first(), y.first()) {
            (Some(p), Some(q)) if q < p => result.extend(y.into_iter().chain(x)),
            _ => result.extend(x.into_iter().chain(y)),
        }
        if let Some(shared) = shared_a.get(i) {
            result.push((*shared).clone());
        }
    }
    result
}
