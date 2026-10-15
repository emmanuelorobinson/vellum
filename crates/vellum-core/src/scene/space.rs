use super::{Scene, SceneError};
use crate::ports::{Rect, SpatialIndex};
use std::{
    cmp::Ordering,
    collections::{BTreeSet, BinaryHeap},
};
use vellum_types::Point;
/// Space between a new frame and existing visible bounds, in model pixels.
pub const SPACE_GAP: f64 = 16.;
const MAX_PROBES: usize = 10_000;
#[derive(Clone, Copy)]
struct Candidate {
    x: f64,
    y: f64,
    distance: f64,
}
impl PartialEq for Candidate {
    fn eq(&self, b: &Self) -> bool {
        self.cmp(b) == Ordering::Equal
    }
}
impl Eq for Candidate {}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        Some(self.cmp(b))
    }
}
impl Ord for Candidate {
    fn cmp(&self, b: &Self) -> Ordering {
        b.distance
            .total_cmp(&self.distance)
            .then_with(|| b.y.total_cmp(&self.y))
            .then_with(|| b.x.total_cmp(&self.x))
    }
}
impl<S: SpatialIndex> Scene<S> {
    /// Best-first search from the requested top-left (origin by default). Tests only indexed visible
    /// bounds; locked nodes and nonselectable placeholder reservations also occupy space. Uses a 16 px
    /// gap and deterministic distance/y/x ties. Search is bounded and never returns an occupied fallback.
    pub fn find_space(
        &self,
        width: f64,
        height: f64,
        near: Option<Point>,
    ) -> Result<Rect, SceneError> {
        let origin = near.unwrap_or(Point { x: 0., y: 0. });
        if !Rect::new(origin.x, origin.y, width, height).is_valid() || width <= 0. || height <= 0. {
            return Err(SceneError::InvalidQuery);
        }
        let mut pending = BinaryHeap::new();
        let mut seen = BTreeSet::new();
        let mut push = |pending: &mut BinaryHeap<Candidate>, x: f64, y: f64| {
            if !Rect::new(x, y, width, height).expand(SPACE_GAP).is_valid() {
                return;
            }
            // Normalize signed zero to avoid revisiting the same point.
            let x = if x == 0. { 0. } else { x };
            let y = if y == 0. { 0. } else { y };
            if seen.insert((x.to_bits(), y.to_bits())) {
                pending.push(Candidate {
                    x,
                    y,
                    distance: (x - origin.x).hypot(y - origin.y),
                });
            }
        };
        push(&mut pending, origin.x, origin.y);
        for _ in 0..MAX_PROBES {
            let Some(c) = pending.pop() else {
                return Err(SceneError::SearchLimit);
            };
            let rect = Rect::new(c.x, c.y, width, height);
            let mut obstacles = self
                .index
                .query_rect(rect.expand(SPACE_GAP))
                .into_iter()
                .filter_map(|id| self.entries.get(&id).and_then(|e| e.bounds))
                .filter(|b| {
                    rect.x < b.right() + SPACE_GAP
                        && rect.right() > b.x - SPACE_GAP
                        && rect.y < b.bottom() + SPACE_GAP
                        && rect.bottom() > b.y - SPACE_GAP
                })
                .collect::<Vec<_>>();
            if obstacles.is_empty() {
                return Ok(rect);
            }
            obstacles.sort_by(|a, b| {
                a.x.total_cmp(&b.x)
                    .then_with(|| a.y.total_cmp(&b.y))
                    .then_with(|| a.width.total_cmp(&b.width))
                    .then_with(|| a.height.total_cmp(&b.height))
            });
            for b in obstacles {
                push(&mut pending, b.right() + SPACE_GAP, c.y);
                push(&mut pending, b.x - width - SPACE_GAP, c.y);
                push(&mut pending, c.x, b.bottom() + SPACE_GAP);
                push(&mut pending, c.x, b.y - height - SPACE_GAP);
            }
        }
        Err(SceneError::SearchLimit)
    }
}
