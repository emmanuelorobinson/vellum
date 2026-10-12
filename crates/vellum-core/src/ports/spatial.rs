//! Hit-testing, culling, snapping. Today: rstar.

use vellum_types::NodeId;

use crate::ports::Rect;

/// Finds nodes by where they are. Rectangles are closed: touching counts (see [`Rect`]).
///
/// Results come in no particular order; a caller that needs z-order sorts them itself (the scene does).
pub trait SpatialIndex {
    /// Throws away everything and indexes `items`.
    fn rebuild(&mut self, items: &[(NodeId, Rect)]);
    /// Adds or moves the `changed` entries, then drops the `removed` ones. An id in `removed` that the index
    /// does not hold is ignored; one id should not be in both lists.
    fn update(&mut self, changed: &[(NodeId, Rect)], removed: &[NodeId]);
    /// Every node whose rectangle intersects `r`.
    fn query_rect(&self, r: Rect) -> Vec<NodeId>;
    /// Every node whose rectangle contains `(x, y)`.
    fn query_point(&self, x: f64, y: f64) -> Vec<NodeId>;
}
