use vellum_core::ports::{Path, PathOps, PathSegment, Rect, StrokeStyle};
use vellum_types::{BoolOp, StrokeAlign};

/// Booleans return the first path unchanged. Bounds are those of the end points and control points, which is
/// never smaller than the true bounds of a curve.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdentityPathOps;

impl PathOps for IdentityPathOps {
    fn boolean(&self, a: &Path, _b: &Path, _op: BoolOp) -> Path {
        a.clone()
    }

    fn bounds(&self, path: &Path, stroke: Option<&StrokeStyle>) -> Option<Rect> {
        let mut points = Vec::new();
        for segment in &path.segments {
            match segment {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => points.push(*p),
                PathSegment::QuadTo(c, p) => points.extend([*c, *p]),
                PathSegment::CubicTo(c1, c2, p) => points.extend([*c1, *c2, *p]),
                PathSegment::Close => {}
            }
        }
        let first = points.first()?;
        let (mut left, mut top, mut right, mut bottom) = (first.x, first.y, first.x, first.y);
        for p in &points {
            left = left.min(p.x);
            top = top.min(p.y);
            right = right.max(p.x);
            bottom = bottom.max(p.y);
        }
        let fill = Rect::new(left, top, right - left, bottom - top);
        Some(match stroke {
            None => fill,
            Some(s) => fill.expand(match s.align {
                StrokeAlign::Inside => 0.0,
                StrokeAlign::Center => s.width / 2.0,
                StrokeAlign::Outside => s.width,
            }),
        })
    }
}
