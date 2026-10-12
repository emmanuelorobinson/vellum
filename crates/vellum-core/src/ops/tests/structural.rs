//! Each op's valid case: what the document looks like afterwards and what the change set says.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use vellum_types::NodeKind;

use super::common::*;
use crate::ops::{Op, apply};

fn names(list: &[vellum_types::NodeId]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

#[test]
fn insert_places_a_subtree_and_lists_every_new_node() {
    let group = node(
        json!({"type": "group", "id": "gnew000001", "name": "New", "width": "hug",
        "height": "hug", "children": [{"type": "rect", "id": "rnew000001", "name": "R", "width": 1, "height": 1}]}),
    );
    let plan = planned(
        &document(),
        vec![Op::Insert {
            to: in_node("frame00001", 1),
            node: Box::new(group),
        }],
    )
    .expect("accepted");
    assert_eq!(
        child_ids(&plan.document, "frame00001"),
        ["rect000001", "gnew000001", "rect000002"]
    );
    assert_eq!(names(&plan.changes.inserted), ["gnew000001", "rnew000001"]);
    assert!(plan.changes.updated.is_empty() && plan.changes.removed.is_empty());

    let doc = accepted(vec![Op::Insert {
        to: on_page("canvas0001", 0),
        node: Box::new(shape("rect", "rnew000002", json!({}))),
    }]);
    assert_eq!(child_ids(&doc, "canvas0001")[0], "rnew000002");
}

#[test]
fn update_sets_properties_and_keeps_children() {
    let plan = planned(
        &document(),
        vec![Op::Update {
            id: id("frame00001"),
            patch: patch(json!({"name": "Hero", "locked": true})),
        }],
    )
    .expect("accepted");
    let frame = find(&plan.document, "frame00001").expect("frame");
    assert_eq!(frame.name(), "Hero");
    assert!(frame.locked());
    assert_eq!(frame.children().len(), 2);
    assert_eq!(names(&plan.changes.updated), ["frame00001"]);
}

#[test]
fn update_reaches_nodes_inside_a_component() {
    let doc = accepted(vec![Op::Update {
        id: id("ctext00001"),
        patch: patch(json!({"name": "Caption"})),
    }]);
    assert_eq!(find(&doc, "ctext00001").expect("text").name(), "Caption");
}

#[test]
fn delete_removes_a_subtree() {
    let plan = planned(
        &document(),
        vec![Op::Delete {
            id: id("frame00001"),
        }],
    )
    .expect("accepted");
    assert!(find(&plan.document, "rect000001").is_none());
    assert_eq!(
        names(&plan.changes.removed),
        ["frame00001", "rect000001", "rect000002"]
    );
}

#[test]
fn move_reparents_and_counts_the_final_index() {
    let doc = accepted(vec![Op::Move {
        id: id("rect000003"),
        to: in_node("frame00001", 0),
    }]);
    assert_eq!(
        child_ids(&doc, "frame00001"),
        ["rect000003", "rect000001", "rect000002"]
    );
    assert!(!child_ids(&doc, "canvas0001").contains(&"rect000003".to_string()));

    let doc = accepted(vec![Op::Move {
        id: id("rect000001"),
        to: in_node("frame00001", 1),
    }]);
    assert_eq!(
        child_ids(&doc, "frame00001"),
        ["rect000002", "rect000001"],
        "within one parent the index is where the node ends up"
    );

    let doc = accepted(vec![Op::Move {
        id: id("frame00001"),
        to: on_page("slides0001", 1),
    }]);
    assert_eq!(
        child_ids(&doc, "slides0001"),
        ["slide00001", "frame00001"],
        "a node can move to another page"
    );
}

#[test]
fn reorder_moves_within_the_parent() {
    let plan = planned(
        &document(),
        vec![Op::Reorder {
            id: id("frame00001"),
            index: 2,
        }],
    )
    .expect("accepted");
    assert_eq!(
        child_ids(&plan.document, "canvas0001"),
        [
            "rect000003",
            "group00001",
            "frame00001",
            "bool000001",
            "text000001"
        ]
    );
    assert_eq!(names(&plan.changes.updated), ["frame00001"]);
}

#[test]
fn copy_gives_every_node_a_fresh_id_and_reports_the_new_root() {
    let plan = planned(
        &document(),
        vec![
            Op::Copy {
                id: id("frame00001"),
                to: on_page("canvas0001", 1),
            },
            Op::Copy {
                id: id("ctext00001"),
                to: in_node("frame00001", 0),
            },
        ],
    )
    .expect("accepted");
    let copy = &plan.document.pages[0].children[1];
    assert_eq!(copy.id().as_str(), "new0000001");
    let kids: Vec<_> = copy.children().iter().map(|n| n.id().to_string()).collect();
    assert_eq!(kids, ["new0000002", "new0000003"]);
    assert_eq!(copy.name(), "Card");
    assert_eq!(
        plan.created,
        BTreeMap::from([(0, id("new0000001")), (1, id("new0000004"))])
    );
    assert_eq!(
        find(&plan.document, "new0000004").expect("copied").kind(),
        NodeKind::Text,
        "a node can be copied out of a component"
    );
}

#[test]
fn replace_swaps_a_node_in_place() {
    let doc = accepted(vec![Op::Replace {
        id: id("rect000003"),
        node: Box::new(shape("ellipse", "rect000003", json!({}))),
    }]);
    assert_eq!(child_ids(&doc, "canvas0001")[1], "rect000003");
    assert_eq!(
        find(&doc, "rect000003").expect("node").kind(),
        NodeKind::Ellipse,
        "the replacement may keep the old id"
    );

    let doc = accepted(vec![Op::Replace {
        id: id("frame00001"),
        node: Box::new(shape("rect", "rnew000001", json!({}))),
    }]);
    assert_eq!(child_ids(&doc, "canvas0001")[0], "rnew000001");
    assert!(
        find(&doc, "rect000001").is_none(),
        "the old subtree is gone"
    );
}

#[test]
fn set_variables_adds_changes_and_removes() {
    let plan = planned(
        &document(),
        vec![Op::SetVariables {
            set: BTreeMap::from([
                (
                    var_id("color00002"),
                    variable(json!({"type": "color", "name": "color/alias", "values": [{"$alias": "color00001"}]})),
                ),
                (
                    var_id("number0001"),
                    variable(json!({"type": "number", "name": "space/lg", "values": [{"value": 24}]})),
                ),
            ]),
            remove: BTreeSet::new(),
        }],
    )
    .expect("accepted");
    assert_eq!(plan.document.variables.len(), 3);
    assert_eq!(
        plan.changes.variables,
        [var_id("color00002"), var_id("number0001")]
    );

    let doc = accepted(vec![Op::SetVariables {
        set: BTreeMap::new(),
        remove: BTreeSet::from([var_id("number0001")]),
    }]);
    assert!(!doc.variables.contains_key(&var_id("number0001")));
}

#[test]
fn wrap_in_frame_keeps_order_and_rebases_positions() {
    let frame = shape("frame", "wrap000001", at(5.0, 5.0));
    let doc = accepted(vec![Op::WrapInFrame {
        ids: vec![id("rect000002"), id("rect000001")],
        frame: Box::new(frame),
    }]);
    assert_eq!(child_ids(&doc, "frame00001"), ["wrap000001"]);
    assert_eq!(
        child_ids(&doc, "wrap000001"),
        ["rect000001", "rect000002"],
        "the wrapped nodes keep their stacking order"
    );
    assert_eq!(
        position(&doc, "rect000001"),
        json!({"type": "absolute", "x": 5.0, "y": 5.0})
    );
    assert_eq!(
        position(&doc, "rect000002"),
        json!({"type": "absolute", "x": 25.0, "y": 25.0})
    );
}

#[test]
fn wrap_goes_where_the_frontmost_wrapped_node_was() {
    let doc = accepted(vec![Op::Group {
        ids: vec![id("rect000003"), id("frame00001")],
        group: Box::new(shape("group", "gnew000001", json!({}))),
    }]);
    assert_eq!(
        child_ids(&doc, "canvas0001"),
        ["gnew000001", "group00001", "bool000001", "text000001"]
    );
    assert_eq!(child_ids(&doc, "gnew000001"), ["frame00001", "rect000003"]);
    assert_eq!(
        position(&doc, "frame00001"),
        json!({"type": "absolute", "x": 100.0, "y": 50.0}),
        "a wrapper at auto leaves positions alone"
    );
}

#[test]
fn ungroup_puts_children_where_the_group_was() {
    let plan = planned(
        &document(),
        vec![Op::Ungroup {
            id: id("group00001"),
        }],
    )
    .expect("accepted");
    assert_eq!(
        child_ids(&plan.document, "canvas0001"),
        [
            "frame00001",
            "rect000003",
            "ellipse001",
            "bool000001",
            "text000001"
        ]
    );
    assert_eq!(
        position(&plan.document, "ellipse001"),
        json!({"type": "absolute", "x": 205.0, "y": 205.0})
    );
    assert_eq!(names(&plan.changes.removed), ["group00001"]);
}

#[test]
fn later_ops_see_earlier_ones() {
    let doc = accepted(vec![
        Op::Insert {
            to: on_page("canvas0001", 0),
            node: Box::new(shape("frame", "fnew000001", json!({}))),
        },
        Op::Move {
            id: id("rect000003"),
            to: in_node("fnew000001", 0),
        },
        Op::Update {
            id: id("rect000003"),
            patch: patch(json!({"name": "Moved"})),
        },
    ]);
    assert_eq!(child_ids(&doc, "fnew000001"), ["rect000003"]);
    assert_eq!(find(&doc, "rect000003").expect("node").name(), "Moved");
}

#[test]
fn apply_hands_the_store_one_commit_carrying_the_origin() {
    let mut store = Store::new(document());
    let origin = alice().in_session(vellum_types::SessionId::new(BOB).expect("ulid"));
    let applied = apply(
        &mut store,
        &mut Ids::default(),
        crate::ops::Transaction::new(
            origin.clone(),
            vec![
                Op::Delete {
                    id: id("rect000003"),
                },
                Op::Copy {
                    id: id("rect000001"),
                    to: on_page("canvas0001", 0),
                },
            ],
        ),
    )
    .expect("accepted");
    assert_eq!(store.applied, 1, "one transaction is one commit");
    assert_eq!(applied.changes.origin, origin, "Invariant 2");
    assert_eq!(applied.created, BTreeMap::from([(1, id("new0000001"))]));
    assert!(find(&store.doc, "rect000003").is_none());
    assert!(find(&store.doc, "new0000001").is_some());
}
