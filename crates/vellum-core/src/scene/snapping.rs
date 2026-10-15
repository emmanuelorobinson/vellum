use super::{Scene, SceneError};
use crate::ports::{Rect, SpatialIndex};
use std::collections::BTreeSet;
use vellum_types::{NodeIdPath, Point};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SnapAxis {
    X,
    Y,
}
/// Order is the deterministic tie-break when corrections have the same distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SnapKind {
    Edge,
    Centre,
    EqualSpacing,
    Grid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SnapGuide {
    pub axis: SnapAxis,
    pub kind: SnapKind,
    /// The moving anchor's coordinate after snapping (centre for spacing, origin for grid).
    pub coordinate: f64,
    /// Target occurrences; empty for pixel grid, two for equal spacing.
    pub targets: Vec<NodeIdPath>,
    /// Gap in model pixels for an equal-spacing guide.
    pub gap: Option<f64>,
}
/// Host preferences. The search rectangle normally is the viewport; there is no document-wide fallback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapOptions {
    pub search: Rect,
    /// Screen pixels, divided by zoom before comparing model distances.
    pub tolerance: f64,
    pub zoom: f64,
    pub edges: bool,
    pub centres: bool,
    pub equal_spacing: bool,
    /// None disables pixel grid. Some(1) snaps the moving origin to whole model pixels.
    pub grid: Option<f64>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SnapResult {
    pub offset: Point,
    pub guides: Vec<SnapGuide>,
}
#[derive(Clone)]
struct Choice {
    delta: f64,
    guide: SnapGuide,
}
impl<S: SpatialIndex> Scene<S> {
    /// Translation only; never mutates nodes. Selected subtrees are excluded using scene ancestry.
    pub fn snap(
        &self,
        moving: Rect,
        exclude: &BTreeSet<NodeIdPath>,
        o: SnapOptions,
    ) -> Result<SnapResult, SceneError> {
        if !moving.is_valid()
            || !o.search.is_valid()
            || !o.tolerance.is_finite()
            || o.tolerance < 0.
            || !o.zoom.is_finite()
            || o.zoom <= 0.
            || o.grid.is_some_and(|g| !g.is_finite() || g <= 0.)
        {
            return Err(SceneError::InvalidQuery);
        }
        let tolerance = o.tolerance / o.zoom;
        if !tolerance.is_finite() {
            return Err(SceneError::InvalidQuery);
        }
        let search = o.search.expand(tolerance);
        if !search.is_valid() {
            return Err(SceneError::InvalidQuery);
        }
        let mut candidates = self
            .index
            .query_rect(search)
            .into_iter()
            .filter(|id| !self.excluded(id, exclude))
            .filter_map(|id| {
                self.entries
                    .get(&id)
                    .filter(|e| e.visible && e.selectable)
                    .and_then(|e| e.bounds)
                    .map(|r| (id, r))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        candidates.dedup_by(|a, b| a.0 == b.0);
        let mut offset = Point { x: 0., y: 0. };
        let mut guides = vec![];
        for axis in [SnapAxis::X, SnapAxis::Y] {
            let (start, size, other, other_end) = coords(moving, axis);
            let mut best: Option<Choice> = None;
            let mut offer = |delta: f64,
                             kind: SnapKind,
                             coordinate: f64,
                             targets: Vec<NodeIdPath>,
                             gap: Option<f64>| {
                if !delta.is_finite() || delta.abs() > tolerance || !coordinate.is_finite() {
                    return;
                }
                let next = Choice {
                    delta,
                    guide: SnapGuide {
                        axis,
                        kind,
                        coordinate,
                        targets,
                        gap,
                    },
                };
                let key = |c: &Choice| (c.guide.kind, c.guide.targets.clone());
                if best.as_ref().is_none_or(|b| {
                    delta
                        .abs()
                        .total_cmp(&b.delta.abs())
                        .then_with(|| key(&next).cmp(&key(b)))
                        .is_lt()
                }) {
                    best = Some(next);
                }
            };
            for (id, r) in &candidates {
                let (s, w, _, _) = coords(*r, axis);
                if o.edges {
                    for from in [start, start + size] {
                        for to in [s, s + w] {
                            offer(to - from, SnapKind::Edge, to, vec![id.clone()], None);
                        }
                    }
                }
                if o.centres {
                    let to = s + w / 2.;
                    offer(
                        to - (start + size / 2.),
                        SnapKind::Centre,
                        to,
                        vec![id.clone()],
                        None,
                    );
                }
            }
            if o.equal_spacing {
                let mut neighbors = candidates
                    .iter()
                    .filter(|(_, r)| {
                        let (_, _, a, b) = coords(*r, axis);
                        a < other_end && b > other
                    })
                    .collect::<Vec<_>>();
                neighbors.sort_by(|a, b| {
                    coords(a.1, axis)
                        .0
                        .total_cmp(&coords(b.1, axis).0)
                        .then_with(|| a.0.cmp(&b.0))
                });
                for pair in neighbors.windows(2) {
                    let (left, lw, la, lb) = coords(pair[0].1, axis);
                    let (right, rw, ra, rb) = coords(pair[1].1, axis);
                    let gap = right - left - lw;
                    if gap < 0. || la >= rb || ra >= lb {
                        continue;
                    }
                    let targets = vec![pair[0].0.clone(), pair[1].0.clone()];
                    let between = (left + lw + right - size) / 2.;
                    let inner_gap = (right - left - lw - size) / 2.;
                    if inner_gap >= 0. {
                        offer(
                            between - start,
                            SnapKind::EqualSpacing,
                            between + size / 2.,
                            targets.clone(),
                            Some(inner_gap),
                        );
                    }
                    let after = right + rw + gap;
                    offer(
                        after - start,
                        SnapKind::EqualSpacing,
                        after + size / 2.,
                        targets.clone(),
                        Some(gap),
                    );
                    let before = left - gap - size;
                    offer(
                        before - start,
                        SnapKind::EqualSpacing,
                        before + size / 2.,
                        targets,
                        Some(gap),
                    );
                }
            }
            if let Some(grid) = o.grid {
                let to = (start / grid).round() * grid;
                offer(to - start, SnapKind::Grid, to, vec![], None);
            }
            if let Some(best) = best {
                match axis {
                    SnapAxis::X => offset.x = best.delta,
                    SnapAxis::Y => offset.y = best.delta,
                };
                guides.push(best.guide);
            }
        }
        Ok(SnapResult { offset, guides })
    }
}
fn coords(r: Rect, axis: SnapAxis) -> (f64, f64, f64, f64) {
    match axis {
        SnapAxis::X => (r.x, r.width, r.y, r.bottom()),
        SnapAxis::Y => (r.y, r.height, r.x, r.right()),
    }
}
