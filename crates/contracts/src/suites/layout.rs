//! `LayoutEngine`.
//!
//! The rules are ADR-002's, in the cases both Chrome and Vellum agree on: fixed, hug and fill along a row or a
//! column, gap, padding, justify, align, and text measured by the `TextEngine` it is given. Prompt 18 adds the
//! fixtures for text wrapping, grid tracks and the stroke-in-layout switch.

use vellum_core::ports::{
    Dimension, DirtySet, Insets, LayoutContent, LayoutEngine, LayoutNode, LayoutResult,
    LayoutStyle, LayoutTree, Rect, RichText, TextEngine,
};
use vellum_types::{Align, Justify, Layout};

use super::close;
use crate::samples::node_id;

const FIXED: fn(f64) -> Dimension = Dimension::Fixed;
const HUG: Dimension = Dimension::Hug { fallback: None };
const FILL: Dimension = Dimension::Fill { fallback: None };

fn style(mode: Layout, width: Dimension, height: Dimension) -> LayoutStyle {
    LayoutStyle {
        mode,
        width,
        height,
        x: 0.0,
        y: 0.0,
        gap: 0.0,
        padding: Insets::default(),
        justify: Justify::Start,
        align: Align::Start,
    }
}

fn frame(id: &str, style: LayoutStyle, children: Vec<LayoutNode>) -> LayoutNode {
    LayoutNode {
        id: node_id(id),
        style,
        content: LayoutContent::Frame,
        children,
    }
}

fn leaf(id: &str, width: Dimension, height: Dimension) -> LayoutNode {
    frame(id, style(Layout::None, width, height), vec![])
}

fn at(mut node: LayoutNode, x: f64, y: f64) -> LayoutNode {
    node.style.x = x;
    node.style.y = y;
    node
}

fn tree(root: LayoutNode) -> LayoutTree {
    LayoutTree { root }
}

fn expect(result: &LayoutResult, id: &str, x: f64, y: f64, w: f64, h: f64, what: &str) {
    let got = result
        .boxes
        .get(&node_id(id))
        .unwrap_or_else(|| panic!("LayoutEngine ({what}): {id} has no box"));
    let want = Rect::new(x, y, w, h);
    assert!(
        close(got.x, x) && close(got.y, y) && close(got.width, w) && close(got.height, h),
        "LayoutEngine ({what}): {id} should be {want:?} but is {got:?}"
    );
}

fn lay<E: LayoutEngine>(
    make: &impl Fn() -> E,
    t: &LayoutTree,
    text: &dyn TextEngine,
) -> LayoutResult {
    make().layout(t, &DirtySet::all(), text)
}

fn count(node: &LayoutNode) -> usize {
    1 + node.children.iter().map(count).sum::<usize>()
}

