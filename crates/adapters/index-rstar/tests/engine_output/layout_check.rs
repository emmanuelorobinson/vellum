use super::common::*;
use serde_json::json;
use vellum_contracts::fakes::FixedMetricText;
use vellum_core::{
    layout::{Axis, LayoutProblem},
    layout_check::{self, Problem},
    ports::Rect,
};
use vellum_index_rstar::RstarIndex;
#[test]
fn all_requested_diagnostics_and_problem_only_projection() {
    let roots = resolve(vec![
        node(
            "f",
            "frame",
            json!({"layout":"row","clip":true,"children":[
                node("a","rect",json!({})), node("b","rect",json!({})),
                node("t","text",json!({"resize":"fixed","content":[{"text":"a\nb\nc","font":"TEST DATA Sans","size":20}]}))
            ]}),
        ),
        node("loose", "rect", json!({})),
        node("inside", "rect", json!({})),
        node("hidden", "rect", json!({"visible":false})),
    ]);
    let mut layout = boxes(&[
        ("f", [0., 0., 100., 100.]),
        ("f/a", [80., 0., 30., 30.]),
        ("f/b", [85., 10., 10., 10.]),
        ("f/t", [0., 0., 10., 10.]),
        ("loose", [500., 0., 20., 20.]),
        ("inside", [40., 40., 10., 10.]),
    ]);
    layout.problems.push(LayoutProblem::HugCycleBroken {
        parent: id("f"),
        child: id("f/a"),
        axis: Axis::Width,
    });
    let r =
        layout_check::check(&roots, &layout, &FixedMetricText::new(), RstarIndex::new()).unwrap();
    assert!(r.problems.contains(&Problem::TextOverflow {
        node: id("f/t"),
        horizontal: true,
        vertical: true
    }));
    assert!(r.problems.contains(&Problem::ClippedChild {
        node: id("f/a"),
        parent: id("f")
    }));
    assert!(r.problems.contains(&Problem::OverlappingSiblings {
        parent: id("f"),
        first: id("f/a"),
        second: id("f/b")
    }));
    assert!(
        r.problems
            .contains(&Problem::OutsideFrame { node: id("loose") })
    );
    assert!(r.problems.contains(&Problem::HugCycle {
        parent: id("f"),
        child: id("f/a"),
        axis: Axis::Width
    }));
    assert!(
        !r.problems
            .contains(&Problem::OutsideFrame { node: id("inside") })
    );
    assert!(!r.nodes.iter().any(|n| n.node == id("hidden")));
    let only = r.problems_only();
    assert!(!only.nodes.iter().any(|n| n.node == id("inside")));
    assert_eq!(only.problems, r.problems);
    assert_eq!(
        r.nodes.iter().find(|n| n.node == id("f/b")).unwrap().bounds,
        Rect::new(85., 10., 10., 10.)
    );
}
#[test]
fn touching_edges_and_absolute_siblings_are_not_auto_layout_overlap() {
    let roots = resolve(vec![node(
        "f",
        "frame",
        json!({"layout":"row","children":[node("a","rect",json!({})),node("b","rect",json!({})),node("c","rect",json!({"position":{"type":"absolute","x":0,"y":0}}))]}),
    )]);
    let l = boxes(&[
        ("f", [0., 0., 100., 100.]),
        ("f/a", [0., 0., 20., 20.]),
        ("f/b", [20., 0., 20., 20.]),
        ("f/c", [0., 0., 20., 20.]),
    ]);
    let r = layout_check::check(&roots, &l, &FixedMetricText::new(), RstarIndex::new()).unwrap();
    assert!(
        !r.problems
            .iter()
            .any(|p| matches!(p, Problem::OverlappingSiblings { .. }))
    );
}
#[test]
fn rounded_clips_and_rotated_ancestors_use_world_geometry() {
    let roots = resolve(vec![node(
        "f",
        "frame",
        json!({"clip":true,"cornerRadius":30,"rotation":90,"children":[node("g","group",json!({"children":[node("c","rect",json!({}))]}))]}),
    )]);
    let l = boxes(&[
        ("f", [100., 100., 100., 100.]),
        ("f/g", [0., 0., 100., 100.]),
        ("f/g/c", [0., 0., 2., 2.]),
    ]);
    let r = layout_check::check(&roots, &l, &FixedMetricText::new(), RstarIndex::new()).unwrap();
    assert!(r.problems.contains(&Problem::ClippedChild {
        node: id("f/g/c"),
        parent: id("f")
    }));
}
#[test]
fn missing_layout_is_an_error_not_an_empty_success() {
    let roots = resolve(vec![node("r", "rect", json!({}))]);
    assert!(
        layout_check::check(
            &roots,
            &boxes(&[]),
            &FixedMetricText::new(),
            RstarIndex::new()
        )
        .is_err()
    );
}
#[test]
fn identical_definition_ids_remain_distinct_occurrences() {
    let mut doc = vellum_types::Document::new("TEST DATA");
    doc.components.insert("component".parse().unwrap(),serde_json::from_value(json!({"name":"TEST DATA","root":node("root","frame",json!({"clip":true,"children":[node("shared","rect",json!({"fills":[{"type":"solid","color":"#123456"}]}))]}))})).unwrap());
    let roots = resolve_in(
        &doc,
        vec![
            node("one", "instance", json!({"componentId":"component"})),
            node("two", "instance", json!({"componentId":"component"})),
        ],
    );
    assert_eq!(
        roots[0].children[0].value.node.id(),
        roots[1].children[0].value.node.id()
    );
    let l = boxes(&[
        ("one", [0., 0., 100., 100.]),
        ("one/shared", [95., 0., 20., 20.]),
        ("two", [200., 0., 100., 100.]),
        ("two/shared", [0., 0., 20., 20.]),
    ]);
    let r = layout_check::check(&roots, &l, &FixedMetricText::new(), RstarIndex::new()).unwrap();
    assert!(r.problems.contains(&Problem::ClippedChild {
        node: id("one/shared"),
        parent: id("one")
    }));
    assert!(!r.problems.contains(&Problem::ClippedChild {
        node: id("two/shared"),
        parent: id("two")
    }));
}
