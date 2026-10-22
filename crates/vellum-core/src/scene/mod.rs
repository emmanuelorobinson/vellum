//! Read-only world-space scene queries over resolved occurrence bounds. Document edits remain Ops.
//! Selection uses transformed layout boxes; path/fill coverage belongs to the geometry/rendering layer.
pub(crate) mod geometry;
mod resolved;
mod snapping;
mod space;
use crate::ports::{Rect, SpatialIndex};
use geometry::{Affine, Polygon};
pub use resolved::from_resolved;
pub use snapping::{SnapAxis, SnapGuide, SnapKind, SnapOptions, SnapResult};
use std::collections::{BTreeMap, BTreeSet};
use vellum_types::{NodeIdPath, Point};

/// One occurrence in a page. Roots and siblings are supplied back to front; parents need not precede children.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    pub address: NodeIdPath,
    pub parent: Option<NodeIdPath>,
    /// Layout box relative to parent, before transforms.
    pub bounds: Rect,
    /// Clockwise degrees, around this box's centre.
    pub rotation: f64,
    pub flip_x: bool,
    pub flip_y: bool,
    pub visible: bool,
    pub locked: bool,
    pub clip: bool,
    /// Top-left, top-right, bottom-right, bottom-left; used only for child clipping.
    pub clip_radii: [f64; 4],
    /// False for session overlays such as planned-frame reservations; still blocks `find_space`.
    pub selectable: bool,
}
impl SceneNode {
    pub fn default_for(address: NodeIdPath) -> Self {
        Self {
            address,
            parent: None,
            bounds: Rect::default(),
            rotation: 0.,
            flip_x: false,
            flip_y: false,
            visible: true,
            locked: false,
            clip: false,
            clip_radii: [0.; 4],
            selectable: true,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitDepth {
    TopLevel,
    Deepest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marquee {
    Intersects,
    Contained,
}
/// Invalid inputs leave the previous scene and index intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneError {
    DuplicateAddress(NodeIdPath),
    MissingParent(NodeIdPath),
    Cycle(NodeIdPath),
    InvalidGeometry(NodeIdPath),
    MissingLayout(NodeIdPath),
    UnresolvedValue(NodeIdPath),
    InvalidQuery,
    SearchLimit,
}
#[derive(Debug, Clone)]
struct Entry {
    parent: Option<NodeIdPath>,
    top: NodeIdPath,
    transform: Affine,
    /// The world box around the occurrence's own transformed box, ignoring clipping and visibility.
    world: Rect,
    polygon: Polygon,
    clip: Option<Polygon>,
    bounds: Option<Rect>,
    visible: bool,
    locked: bool,
    selectable: bool,
    z: usize,
}
/// What an editor knows about one occurrence: where it is on the canvas and whether a person can pick it.
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    pub parent: Option<NodeIdPath>,
    /// The world box around its transformed box, before clipping; hidden occurrences keep theirs.
    pub bounds: Rect,
    pub visible: bool,
    /// Locked here or by an ancestor.
    pub locked: bool,
    pub selectable: bool,
}
/// One scene per active page. Reconciliation scans prepared nodes; only changed envelopes reach the index.
pub struct Scene<S> {
    index: S,
    entries: BTreeMap<NodeIdPath, Entry>,
}
impl<S: SpatialIndex> Scene<S> {
    /// Takes ownership of an empty index.
    pub fn new(index: S) -> Self {
        Self {
            index,
            entries: BTreeMap::new(),
        }
    }
    pub fn sync(&mut self, nodes: &[SceneNode]) -> Result<(), SceneError> {
        let mut source = BTreeMap::new();
        let mut children: BTreeMap<Option<NodeIdPath>, Vec<&SceneNode>> = BTreeMap::new();
        for node in nodes {
            if !node.bounds.is_valid()
                || !node.rotation.is_finite()
                || node.clip_radii.iter().any(|r| !r.is_finite() || *r < 0.)
            {
                return Err(SceneError::InvalidGeometry(node.address.clone()));
            }
            if source.insert(node.address.clone(), node).is_some() {
                return Err(SceneError::DuplicateAddress(node.address.clone()));
            }
            children.entry(node.parent.clone()).or_default().push(node);
        }
        for node in nodes {
            if let Some(parent) = &node.parent
                && !source.contains_key(parent)
            {
                return Err(SceneError::MissingParent(parent.clone()));
            }
        }
        let mut next = BTreeMap::new();
        let mut z = 0;
        fn visit(
            n: &SceneNode,
            parent: Option<&Entry>,
            children: &BTreeMap<Option<NodeIdPath>, Vec<&SceneNode>>,
            out: &mut BTreeMap<NodeIdPath, Entry>,
            z: &mut usize,
        ) -> Result<(), SceneError> {
            let transform = parent
                .map_or(Affine::IDENTITY, |p| p.transform)
                .then(Affine::local(n.bounds, n.rotation, n.flip_x, n.flip_y));
            let own = geometry::corners(Rect::new(0., 0., n.bounds.width, n.bounds.height))
                .into_iter()
                .map(|p| transform.point(p))
                .collect::<Vec<_>>();
            if own.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
                return Err(SceneError::InvalidGeometry(n.address.clone()));
            }
            let visible = n.visible && parent.is_none_or(|p| p.visible);
            let polygon = if let Some(c) = parent.and_then(|p| p.clip.as_ref()) {
                geometry::clip(&own, c)
            } else {
                own.clone()
            };
            let bounds = if visible {
                geometry::bounds(&polygon)
            } else {
                None
            };
            if bounds.is_some_and(|r| !r.is_valid()) {
                return Err(SceneError::InvalidGeometry(n.address.clone()));
            }
            let world = geometry::bounds(&own)
                .ok_or_else(|| SceneError::InvalidGeometry(n.address.clone()))?;
            let entry = Entry {
                parent: n.parent.clone(),
                top: parent.map_or_else(|| n.address.clone(), |p| p.top.clone()),
                transform,
                world,
                polygon: polygon.clone(),
                clip: if n.clip {
                    Some({
                        let own_clip =
                            geometry::rounded(n.bounds.width, n.bounds.height, n.clip_radii)
                                .into_iter()
                                .map(|p| transform.point(p))
                                .collect::<Vec<_>>();
                        if let Some(c) = parent.and_then(|p| p.clip.as_ref()) {
                            geometry::clip(&own_clip, c)
                        } else {
                            own_clip
                        }
                    })
                } else {
                    parent.and_then(|p| p.clip.clone())
                },
                bounds,
                visible,
                locked: n.locked || parent.is_some_and(|p| p.locked),
                selectable: n.selectable && parent.is_none_or(|p| p.selectable),
                z: *z,
            };
            *z += 1;
            out.insert(n.address.clone(), entry.clone());
            if let Some(kids) = children.get(&Some(n.address.clone())) {
                for kid in kids {
                    visit(kid, Some(&entry), children, out, z)?;
                }
            }
            Ok(())
        }
        if let Some(roots) = children.get(&None) {
            for root in roots {
                visit(root, None, &children, &mut next, &mut z)?;
            }
        }
        if next.len() != nodes.len() {
            return Err(SceneError::Cycle(
                nodes
                    .iter()
                    .find(|n| !next.contains_key(&n.address))
                    .unwrap()
                    .address
                    .clone(),
            ));
        }
        let changed = next
            .iter()
            .filter_map(|(id, e)| {
                e.bounds
                    .filter(|b| self.entries.get(id).and_then(|p| p.bounds) != Some(*b))
                    .map(|b| (id.clone(), b))
            })
            .collect::<Vec<_>>();
        let removed = self
            .entries
            .iter()
            .filter(|(id, e)| {
                e.bounds.is_some() && next.get(*id).is_none_or(|n| n.bounds.is_none())
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        if !changed.is_empty() || !removed.is_empty() {
            self.index.update(&changed, &removed);
        }
        self.entries = next;
        Ok(())
    }
    /// An occurrence, as an editor needs it.
    pub fn occurrence(&self, address: &NodeIdPath) -> Option<Occurrence> {
        self.entries.get(address).map(|e| Occurrence {
            parent: e.parent.clone(),
            bounds: e.world,
            visible: e.visible,
            locked: e.locked,
            selectable: e.selectable,
        })
    }
    /// Top-level occurrences, back to front.
    pub fn roots(&self) -> Vec<NodeIdPath> {
        self.ordered(|e| e.parent.is_none())
    }
    /// An occurrence's children, back to front.
    pub fn children(&self, address: &NodeIdPath) -> Vec<NodeIdPath> {
        self.ordered(|e| e.parent.as_ref() == Some(address))
    }
    fn ordered(&self, keep: impl Fn(&Entry) -> bool) -> Vec<NodeIdPath> {
        let mut found = self
            .entries
            .iter()
            .filter(|(_, e)| keep(e))
            .map(|(id, e)| (e.z, id.clone()))
            .collect::<Vec<_>>();
        found.sort_by_key(|e| e.0);
        found.into_iter().map(|e| e.1).collect()
    }
    /// A world point in the occurrence's own box (before its rotation and flips), which is also where its
    /// children are positioned. None for an unknown occurrence or a box squashed flat.
    pub fn to_local(&self, address: &NodeIdPath, p: Point) -> Option<Point> {
        let inverse = self.entries.get(address)?.transform.invert()?;
        Some(inverse.point(p))
    }
    /// Frontmost eligible box; deep selection follows paint order, rather than globally preferring depth.
    pub fn hit_point(&self, p: Point, depth: HitDepth) -> Option<NodeIdPath> {
        if !p.x.is_finite() || !p.y.is_finite() {
            return None;
        }
        self.select(self.index.query_point(p.x, p.y), depth, |e| {
            geometry::contains(&e.polygon, p)
        })
        .into_iter()
        .next()
    }
    /// Every matching eligible box, front to back; top-level ancestors are deduplicated.
    pub fn hit_rect(&self, r: Rect, depth: HitDepth, mode: Marquee) -> Vec<NodeIdPath> {
        if !r.is_valid() {
            return vec![];
        }
        self.select(self.index.query_rect(r), depth, |e| match mode {
            Marquee::Intersects => geometry::intersects_rect(&e.polygon, r),
            Marquee::Contained => e.polygon.iter().all(|p| r.contains_point(p.x, p.y)),
        })
    }
    fn select(
        &self,
        ids: Vec<NodeIdPath>,
        depth: HitDepth,
        hit: impl Fn(&Entry) -> bool,
    ) -> Vec<NodeIdPath> {
        let mut found = ids
            .into_iter()
            .filter_map(|id| {
                self.entries
                    .get(&id)
                    .filter(|e| e.visible && !e.locked && e.selectable && hit(e))
                    .map(|e| {
                        (
                            e.z,
                            if depth == HitDepth::TopLevel {
                                e.top.clone()
                            } else {
                                id
                            },
                        )
                    })
            })
            .collect::<Vec<_>>();
        found.sort_by_key(|a| std::cmp::Reverse(a.0));
        let mut seen = BTreeSet::new();
        found
            .into_iter()
            .map(|(_, id)| id)
            .filter(|id| seen.insert(id.clone()))
            .collect()
    }
    /// Visible boxes in paint order, including locked layers. Narrow-phase clipping avoids AABB corner hits.
    pub fn cull(&self, viewport: Rect) -> Vec<NodeIdPath> {
        if !viewport.is_valid() {
            return vec![];
        }
        let mut found = self
            .index
            .query_rect(viewport)
            .into_iter()
            .filter_map(|id| {
                self.entries
                    .get(&id)
                    .filter(|e| e.visible && geometry::intersects_rect(&e.polygon, viewport))
                    .map(|e| (e.z, id))
            })
            .collect::<Vec<_>>();
        found.sort_by_key(|e| e.0);
        found.into_iter().map(|e| e.1).collect()
    }
    pub(super) fn excluded(&self, id: &NodeIdPath, exclude: &BTreeSet<NodeIdPath>) -> bool {
        let mut cursor = Some(id);
        while let Some(id) = cursor {
            if exclude.contains(id) {
                return true;
            }
            cursor = self.entries.get(id).and_then(|e| e.parent.as_ref());
        }
        false
    }
}
