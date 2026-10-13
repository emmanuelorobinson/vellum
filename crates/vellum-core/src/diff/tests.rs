//! `diff(old, new)`: ops by node id and property that turn `old` into `new` when planned. The property test over
//! random documents is in `tests/diff_merge.rs`; these are the cases by name.

use std::collections::BTreeMap;

use serde_json::json;
use vellum_types::{ComponentId, Document, PageKind};

use super::diff;
use crate::ops::tests::common::*;
use crate::ops::{Op, plan};

/// Diffs, plans the ops on `old` and checks they lead to `new` exactly. Returns the ops.
fn reaches(old: &Document, new: &Document) -> Vec<Op> {
    let ops = diff(old, new).unwrap_or_else(|e| panic!("diff failed: {e} at {}", e.location()));
    if ops.is_empty() {
        assert_eq!(old, new, "no ops, but the documents differ");
        return ops;
    }
    let planned = plan(old, &tx(ops.clone()), &mut Ids::default())
        .unwrap_or_else(|e| panic!("the diff was refused: {e} at {}\n{ops:#?}", e.location()));
    assert_eq!(
        &planned.document, new,
        "the diff does not lead to the new document\n{ops:#?}"
    );
    ops
}

/// `doc` with `ops` applied.
fn edited(ops: Vec<Op>) -> Document {
    accepted(ops)
}

fn comp(text: &str) -> ComponentId {
    ComponentId::new(text).expect("a valid id")
}

#[test]
fn the_same_document_gives_no_ops() {
    assert!(diff(&document(), &document()).expect("diff").is_empty());
}

#[test]
fn a_property_change_is_one_update_with_only_that_property() {
    let new = edited(vec![Op::Update {
        id: id("rect000001"),
        patch: patch(json!({"opacity": 0.5, "name": "TEST DATA A2"})),
    }]);
    let ops = reaches(&document(), &new);
    assert_eq!(
        ops,
        [Op::Update {
            id: id("rect000001"),
            patch: patch(json!({"name": "TEST DATA A2", "opacity": 0.5})),
        }]
    );
}

#[test]
fn a_property_back_at_its_default_is_null_in_the_patch() {
    let new = edited(vec![Op::Update {
        id: id("text000001"),
        patch: patch(json!({"fills": null})),
    }]);
    let ops = reaches(&document(), &new);
    assert_eq!(
        ops,
        [Op::Update {
            id: id("text000001"),
            patch: patch(json!({"fills": null})),
        }]
    );
}

#[test]
fn a_move_is_a_move_not_a_delete_and_insert() {
    // Into another parent, on another page.
    let new = edited(vec![Op::Move {
        id: id("rect000001"),
        to: in_node("slide00001", 0),
    }]);
    let ops = reaches(&document(), &new);
    assert_eq!(
        ops,
        [Op::Move {
            id: id("rect000001"),
            to: in_node("slide00001", 0),
        }]
    );

    // Among its siblings.
    let new = edited(vec![Op::Reorder {
        id: id("text000001"),
        index: 0,
    }]);
    let ops = reaches(&document(), &new);
    assert_eq!(
        ops,
        [Op::Reorder {
            id: id("text000001"),
            index: 0,
        }]
    );
}

#[test]
fn a_moved_and_changed_node_keeps_its_id() {
    let new = edited(vec![
        Op::Move {
            id: id("frame00001"),
            to: in_node("group00001", 0),
        },
        Op::Update {
            id: id("frame00001"),
            patch: patch(json!({"name": "TEST DATA moved"})),
        },
    ]);
    let ops = reaches(&document(), &new);
    assert!(
        ops.iter()
            .all(|op| !matches!(op, Op::Delete { .. } | Op::Insert { .. })),
        "{ops:#?}"
    );
}

#[test]
fn inserts_and_deletes_are_whole_subtrees() {
    let new = edited(vec![
        Op::Delete {
            id: id("frame00001"),
        },
        Op::Insert {
            to: on_page("canvas0001", 1),
            node: Box::new(node(
                json!({"type": "frame", "id": "fnew000001", "name": "TEST DATA F",
                "width": 10, "height": 10, "children": [
                    {"type": "rect", "id": "rnew000001", "name": "R", "width": 1, "height": 1}
                ]}),
            )),
        },
    ]);
    let ops = reaches(&document(), &new);
    assert_eq!(ops.len(), 2, "{ops:#?}");
    assert!(matches!(&ops[0], Op::Insert { node, .. } if node.children().len() == 1));
    assert!(matches!(&ops[1], Op::Delete { id: d } if d == &id("frame00001")));
}

