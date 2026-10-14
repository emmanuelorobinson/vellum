//! LayoutEngine over Taffy 0.14. ADR-002 sizing is prepared by core, not reimplemented here.
//! The persistent tree preserves Taffy's measurement caches; only changed inputs and dirty nodes are
//! invalidated. Reconciliation scans identities/own fields but does not recompute clean subtrees.
mod mapping;
use std::collections::{BTreeMap, BTreeSet};
use taffy::{compute_leaf_layout, prelude::*};
use vellum_core::{
    layout::{self, LayoutError, LayoutProblem, PreparedNode},
    ports::{
        self, DirtySet, Extent, LayoutContent, LayoutEngine, LayoutResult, LayoutTree, TextEngine,
    },
};
use vellum_types::NodeIdPath;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
struct Entry {
    id: NodeId,
    style: Style,
    content: LayoutContent,
    children: Vec<NodeId>,
}
/// Stateful per-tree adapter. Dirty text nodes also invalidate font/measurement changes without an edit.
pub struct TaffyLayout {
    tree: TaffyTree<LayoutContent>,
    entries: BTreeMap<NodeIdPath, Entry>,
}
impl Default for TaffyLayout {
    fn default() -> Self {
        Self::new()
    }
}
impl TaffyLayout {
    pub fn new() -> Self {
        let mut tree = TaffyTree::new();
        tree.disable_rounding();
        Self {
            tree,
            entries: BTreeMap::new(),
        }
    }
    /// Typed input failures; no stale or partial boxes on an invalid tree.
    pub fn try_layout(
        &mut self,
        input: &LayoutTree,
        dirty: &DirtySet,
        text: &dyn TextEngine,
    ) -> Result<LayoutResult, LayoutError> {
        let prepared = layout::prepare(input)?;
        // Detach changed old child lists before attaching any new ones. A valid edit can reverse
        // old ancestry; attaching first could temporarily form a cycle in Taffy's parent links.
        let mut desired = BTreeMap::new();
        fn topology(n: &PreparedNode, out: &mut BTreeMap<NodeIdPath, Vec<NodeIdPath>>) {
            out.insert(
                n.id.clone(),
                n.children.iter().map(|c| c.id.clone()).collect(),
            );
            for c in &n.children {
                topology(c, out);
            }
        }
        topology(&prepared.root, &mut desired);
        let detach: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(address, entry)| {
                let wanted = desired.get(address).and_then(|children| {
                    children
                        .iter()
                        .map(|id| self.entries.get(id).map(|e| e.id))
                        .collect::<Option<Vec<_>>>()
                });
                (wanted.as_ref() != Some(&entry.children)).then_some(entry.id)
            })
            .collect();
        for id in detach {
            self.tree.set_children(id, &[]).expect("owned id");
        }
        let mut seen = BTreeSet::new();
        let root = self.sync(&prepared.root, None, dirty, &mut seen);
        let removed: Vec<_> = self
            .entries
            .keys()
            .filter(|id| !seen.contains(*id))
            .cloned()
            .collect();
        for id in removed {
            let e = self.entries.remove(&id).unwrap();
            self.tree.remove(e.id).expect("owned Taffy id");
        }
        self.tree
            .compute_layout_with_measure(root, Size::MAX_CONTENT, |inputs, _, context, style| {
                compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        let Some(LayoutContent::Text(rich)) = context else {
                            return Size::ZERO;
                        };
                        let width = known.width.unwrap_or(match available.width {
                            AvailableSpace::Definite(w) => w,
                            AvailableSpace::MinContent => 0.0,
                            AvailableSpace::MaxContent => f32::INFINITY,
                        });
                        let metrics = text.measure(rich, Extent::with_width(width as f64));
                        Size {
                            width: known.width.unwrap_or(metrics.width as f32),
                            height: known.height.unwrap_or(metrics.height as f32),
                        }
                    },
                )
            })
            .expect("owned Taffy root");
        let mut result = LayoutResult {
            problems: prepared.problems.clone(),
            ..Default::default()
        };
        self.collect(&prepared.root, true, true, &mut result);
        Ok(result)
    }
    fn sync(
        &mut self,
        n: &PreparedNode,
        parent: Option<&PreparedNode>,
        dirty: &DirtySet,
        seen: &mut BTreeSet<NodeIdPath>,
    ) -> NodeId {
        seen.insert(n.id.clone());
        // Existing child lists were detached before reconciliation when topology changed.
        let children: Vec<_> = n
            .children
            .iter()
            .map(|c| self.sync(c, Some(n), dirty, seen))
            .collect();
        let style = mapping::style(n, parent);
        if let Some(e) = self.entries.get_mut(&n.id) {
            if e.style != style {
                self.tree.set_style(e.id, style.clone()).expect("owned id");
                e.style = style;
            }
            if e.content != n.content {
                self.tree
                    .set_node_context(e.id, Some(n.content.clone()))
                    .expect("owned id");
                e.content = n.content.clone();
            }
            // The live list may have changed through another parent's move, even when the old input matches.
            if e.children != children || self.tree.children(e.id).expect("owned id") != children {
                self.tree.set_children(e.id, &children).expect("owned ids");
                e.children = children;
            }
            if dirty.all || dirty.nodes.contains(&n.id) {
                self.tree.mark_dirty(e.id).expect("owned id");
            }
            e.id
        } else {
            let id = self
                .tree
                .new_with_children(style.clone(), &children)
                .expect("valid style");
            self.tree
                .set_node_context(id, Some(n.content.clone()))
                .expect("new id");
            self.entries.insert(
                n.id.clone(),
                Entry {
                    id,
                    style,
                    content: n.content.clone(),
                    children,
                },
            );
            id
        }
    }
    fn collect(&self, n: &PreparedNode, shown: bool, root: bool, result: &mut LayoutResult) {
        let shown = shown && n.style.visible;
        if shown {
            let l = self.tree.layout(self.entries[&n.id].id).expect("owned id");
            result.boxes.insert(
                n.id.clone(),
                ports::Rect::new(
                    if root { n.style.x } else { l.location.x as f64 },
                    if root { n.style.y } else { l.location.y as f64 },
                    l.size.width as f64,
                    l.size.height as f64,
                ),
            );
        }
        for c in &n.children {
            self.collect(c, shown, false, result);
        }
    }
}
impl LayoutEngine for TaffyLayout {
    fn layout(
        &mut self,
        tree: &LayoutTree,
        dirty: &DirtySet,
        text: &dyn TextEngine,
    ) -> LayoutResult {
        self.try_layout(tree, dirty, text)
            .unwrap_or_else(|error| LayoutResult {
                problems: vec![LayoutProblem::Invalid(error)],
                ..Default::default()
            })
    }
}
