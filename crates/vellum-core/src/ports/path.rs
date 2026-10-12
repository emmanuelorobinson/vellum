//! Path booleans and geometry. Today: kurbo and i_overlay.

use vellum_types::{BoolOp, Point, StrokeAlign, StrokeCap, StrokeJoin};

use crate::ports::Rect;

/// One step of a path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    /// Starts a new sub-path.
    MoveTo(Point),
    /// A straight line.
    LineTo(Point),
    /// A quadratic curve: control point, end point.
    QuadTo(Point, Point),
    /// A cubic curve: two control points, end point.
    CubicTo(Point, Point, Point),
    /// A line back to the start of the sub-path.
    Close,
}

/// A vector path in model coordinates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path {
    /// The steps, in order.
    pub segments: Vec<PathSegment>,
}

impl Path {
    /// A closed rectangle.
    pub fn rect(x: f64, y: f64, width: f64, height: f64) -> Self {
        let p = |x, y| Point { x, y };
        Self {
            segments: vec![
                PathSegment::MoveTo(p(x, y)),
                PathSegment::LineTo(p(x + width, y)),
                PathSegment::LineTo(p(x + width, y + height)),
                PathSegment::LineTo(p(x, y + height)),
                PathSegment::Close,
            ],
        }
    }

    /// Whether the path draws nothing.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

/// A stroke's geometry with every variable resolved. Paints are the renderer's business, not geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeStyle {
    /// Width in pixels.
    pub width: f64,
    /// Inside, centred on or outside the edge.
    pub align: StrokeAlign,
    /// The end of an open stroke.
    pub cap: StrokeCap,
    /// The corner of a stroke.
    pub join: StrokeJoin,
}

/// Path booleans and bounds.
pub trait PathOps {
    /// `a` combined with `b`. The result lies within the union of the inputs' bounds.
    fn boolean(&self, a: &Path, b: &Path, op: BoolOp) -> Path;
    /// Expands the stroke geometry to a filled path. Adapters without stroking refuse explicitly.
    /// Dash and per-side stroke geometry are not represented by StrokeStyle and must be refused by
    /// the caller rather than silently outlined as a solid uniform stroke.
    fn outline_stroke(&self, _path: &Path, _stroke: &StrokeStyle) -> Result<Path, String> {
        Err("stroke outlining is not implemented by this adapter".into())
    }
    /// The smallest rectangle holding the path, grown to hold its stroke when there is one. None for a path
    /// that draws nothing.
    fn bounds(&self, path: &Path, stroke: Option<&StrokeStyle>) -> Option<Rect>;
}
