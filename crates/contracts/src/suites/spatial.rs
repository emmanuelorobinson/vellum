//! `SpatialIndex`.

use vellum_core::ports::{Rect, SpatialIndex};
use vellum_types::NodeId;

use crate::samples::node_id;

fn sorted(mut ids: Vec<NodeId>) -> Vec<String> {
    ids.sort();
    ids.into_iter().map(|id| id.to_string()).collect()
}

fn rect(index: &dyn SpatialIndex, r: Rect) -> Vec<String> {
    sorted(index.query_rect(r))
}

fn point(index: &dyn SpatialIndex, x: f64, y: f64) -> Vec<String> {
    sorted(index.query_point(x, y))
}

/// Queries find what they should, edges count, updates move and remove, and rebuilding starts over.
pub fn run<S: SpatialIndex>(make: impl Fn() -> S) {
    let a = (node_id("a"), Rect::new(0.0, 0.0, 10.0, 10.0));
    let b = (node_id("b"), Rect::new(5.0, 5.0, 10.0, 10.0));
    let c = (node_id("c"), Rect::new(20.0, 20.0, 5.0, 5.0));
    let line = (node_id("line"), Rect::new(30.0, 30.0, 10.0, 0.0));

    let empty = make();
    assert!(
        rect(&empty, Rect::new(-1e6, -1e6, 2e6, 2e6)).is_empty()
            && point(&empty, 0.0, 0.0).is_empty(),
        "SpatialIndex: an empty index found something"
    );

    let mut index = make();
    index.rebuild(&[a.clone(), b.clone(), c.clone(), line.clone()]);

    assert_eq!(rect(&index, Rect::new(0.0, 0.0, 1.0, 1.0)), ["a"]);
    assert_eq!(rect(&index, Rect::new(6.0, 6.0, 1.0, 1.0)), ["a", "b"]);
    assert_eq!(
        rect(&index, Rect::new(100.0, 100.0, 1.0, 1.0)),
        Vec::<String>::new()
    );
    assert_eq!(
        rect(&index, Rect::new(-1e6, -1e6, 2e6, 2e6)),
        ["a", "b", "c", "line"],
        "SpatialIndex: a rectangle over everything must find everything"
    );
    assert_eq!(
        rect(&index, Rect::new(10.0, 0.0, 5.0, 5.0)),
        ["a", "b"],
        "SpatialIndex: rectangles that only touch must intersect"
    );

    assert_eq!(point(&index, 2.0, 2.0), ["a"]);
    assert_eq!(point(&index, 7.0, 7.0), ["a", "b"]);
    assert_eq!(
        point(&index, 10.0, 10.0),
        ["a", "b"],
        "SpatialIndex: a point on a corner is inside"
    );
    assert_eq!(point(&index, 15.0, 15.0), ["b"]);
    assert_eq!(
        point(&index, 35.0, 30.0),
        ["line"],
        "SpatialIndex: a rectangle with no height must still be found"
    );
    assert_eq!(point(&index, 35.0, 31.0), Vec::<String>::new());
    assert_eq!(point(&index, -1.0, -1.0), Vec::<String>::new());

    index.update(
        &[
            (node_id("a"), Rect::new(100.0, 100.0, 10.0, 10.0)),
            (node_id("d"), Rect::new(0.0, 0.0, 3.0, 3.0)),
        ],
        &[node_id("b"), node_id("never-there")],
    );
    assert_eq!(
        point(&index, 2.0, 2.0),
        ["d"],
        "SpatialIndex: a moved node still answers at its old place"
    );
    assert_eq!(point(&index, 105.0, 105.0), ["a"]);
    assert_eq!(
        point(&index, 15.0, 15.0),
        Vec::<String>::new(),
        "SpatialIndex: a removed node still answers"
    );
    assert_eq!(
        rect(&index, Rect::new(-1e6, -1e6, 2e6, 2e6)),
        ["a", "c", "d", "line"]
    );

    index.rebuild(&[c]);
    assert_eq!(
        rect(&index, Rect::new(-1e6, -1e6, 2e6, 2e6)),
        ["c"],
        "SpatialIndex: rebuild must forget everything before it"
    );
}