#[test]
fn a_node_that_changes_kind_under_the_same_id_is_replaced() {
    let new = edited(vec![Op::Replace {
        id: id("group00001"),
        node: Box::new(shape("rect", "group00001", json!({}))),
    }]);
    reaches(&document(), &new);
}

#[test]
fn children_move_out_of_a_deleted_parent_and_into_a_new_one() {
    // Ungroup and wrap: the children survive both, and keep their ids.
    let new = edited(vec![
        Op::Ungroup {
            id: id("group00001"),
        },
        Op::WrapInFrame {
            ids: vec![id("rect000001"), id("rect000002")],
            frame: Box::new(shape("frame", "fnew000001", at(0.0, 0.0))),
        },
    ]);
    let ops = reaches(&document(), &new);
    for kept in ["ellipse001", "rect000001", "rect000002"] {
        assert!(
            !ops.iter()
                .any(|op| matches!(op, Op::Delete { id: d } if d == &id(kept))),
            "{kept} was deleted\n{ops:#?}"
        );
    }
}

#[test]
fn a_parent_and_child_can_swap() {
    let new = edited(vec![
        Op::Move {
            id: id("rect000001"),
            to: on_page("canvas0001", 0),
        },
        Op::Replace {
            id: id("rect000001"),
            node: Box::new(shape("frame", "rect000001", json!({}))),
        },
    ]);
    reaches(&document(), &new);
    // frame00001 inside the group, the group inside a new frame.
    let new = edited(vec![
        Op::Move {
            id: id("frame00001"),
            to: in_node("group00001", 0),
        },
        Op::WrapInFrame {
            ids: vec![id("group00001")],
            frame: Box::new(shape("frame", "fnew000001", at(0.0, 0.0))),
        },
    ]);
    reaches(&document(), &new);
    reaches(&new, &document());
}

#[test]
fn variables_are_added_changed_and_removed() {
    let new = edited(vec![
        Op::SetVariables {
            set: BTreeMap::from([
                (
                    var_id("color00002"),
                    variable(
                        json!({"type": "color", "name": "color/new", "values": [{"value": "#000000"}]}),
                    ),
                ),
                (
                    var_id("number0001"),
                    variable(
                        json!({"type": "number", "name": "space/lg", "values": [{"value": 24}]}),
                    ),
                ),
            ]),
            remove: Default::default(),
        },
        Op::Update {
            id: id("text000001"),
            patch: patch(json!({"fills": [{"type": "solid", "color": {"$var": "color00002"}}]})),
        },
        Op::SetVariables {
            set: BTreeMap::new(),
            remove: [var_id("color00001")].into(),
        },
    ]);
    // The new variable must exist before the node uses it, and the old one go after the node stops using it.
    reaches(&document(), &new);
    reaches(&new, &document());
}

#[test]
fn pages_are_added_removed_renamed_and_reordered() {
    let new = edited(vec![
        Op::InsertPage {
            index: 0,
            page: Box::new(
                serde_json::from_value(json!({"id": "canvas0002", "name": "TEST DATA P"}))
                    .expect("page"),
            ),
        },
        Op::Move {
            id: id("rect000003"),
            to: on_page("canvas0002", 0),
        },
        Op::DeletePage {
            id: page_id("canvas0001"),
        },
        Op::UpdatePage {
            id: page_id("slides0001"),
            name: "TEST DATA now a canvas".into(),
            kind: PageKind::Canvas,
        },
        Op::Insert {
            to: on_page("slides0001", 1),
            node: Box::new(shape("rect", "rnew000001", json!({}))),
        },
    ]);
    let ops = reaches(&document(), &new);
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::Move { id: m, .. } if m == &id("rect000003"))),
        "a node on a removed page that lives on is moved\n{ops:#?}"
    );
    reaches(&new, &document());
}

#[test]
fn a_page_turning_into_slides_changes_kind_after_its_nodes_leave() {
    let new = edited(vec![
        Op::Delete {
            id: id("rect000003"),
        },
        Op::Delete {
            id: id("group00001"),
        },
        Op::Delete {
            id: id("bool000001"),
        },
        Op::Delete {
            id: id("text000001"),
        },
        Op::UpdatePage {
            id: page_id("canvas0001"),
            name: "Canvas".into(),
            kind: PageKind::Slides,
        },
    ]);
    reaches(&document(), &new);
    reaches(&new, &document());
}

