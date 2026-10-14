//! Auto-layout. Today: Taffy. Alternatives: Yoga, our own. Sizing rules are ADR-002's.

use std::collections::{BTreeMap, BTreeSet};

use vellum_types::{Align, Justify, Layout, NodeIdPath};

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
    /// Equal grid column count, required for a grid.
    pub columns: Option<u32>,
    /// Equal grid row count, or auto rows when absent.
    pub rows: Option<u32>,
    /// Outside auto-layout flow, anchored at x/y from the parent's outer edge.
    pub absolute: bool,
    /// Hidden nodes and descendants have no boxes and take no space.
    pub visible: bool,
    /// Drawing clip; does not affect sizing.
    pub clip: bool,
    /// Include this frame's inward stroke alongside padding (ADR-012).
    pub layout_includes_stroke: bool,
    /// Inward part of the resolved stroke: full inside, half center, zero outside.
    pub stroke: Insets,
}
impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            mode: Layout::None,
            width: Dimension::Fixed(0.0),
            height: Dimension::Fixed(0.0),
            x: 0.0,
            y: 0.0,
            gap: 0.0,
            padding: Insets::default(),
            justify: Justify::Start,
            align: Align::Start,
            columns: None,
            rows: None,
            absolute: false,
            visible: true,
            clip: false,
            layout_includes_stroke: false,
            stroke: Insets::default(),
        }
    }
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
    /// The resolved occurrence it stands for; component instances cannot collide.
    pub id: NodeIdPath,
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
    pub nodes: BTreeSet<NodeIdPath>,
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
    pub fn of<I: Into<NodeIdPath>>(nodes: impl IntoIterator<Item = I>) -> Self {
        Self {
            all: false,
            nodes: nodes.into_iter().map(Into::into).collect(),
        }
    }
}

/// Where every node ended up.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutResult {
    /// One box per visible occurrence. `x` and `y` are from the parent's top-left corner
    /// (the root's from its own `x` and `y`), as a node's `position` reads in the file.
    pub boxes: BTreeMap<NodeIdPath, Rect>,
    /// Invalid inputs or a dependency cycle broken using ADR-002’s fallback rule.
    pub problems: Vec<crate::layout::LayoutProblem>,
}

/// Lays a tree out.
pub trait LayoutEngine {
    /// Boxes for every visible occurrence. `dirty` says what changed since the previous call, so an
    /// incremental engine may reuse the rest; the result must equal a full layout of `tree`.
    fn layout(
        &mut self,
        tree: &LayoutTree,
        dirty: &DirtySet,
        text: &dyn TextEngine,
    ) -> LayoutResult;
}
