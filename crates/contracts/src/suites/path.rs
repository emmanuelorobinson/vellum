//! `PathOps`.
//!
//! Kept to what every implementation does, the identity fake included. Prompt 12 adds the boolean semantics
//! (areas, holes, winding) for the real implementation.

use vellum_core::ports::{Path, PathOps, PathSegment, Rect, StrokeStyle};
use vellum_types::{BoolOp, Point, StrokeAlign, StrokeCap, StrokeJoin};

const OPS: [BoolOp; 4] = [
    BoolOp::Union,
    BoolOp::Subtract,
    BoolOp::Intersect,
    BoolOp::Exclude,
];

fn stroke(width: f64, align: StrokeAlign) -> StrokeStyle {
    StrokeStyle {
        width,
        align,
        cap: StrokeCap::default(),
        join: StrokeJoin::default(),
    }
}

fn same(a: &Rect, b: &Rect) -> bool {
    a.contains_rect(b, 1e-6) && b.contains_rect(a, 1e-6)
}

fn bounds(ops: &dyn PathOps, path: &Path, stroke: Option<&StrokeStyle>) -> Rect {
    ops.bounds(path, stroke)
        .expect("a path that draws something has bounds")
}

/// Bounds hold the shape and its stroke, and a boolean never reaches beyond its inputs.
pub fn run(ops: &dyn PathOps) {
    assert_eq!(
        ops.bounds(&Path::default(), None),
        None,
        "PathOps: an empty path has bounds"
    );

    let square = Path::rect(10.0, 20.0, 30.0, 40.0);
    let fill = Rect::new(10.0, 20.0, 30.0, 40.0);
    assert!(
        same(&bounds(ops, &square, None), &fill),
        "PathOps: the bounds of a rectangle are not the rectangle"
    );
    assert!(
        same(
            &bounds(ops, &square, Some(&stroke(0.0, StrokeAlign::Center))),
            &fill
        ),
        "PathOps: a stroke of no width changed the bounds"
    );

    let centred = bounds(ops, &square, Some(&stroke(10.0, StrokeAlign::Center)));
    assert!(
        centred.contains_rect(&fill.expand(5.0), 1e-6),
        "PathOps: a centred stroke 10 wide must reach 5 beyond the edge, got {centred:?}"
    );
    let outside = bounds(ops, &square, Some(&stroke(10.0, StrokeAlign::Outside)));
    assert!(
        outside.contains_rect(&fill.expand(10.0), 1e-6),
        "PathOps: an outside stroke 10 wide must reach 10 beyond the edge, got {outside:?}"
    );
    let inside = bounds(ops, &square, Some(&stroke(10.0, StrokeAlign::Inside)));
    assert!(
        fill.contains_rect(&inside, 1e-6),
        "PathOps: an inside stroke on a closed shape must stay inside it, got {inside:?}"
    );
    let thin = bounds(ops, &square, Some(&stroke(4.0, StrokeAlign::Center)));
    let thick = bounds(ops, &square, Some(&stroke(8.0, StrokeAlign::Center)));
    assert!(
        thick.contains_rect(&thin, 1e-6),
        "PathOps: a wider stroke has smaller bounds"
    );
    assert!(
        thin.contains_rect(&fill, 1e-6),
        "PathOps: a stroke shrank the bounds"
    );

    let p = |x, y| Point { x, y };
    let arch = Path {
        segments: vec![
            PathSegment::MoveTo(p(0.0, 0.0)),
            PathSegment::CubicTo(p(0.0, 10.0), p(10.0, 10.0), p(10.0, 0.0)),
        ],
    };
    let hull = Rect::new(0.0, 0.0, 10.0, 10.0);
    let curve = bounds(ops, &arch, None);
    assert!(
        curve.contains_rect(&Rect::new(0.0, 0.0, 10.0, 0.0), 1e-6),
        "PathOps: the bounds of a curve miss its end points, got {curve:?}"
    );
    assert!(
        hull.contains_rect(&curve, 1e-6),
        "PathOps: the bounds of a curve reach beyond its control points"
    );

    let a = Path::rect(0.0, 0.0, 20.0, 20.0);
    let b = Path::rect(10.0, 10.0, 20.0, 20.0);
    let both = Rect::new(0.0, 0.0, 30.0, 30.0);
    for op in OPS {
        let result = ops.boolean(&a, &b, op);
        assert_eq!(
            result,
            ops.boolean(&a, &b, op),
            "PathOps: {op:?} gave two results"
        );
        if let Some(r) = ops.bounds(&result, None) {
            assert!(
                both.contains_rect(&r, 1e-6),
                "PathOps: {op:?} reached beyond its inputs, got {r:?}"
            );
        }
        let _ = ops.boolean(&Path::default(), &a, op);
        let _ = ops.boolean(&a, &Path::default(), op);
    }
    let same_shape = ops.boolean(&a, &a, BoolOp::Union);
    assert!(
        same(
            &bounds(ops, &same_shape, None),
            &Rect::new(0.0, 0.0, 20.0, 20.0)
        ),
        "PathOps: a shape united with itself changed size"
    );
}
