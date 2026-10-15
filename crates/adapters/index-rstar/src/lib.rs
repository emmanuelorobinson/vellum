//! R-tree spatial search. Only this adapter knows rstar; scene semantics stay in core.
use rstar::{AABB, RTree, RTreeObject};
use std::collections::BTreeMap;
use vellum_core::ports::{Rect, SpatialIndex};
use vellum_types::NodeIdPath;

/// The Cargo package name.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
#[derive(Debug, Clone, PartialEq)]
struct Item {
    address: NodeIdPath,
    bounds: Rect,
}
impl RTreeObject for Item {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        envelope(self.bounds)
    }
}
fn envelope(r: Rect) -> AABB<[f64; 2]> {
    AABB::from_corners([r.x, r.y], [r.right(), r.bottom()])
}
/// Dynamic index with an address-to-bounds lookup for removing old envelopes in O(log n).
#[derive(Debug, Default)]
pub struct RstarIndex {
    tree: RTree<Item>,
    bounds: BTreeMap<NodeIdPath, Rect>,
}
impl RstarIndex {
    /// An empty spatial index.
    pub fn new() -> Self {
        Self::default()
    }
}
impl SpatialIndex for RstarIndex {
    fn rebuild(&mut self, items: &[(NodeIdPath, Rect)]) {
        self.bounds.clear();
        for (id, r) in items {
            self.bounds.remove(id);
            if r.is_valid() {
                self.bounds.insert(id.clone(), *r);
            }
        }
        self.tree = RTree::bulk_load(
            self.bounds
                .iter()
                .map(|(id, r)| Item {
                    address: id.clone(),
                    bounds: *r,
                })
                .collect(),
        );
    }
    fn update(&mut self, changed: &[(NodeIdPath, Rect)], removed: &[NodeIdPath]) {
        for (id, r) in changed {
            if let Some(old) = self.bounds.remove(id) {
                self.tree.remove(&Item {
                    address: id.clone(),
                    bounds: old,
                });
            }
            if r.is_valid() {
                self.tree.insert(Item {
                    address: id.clone(),
                    bounds: *r,
                });
                self.bounds.insert(id.clone(), *r);
            }
        }
        for id in removed {
            if let Some(old) = self.bounds.remove(id) {
                self.tree.remove(&Item {
                    address: id.clone(),
                    bounds: old,
                });
            }
        }
    }
    fn query_rect(&self, r: Rect) -> Vec<NodeIdPath> {
        if !r.is_valid() {
            return vec![];
        }
        self.tree
            .locate_in_envelope_intersecting(envelope(r))
            .map(|i| i.address.clone())
            .collect()
    }
    fn query_point(&self, x: f64, y: f64) -> Vec<NodeIdPath> {
        self.query_rect(Rect::new(x, y, 0., 0.))
    }
}
