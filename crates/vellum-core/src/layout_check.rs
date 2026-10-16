//! Structured, read-only layout inspection in model coordinates. No store or document mutations.
use crate::{
    layout::{self, Axis, LayoutError, LayoutProblem},
    ports::*,
    resolver::ResolvedNode,
    scene::{
        self, Scene, SceneError,
        geometry::{self, Affine},
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use vellum_types::{Layout, Node, NodeIdPath, Position};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    TextOverflow {
        node: NodeIdPath,
        horizontal: bool,
        vertical: bool,
    },
    ClippedChild {
        node: NodeIdPath,
        parent: NodeIdPath,
    },
    OverlappingSiblings {
        parent: NodeIdPath,
        first: NodeIdPath,
        second: NodeIdPath,
    },
    OutsideFrame {
        node: NodeIdPath,
    },
    HugCycle {
        parent: NodeIdPath,
        child: NodeIdPath,
        axis: Axis,
    },
    InvalidLayout(LayoutError),
}
impl Problem {
    /// Every node the problem is about.
    pub fn nodes(&self) -> Vec<&NodeIdPath> {
        match self {
            Self::TextOverflow { node, .. } | Self::OutsideFrame { node } => vec![node],
            Self::ClippedChild { node, parent } => vec![node, parent],
            Self::OverlappingSiblings {
                parent,
                first,
                second,
            } => vec![parent, first, second],
            Self::HugCycle { parent, child, .. } => vec![parent, child],
            Self::InvalidLayout(e) => vec![match e {
                LayoutError::MissingFallback { node, .. }
                | LayoutError::InvalidNumber { node, .. }
                | LayoutError::InvalidGrid { node }
                | LayoutError::DuplicateAddress { node }
                | LayoutError::UnresolvedValue { node }
                | LayoutError::TextChildren { node } => node,
            }],
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct NodeSnapshot {
    pub node: NodeIdPath,
    pub parent: Option<NodeIdPath>,
    pub bounds: Rect,
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub nodes: Vec<NodeSnapshot>,
    pub problems: Vec<Problem>,
}
impl Report {
    /// Retains every participant in a diagnostic, in original paint order. Geometry stays unquantized.
    pub fn problems_only(&self) -> Self {
        let ids: BTreeSet<_> = self.problems.iter().flat_map(Problem::nodes).collect();
        Self {
            nodes: self
                .nodes
                .iter()
                .filter(|n| ids.contains(&n.node))
                .cloned()
                .collect(),
            problems: self.problems.clone(),
        }
    }
}
/// Tests full transformed layout boxes, even when a clip has hidden a child completely. Merely touching
/// edges is not overlap. Absolute children are excluded from auto-layout sibling checks.
pub fn check<S: SpatialIndex>(
    roots: &[Arc<ResolvedNode>],
    result: &LayoutResult,
    text: &dyn TextEngine,
    index: S,
) -> Result<Report, SceneError> {
    let nodes = scene::from_resolved(roots, result)?;
    let mut validation = Scene::new(index);
    validation.sync(&nodes)?;
    let mut world = BTreeMap::new();
    let mut clips: BTreeMap<NodeIdPath, Vec<vellum_types::Point>> = BTreeMap::new();
    let mut polygons = BTreeMap::new();
    let mut report = Report::default();
    for n in &nodes {
        let transform = n
            .parent
            .as_ref()
            .and_then(|p| world.get(p))
            .copied()
            .unwrap_or(Affine::IDENTITY)
            .then(Affine::local(n.bounds, n.rotation, n.flip_x, n.flip_y));
        let poly = geometry::corners(Rect::new(0., 0., n.bounds.width, n.bounds.height))
            .into_iter()
            .map(|p| transform.point(p))
            .collect::<Vec<_>>();
        report.nodes.push(NodeSnapshot {
            node: n.address.clone(),
            parent: n.parent.clone(),
            bounds: geometry::bounds(&poly).unwrap(),
        });
        if n.clip {
            clips.insert(
                n.address.clone(),
                geometry::rounded(n.bounds.width, n.bounds.height, n.clip_radii)
                    .into_iter()
                    .map(|p| transform.point(p))
                    .collect(),
            );
        }
        polygons.insert(n.address.clone(), poly);
        world.insert(n.address.clone(), transform);
    }
    // A separate injected index contains raw bounds: clipped-away nodes must remain diagnosable.
    // `validation` owns the supplied index; raw candidate queries are furnished by a scene with clipping off.
    let mut raw = nodes.clone();
    for n in &mut raw {
        n.clip = false;
    }
    validation.sync(&raw)?;
    let mut resolved = BTreeMap::new();
    fn collect<'a>(n: &'a ResolvedNode, out: &mut BTreeMap<NodeIdPath, &'a ResolvedNode>) {
        if !n.value.visible {
            return;
        }
        out.insert(n.address.clone(), n);
        for c in &n.children {
            collect(c, out);
        }
    }
    for root in roots {
        collect(root, &mut resolved);
    }
    let parents: BTreeMap<_, _> = nodes
        .iter()
        .map(|n| (n.address.clone(), n.parent.clone()))
        .collect();
    for n in &report.nodes {
        let own = &resolved[&n.node].value.node;
        let poly = &polygons[&n.node];
        let mut parent = n.parent.as_ref();
        while let Some(p) = parent {
            if let Some(clip) = clips.get(p)
                && poly.iter().any(|v| !geometry::contains(clip, *v))
            {
                report.problems.push(Problem::ClippedChild {
                    node: n.node.clone(),
                    parent: p.clone(),
                });
            }
            parent = parents[p].as_ref();
        }
        if !matches!(own, Node::Frame(_)) {
            let in_frame = validation.cull(n.bounds).iter().any(|id| {
                matches!(resolved[id].value.node, Node::Frame(_))
                    && poly.iter().all(|p| geometry::contains(&polygons[id], *p))
            });
            if !in_frame {
                report.problems.push(Problem::OutsideFrame {
                    node: n.node.clone(),
                });
            }
        }
        if matches!(own, Node::Text(_)) {
            let tree = layout::from_resolved(resolved[&n.node])
                .map_err(|_| SceneError::UnresolvedValue(n.node.clone()))?;
            if let LayoutContent::Text(rich) = &tree.root.content {
                let b = result.boxes[&n.node];
                let t = text.layout_box(rich, Extent::new(b.width, b.height));
                if t.overflows_x || t.overflows_y {
                    report.problems.push(Problem::TextOverflow {
                        node: n.node.clone(),
                        horizontal: t.overflows_x,
                        vertical: t.overflows_y,
                    });
                }
            }
        }
    }
    for n in &nodes {
        let r = resolved[&n.address];
        if let Node::Frame(f) = &r.value.node
            && f.layout != Layout::None
        {
            let children = r
                .children
                .iter()
                .filter(|c| {
                    c.value.visible && !matches!(c.value.node.position(), Position::Absolute(_))
                })
                .collect::<Vec<_>>();
            let order: BTreeMap<_, _> = children
                .iter()
                .enumerate()
                .map(|(i, c)| (&c.address, i))
                .collect();
            for (i, a) in children.iter().enumerate() {
                let bounds = geometry::bounds(&polygons[&a.address]).unwrap();
                for b in validation.cull(bounds) {
                    if order.get(&b).is_some_and(|j| *j > i) {
                        let intersection = geometry::clip(&polygons[&a.address], &polygons[&b]);
                        if positive_area(&intersection) {
                            report.problems.push(Problem::OverlappingSiblings {
                                parent: n.address.clone(),
                                first: a.address.clone(),
                                second: b,
                            });
                        }
                    }
                }
            }
        }
    }
    for p in &result.problems {
        report.problems.push(match p {
            LayoutProblem::HugCycleBroken {
                parent,
                child,
                axis,
            } => Problem::HugCycle {
                parent: parent.clone(),
                child: child.clone(),
                axis: *axis,
            },
            LayoutProblem::Invalid(e) => Problem::InvalidLayout(e.clone()),
        });
    }
    Ok(report)
}
fn positive_area(p: &[vellum_types::Point]) -> bool {
    let Some(o) = p.first() else { return false };
    p.windows(2)
        .map(|w| (w[0].x - o.x) * (w[1].y - o.y) - (w[0].y - o.y) * (w[1].x - o.x))
        .sum::<f64>()
        .abs()
        > 1e-9
}