#[test]
fn the_shell_and_components_are_diffed() {
    let mut new = document();
    new.meta.name = "TEST DATA renamed".into();
    new.themes
        .insert("mode".into(), vec!["light".into(), "dark".into()]);
    let button = new
        .components
        .get_mut(&comp("comp000001"))
        .expect("component");
    button.name = "TEST DATA Primary".into();
    button.root = node(
        json!({"type": "frame", "id": "croot00001", "name": "Button", "width": "hug",
        "height": "hug", "children": [
            {"type": "rect", "id": "cbg0000001", "name": "Bg", "width": 1, "height": 1},
            {"type": "text", "id": "ctext00001", "name": "Label 2", "width": "hug", "height": "hug",
             "content": [{"text": "OK", "font": "Inter", "size": 14}]}
        ]}),
    );
    let ops = reaches(&document(), &new);
    assert!(ops.iter().any(|op| matches!(op, Op::SetShell { .. })));
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::UpdateComponent { .. }))
    );
    assert!(
        !ops.iter().any(|op| matches!(op, Op::SetComponent { .. })),
        "the tree is diffed node by node when its root stays\n{ops:#?}"
    );
    reaches(&new, &document());
}

#[test]
fn a_component_with_a_new_root_is_replaced_and_nodes_cross_between_page_and_component() {
    let mut new = document();
    // The label leaves the component for the page; a page rect becomes the component's root.
    let label = crate::ops::find(&new, &id("ctext00001"))
        .expect("label")
        .clone();
    new.pages[0].children.push(label);
    new.pages[0]
        .children
        .retain(|n| n.id() != &id("rect000003"));
    new.components
        .get_mut(&comp("comp000001"))
        .expect("component")
        .root = node(
        json!({"type": "frame", "id": "rect000003", "name": "Root", "width": 10, "height": 10}),
    );
    new.components.insert(
        comp("comp000002"),
        serde_json::from_value(json!({"name": "TEST DATA New", "root": {
            "type": "frame", "id": "croot00002", "name": "R", "width": 1, "height": 1}}))
        .expect("component"),
    );
    reaches(&document(), &new);
    reaches(&new, &document());
}

#[test]
fn instances_reach_components_added_in_the_same_change() {
    let mut new = document();
    new.components.insert(
        comp("comp000002"),
        serde_json::from_value(json!({"name": "TEST DATA New", "root": {
            "type": "frame", "id": "croot00002", "name": "R", "width": 1, "height": 1}}))
        .expect("component"),
    );
    new.pages[0]
        .children
        .push(node(json!({"type": "instance", "id": "inst000001",
        "name": "TEST DATA i", "width": "hug", "height": "hug", "componentId": "comp000002"})));
    reaches(&document(), &new);
    reaches(&new, &document());
}

#[test]
fn every_op_kind_in_one_change_still_reaches_the_new_document() {
    let new = edited(vec![
        Op::Copy {
            id: id("frame00001"),
            to: on_page("canvas0001", 0),
        },
        Op::Group {
            ids: vec![id("rect000003"), id("text000001")],
            group: Box::new(shape("group", "gnew000001", at(5.0, 5.0))),
        },
        Op::Flip {
            id: id("ellipse001"),
            axis: crate::ops::Axis::Horizontal,
        },
        Op::Move {
            id: id("slide00001"),
            to: on_page("canvas0001", 2),
        },
        Op::CreateComponent {
            id: id("group00001"),
            component_id: comp("comp000002"),
        },
    ]);
    reaches(&document(), &new);
    reaches(&new, &document());
}

/// A file that does not hold together is not a diff the op layer accepts; the error says why.
#[test]
fn an_invalid_new_document_is_refused() {
    let mut new = document();
    new.pages[0]
        .children
        .push(node(json!({"type": "instance", "id": "inst000001",
        "name": "TEST DATA i", "width": "hug", "height": "hug", "componentId": "nocomp0001"})));
    let ops = diff(&document(), &new).expect("ops can still be made");
    let error = plan(&document(), &tx(ops), &mut Ids::default()).expect_err("refused");
    assert!(error.to_string().contains("nocomp0001"), "{error}");
}

#[test]
fn a_new_document_with_an_id_twice_is_an_error_not_a_panic() {
    let mut new = document();
    let copy = crate::ops::find(&new, &id("rect000001"))
        .expect("a")
        .clone();
    new.pages[0].children.push(copy);
    let error = diff(&document(), &new).expect_err("refused");
    assert_eq!(
        error.kind,
        crate::ops::OpErrorKind::DuplicateId(id("rect000001"))
    );
}
