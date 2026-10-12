use vellum_core::ports::{Rect, SpatialIndex};
use vellum_types::NodeId;

/// A list that every query walks.
#[derive(Debug, Default)]
pub struct NaiveSpatialIndex {
    items: Vec<(NodeId, Rect)>,
}

impl NaiveSpatialIndex {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SpatialIndex for NaiveSpatialIndex {
    fn rebuild(&mut self, items: &[(NodeId, Rect)]) {
        self.items = items.to_vec();
    }

    fn update(&mut self, changed: &[(NodeId, Rect)], removed: &[NodeId]) {
        for (id, rect) in changed {
            match self.items.iter_mut().find(|(have, _)| have == id) {
                Some(entry) => entry.1 = *rect,
                None => self.items.push((id.clone(), *rect)),
            }
        }
        self.items.retain(|(id, _)| !removed.contains(id));
    }

    fn query_rect(&self, r: Rect) -> Vec<NodeId> {
        self.items
            .iter()
            .filter(|(_, have)| have.intersects(&r))
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn query_point(&self, x: f64, y: f64) -> Vec<NodeId> {
        self.items
            .iter()
            .filter(|(_, have)| have.contains_point(x, y))
            .map(|(id, _)| id.clone())
            .collect()
    }
}
