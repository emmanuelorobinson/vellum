//! Auto-layout. Today: Taffy. Alternatives: Yoga, our own. Sizing rules are ADR-002's.

use std::collections::{BTreeMap, BTreeSet};

use vellum_types::{Align, Justify, Layout, NodeId};

use crate::ports::{Rect, RichText, TextEngine};

/// One side of a node's size, resolved to numbers (the resolver has already replaced every variable).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dimension {
    /// This many pixels.
    Fixed(f64),
    /// As large as the content, or the fallback when there is nothing to measure.
    Hug {
        /// Used when hugging has nothing to measure.
        fallback: Option<f64>,
    },
    /// As large as the parent leaves room for, or the fallback when there is nothing to fill.
    Fill {
        /// Used when there is nothing to fill.
        fallback: Option<f64>,
    },
}

/// Space inside a node, on each side.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    /// Top.
    pub top: f64,
    /// Right.
    pub right: f64,
    /// Bottom.
    pub bottom: f64,
    /// Left.
    pub left: f64,
}

/// How a node is sized and how it arranges its children.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutStyle {
    /// None, row, column or grid.
    pub mode: Layout,
    /// Width.
    pub width: Dimension,
    /// Height.
    pub height: Dimension,
    /// Left edge inside the parent. Used when the parent has no layout, else the parent decides.
    pub x: f64,
    /// Top edge inside the parent. Used when the parent has no layout, else the parent decides.
    pub y: f64,
    /// Space between children along the main axis.
    pub gap: f64,
    /// Space between the edge and the children.
    pub padding: Insets,
    /// Where children sit along the main axis.
    pub justify: Justify,
    /// Where children sit across it.
    pub align: Align,
}

/// What a node holds.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutContent {
    /// Children only, or nothing.
    Frame,
    /// Text, measured by the `TextEngine`.
    Text(RichText),
}

/// One node of the tree to lay out.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutNode {
    /// The node it stands for.
    pub id: NodeId,
    /// Sizing and arrangement.
    pub style: LayoutStyle,
    /// What it holds.
    pub content: LayoutContent,
    /// Its children, back to front.
    pub children: Vec<LayoutNode>,
}

/// The resolved frames of one page or one top-level frame, ready for layout.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutTree {
    /// The top node. It has nothing to fill; a `fill` there uses its fallback.
    pub root: LayoutNode,
}

/// Which nodes changed since the last layout.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirtySet {
    /// Everything: the first layout, or a change the caller cannot narrow.
    pub all: bool,
    /// The nodes that changed. The engine works out which ancestors and siblings that moves.
    pub nodes: BTreeSet<NodeId>,
}

impl DirtySet {
    /// Lay everything out again.
    pub fn all() -> Self {
        Self {
            all: true,
            nodes: BTreeSet::new(),
        }
    }

    /// Only these nodes changed.
    pub fn of(nodes: impl IntoIterator<Item = NodeId>) -> Self {
        Self {
            all: false,
            nodes: nodes.into_iter().collect(),
        }
    }
}

/// Where every node ended up.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutResult {
    /// One box per node in the tree. `x` and `y` are from the parent's top-left corner (the root's from its
    /// own `x` and `y`), which is how a node's `position` reads in the file.
    pub boxes: BTreeMap<NodeId, Rect>,
}

/// Lays a tree out.
pub trait LayoutEngine {
    /// Boxes for every node of `tree`. `dirty` says what changed since the previous call on this engine, so an
    /// incremental engine may reuse the rest; the result must equal a full layout of `tree`.
    fn layout(
        &mut self,
        tree: &LayoutTree,
        dirty: &DirtySet,
        text: &dyn TextEngine,
    ) -> LayoutResult;
}