/// Boxes land where fixed, hug and fill, gap, padding, justify and align say, every node gets exactly one, and
/// the result does not depend on the engine's history.
pub fn run<E: LayoutEngine>(make: impl Fn() -> E, text: &dyn TextEngine) {
    // A frame without layout puts children at their own position.
    let t = tree(frame(
        "root",
        style(Layout::None, FIXED(200.0), FIXED(100.0)),
        vec![at(leaf("c", FIXED(30.0), FIXED(40.0)), 10.0, 20.0)],
    ));
    let r = lay(&make, &t, text);
    expect(&r, "root", 0.0, 0.0, 200.0, 100.0, "fixed root");
    expect(&r, "c", 10.0, 20.0, 30.0, 40.0, "child without layout");

    // A row places children one after another with the gap between, and a hugging row adds them up.
    let mut row = style(Layout::Row, HUG, HUG);
    row.gap = 10.0;
    let t = tree(frame(
        "root",
        row,
        vec![
            leaf("a", FIXED(30.0), FIXED(20.0)),
            leaf("b", FIXED(40.0), FIXED(25.0)),
            leaf("c", FIXED(50.0), FIXED(10.0)),
        ],
    ));
    let r = lay(&make, &t, text);
    expect(&r, "root", 0.0, 0.0, 140.0, 25.0, "hug row");
    expect(&r, "a", 0.0, 0.0, 30.0, 20.0, "row child 1");
    expect(&r, "b", 40.0, 0.0, 40.0, 25.0, "row child 2");
    expect(&r, "c", 90.0, 0.0, 50.0, 10.0, "row child 3");

    // A column does the same downward.
    let mut column = style(Layout::Column, HUG, HUG);
    column.gap = 4.0;
    let t = tree(frame(
        "root",
        column,
        vec![
            leaf("a", FIXED(30.0), FIXED(20.0)),
            leaf("b", FIXED(50.0), FIXED(25.0)),
        ],
    ));
    let r = lay(&make, &t, text);
    expect(&r, "root", 0.0, 0.0, 50.0, 49.0, "hug column");
    expect(&r, "a", 0.0, 0.0, 30.0, 20.0, "column child 1");
    expect(&r, "b", 0.0, 24.0, 50.0, 25.0, "column child 2");

    // Padding pushes children in and grows a hugging parent.
    let mut padded = style(Layout::Row, HUG, HUG);
    padded.padding = Insets {
        top: 5.0,
        right: 6.0,
        bottom: 7.0,
        left: 8.0,
    };
    let t = tree(frame(
        "root",
        padded,
        vec![leaf("a", FIXED(30.0), FIXED(20.0))],
    ));
    let r = lay(&make, &t, text);
    expect(&r, "root", 0.0, 0.0, 44.0, 32.0, "padding");
    expect(&r, "a", 8.0, 5.0, 30.0, 20.0, "padded child");

    // Fill children share what is left equally; fixed children keep their size.
    let t = tree(frame(
        "root",
        style(Layout::Row, FIXED(300.0), FIXED(50.0)),
        vec![
            leaf("a", FILL, FIXED(10.0)),
            leaf("b", FILL, FIXED(10.0)),
            leaf("c", FIXED(60.0), FIXED(10.0)),
        ],
    ));
    let r = lay(&make, &t, text);
    expect(&r, "a", 0.0, 0.0, 120.0, 10.0, "fill 1");
    expect(&r, "b", 120.0, 0.0, 120.0, 10.0, "fill 2");
    expect(&r, "c", 240.0, 0.0, 60.0, 10.0, "fixed beside fill");

    let mut gapped = style(Layout::Row, FIXED(300.0), FIXED(50.0));
    gapped.gap = 10.0;
    let t = tree(frame(
        "root",
        gapped,
        vec![leaf("a", FILL, FIXED(10.0)), leaf("b", FILL, FIXED(10.0))],
    ));
    let r = lay(&make, &t, text);
    expect(&r, "a", 0.0, 0.0, 145.0, 10.0, "fill with gap 1");
    expect(&r, "b", 155.0, 0.0, 145.0, 10.0, "fill with gap 2");

    // Fixed children never shrink, and fill gets nothing when there is no room.
    let t = tree(frame(
        "root",
        style(Layout::Row, FIXED(100.0), FIXED(50.0)),
        vec![
            leaf("a", FIXED(80.0), FIXED(10.0)),
            leaf("b", FIXED(80.0), FIXED(10.0)),
            leaf("c", FILL, FIXED(10.0)),
        ],
    ));
    let r = lay(&make, &t, text);
    expect(&r, "a", 0.0, 0.0, 80.0, 10.0, "overflow 1");
    expect(&r, "b", 80.0, 0.0, 80.0, 10.0, "overflow 2");
    expect(&r, "c", 160.0, 0.0, 0.0, 10.0, "fill with no room");

    // Fill across the axis takes the parent's inner size.
    let mut inner = style(Layout::Row, FIXED(300.0), FIXED(100.0));
    inner.padding = Insets {
        top: 10.0,
        right: 0.0,
        bottom: 20.0,
        left: 0.0,
    };
    let t = tree(frame("root", inner, vec![leaf("a", FIXED(50.0), FILL)]));
    let r = lay(&make, &t, text);
    expect(&r, "a", 0.0, 10.0, 50.0, 70.0, "fill across");

    // Fill on the main axis of a hugging parent uses its fallback (ADR-002 row 5).
    let t = tree(frame(
        "root",
        style(Layout::Row, HUG, HUG),
        vec![leaf(
            "a",
            Dimension::Fill {
                fallback: Some(40.0),
            },
            FIXED(10.0),
        )],
    ));
    let r = lay(&make, &t, text);
    expect(&r, "a", 0.0, 0.0, 40.0, 10.0, "fill in a hugging parent");
    expect(&r, "root", 0.0, 0.0, 40.0, 10.0, "parent of that fill");

    // Justify places the group along the row; align places each child across it.
    for (justify, xs) in [
        (Justify::Start, [0.0, 50.0]),
        (Justify::Center, [100.0, 150.0]),
        (Justify::End, [200.0, 250.0]),
        (Justify::SpaceBetween, [0.0, 250.0]),
    ] {
        let mut s = style(Layout::Row, FIXED(300.0), FIXED(100.0));
        s.justify = justify;
        let t = tree(frame(
            "root",
            s,
            vec![
                leaf("a", FIXED(50.0), FIXED(20.0)),
                leaf("b", FIXED(50.0), FIXED(20.0)),
            ],
        ));
        let r = lay(&make, &t, text);
        expect(
            &r,
            "a",
            xs[0],
            0.0,
            50.0,
            20.0,
            &format!("justify {justify:?} 1"),
        );
        expect(
            &r,
            "b",
            xs[1],
            0.0,
            50.0,
            20.0,
            &format!("justify {justify:?} 2"),
        );
    }
    for (align, y) in [
        (Align::Start, 0.0),
        (Align::Center, 40.0),
        (Align::End, 80.0),
    ] {
        let mut s = style(Layout::Row, FIXED(300.0), FIXED(100.0));
        s.align = align;
        let t = tree(frame("root", s, vec![leaf("a", FIXED(50.0), FIXED(20.0))]));
        let r = lay(&make, &t, text);
        expect(&r, "a", 0.0, y, 50.0, 20.0, &format!("align {align:?}"));
    }

    // Text is measured by the text engine it is handed.
    let rich = RichText::plain("hello brave new world", "TEST DATA Sans", 10.0);
    let unbounded = text.measure(&rich, vellum_core::ports::Extent::UNBOUNDED);
    let hugging_text = LayoutNode {
        id: node_id("t"),
        style: style(Layout::None, HUG, HUG),
        content: LayoutContent::Text(rich.clone()),
        children: vec![],
    };
    let t = tree(frame(
        "root",
        style(Layout::Column, HUG, HUG),
        vec![hugging_text],
    ));
    let r = lay(&make, &t, text);
    expect(
        &r,
        "t",
        0.0,
        0.0,
        unbounded.width,
        unbounded.height,
        "hugging text",
    );
    expect(
        &r,
        "root",
        0.0,
        0.0,
        unbounded.width,
        unbounded.height,
        "parent of hugging text",
    );

    let wrapped = text.measure(&rich, vellum_core::ports::Extent::with_width(40.0));
    let narrow = LayoutNode {
        id: node_id("t"),
        style: style(Layout::None, FIXED(40.0), HUG),
        content: LayoutContent::Text(rich),
        children: vec![],
    };
    let t = tree(frame("root", style(Layout::Column, HUG, HUG), vec![narrow]));
    let r = lay(&make, &t, text);
    expect(
        &r,
        "t",
        0.0,
        0.0,
        40.0,
        wrapped.height,
        "text wrapped in a fixed width",
    );

    // One box per node, nothing else, the same every time.
    let mut nested_style = style(Layout::Column, FIXED(200.0), HUG);
    nested_style.gap = 3.0;
    let nested = tree(frame(
        "root",
        nested_style,
        vec![
            frame(
                "row",
                style(Layout::Row, FILL, HUG),
                vec![
                    leaf("a", FILL, FIXED(10.0)),
                    leaf("b", FIXED(20.0), FIXED(30.0)),
                ],
            ),
            leaf("c", FIXED(15.0), FIXED(15.0)),
        ],
    ));
    let first = lay(&make, &nested, text);
    assert_eq!(
        first.boxes.len(),
        count(&nested.root),
        "LayoutEngine: not one box per node"
    );
    assert_eq!(
        first,
        lay(&make, &nested, text),
        "LayoutEngine: a fresh engine gave a different layout"
    );
    let mut engine = make();
    let a = engine.layout(&nested, &DirtySet::all(), text);
    let b = engine.layout(&nested, &DirtySet::all(), text);
    assert_eq!(
        a, b,
        "LayoutEngine: laying out twice on one engine gave different layouts"
    );
    assert_eq!(a, first);
    expect(&first, "row", 0.0, 0.0, 200.0, 30.0, "nested row");
    expect(&first, "a", 0.0, 0.0, 180.0, 10.0, "nested fill");
    expect(&first, "c", 0.0, 33.0, 15.0, 15.0, "after the nested row");

    // Laying out only what changed gives what laying out everything gives.
    let mut engine = make();
    engine.layout(&nested, &DirtySet::all(), text);
    let mut changed = nested.clone();
    changed.root.children[0].children[1].style.width = FIXED(60.0);
    changed.root.children[1].style.height = FIXED(5.0);
    let incremental = engine.layout(&changed, &DirtySet::of([node_id("b"), node_id("c")]), text);
    assert_eq!(
        incremental,
        lay(&make, &changed, text),
        "LayoutEngine: laying out only the changed nodes left a stale box"
    );
    expect(
        &incremental,
        "a",
        0.0,
        0.0,
        140.0,
        10.0,
        "fill after its sibling grew",
    );
}
