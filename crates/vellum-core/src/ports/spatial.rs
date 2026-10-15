//! Hit-testing, culling, snapping. Today: rstar.

use vellum_types::NodeIdPath;

use crate::ports::Rect;

/// Finds nodes by where they are. Rectangles are closed: touching counts (see [`Rect`]).
///
/// Occurrence paths distinguish repeated component descendants. Repeated addresses use the last
/// entry. Nonfinite/negative rectangles are omitted (an invalid update removes the old entry);
/// invalid queries return no results. `removed` wins if an address occurs in both update lists.
///
/// Results come in no particular order; a caller that needs z-order sorts them itself (the scene does).
pub trait SpatialIndex {
    /// Throws away everything and indexes `items`.
    fn rebuild(&mut self, items: &[(NodeIdPath, Rect)]);
    /// Adds or moves the `changed` entries, then drops the `removed` ones. An id in `removed` that the index
    /// does not hold is ignored; removal wins over a simultaneous change.
    fn update(&mut self, changed: &[(NodeIdPath, Rect)], removed: &[NodeIdPath]);
    /// Every node whose rectangle intersects `r`.
    fn query_rect(&self, r: Rect) -> Vec<NodeIdPath>;
    /// Every node whose rectangle contains `(x, y)`.
    fn query_point(&self, x: f64, y: f64) -> Vec<NodeIdPath>;
}
