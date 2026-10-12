//! Every op returns its inverse: applying the plan's edits and then its inverse gives back the document
//! exactly, and the store's undo uses that inverse.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::common::*;
use crate::ops::{Op, apply, apply_edits};

fn one_of_each() -> Vec<(&'static str, Op)> {
    vec![
        (
            "insert",
            Op::Insert {
                to: in_node("frame00001", 1),
                node: Box::new(shape("rect", "rnew000001", json!({}))),
            },
        ),
        (
            "update",
            Op::Update {
                id: id("rect000001"),
                patch: patch(json!({"name": "X", "opacity": 0.5, "stroke": null})),
            },
        ),
        (
            "delete",
            Op::Delete {
                id: id("frame00001"),
            },
        ),
        (
            "move",
            Op::Move {
                id: id("rect000002"),
                to: on_page("canvas0001", 3),
            },
        ),
        (
            "copy",
            Op::Copy {
                id: id("group00001"),
                to: in_node("frame00001", 2),
            },
        ),
        (
            "replace",
            Op::Replace {
                id: id("group00001"),
                node: Box::new(shape(
                    "star",
                    "snew000001",
                    json!({"points": 5, "innerRatio": 0.5}),
                )),
            },
        ),
        (
            "set_variables",
            Op::SetVariables {
                set: BTreeMap::from([(
                    var_id("color00001"),
                    variable(
                        json!({"type": "color", "name": "x", "values": [{"value": "#000000"}]}),
                    ),
                )]),
                remove: BTreeSet::from([var_id("number0001")]),
            },
        ),
        (
            "reorder",
            Op::Reorder {
                id: id("text000001"),
                index: 0,
            },
        ),
        (
            "wrap_in_frame",
            Op::WrapInFrame {
                ids: vec![id("rect000001"), id("rect000002")],
                frame: Box::new(shape("frame", "fnew000001", at(1.0, 2.0))),
            },
        ),
        (
            "group",
            Op::Group {
                ids: vec![id("rect000003"), id("bool000001")],
                group: Box::new(shape("group", "gnew000001", at(3.0, 4.0))),
            },
        ),
        (
            "ungroup",
            Op::Ungroup {
                id: id("group00001"),
            },
        ),
    ]
}

#[test]
fn every_op_has_an_inverse_that_restores_the_document() {
    let original = document();
    for (name, op) in one_of_each() {
        let plan = planned(&original, vec![op]).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_ne!(plan.document, original, "{name}: changed nothing");
        let mut doc = original.clone();
        apply_edits(&mut doc, &plan.edits, alice()).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(doc, plan.document, "{name}: the edits are not the plan");
        apply_edits(&mut doc, &plan.inverse, alice()).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            doc, original,
            "{name}: the inverse did not restore the document"
        );
    }
}

#[test]
fn a_whole_batch_inverts_too() {
    let original = document();
    let ops = one_of_each()
        .into_iter()
        .filter(|(name, _)| !matches!(*name, "delete" | "replace" | "ungroup" | "wrap_in_frame"))
        .map(|(_, op)| op)
        .collect();
    let plan = planned(&original, ops).expect("accepted");
    let mut doc = plan.document.clone();
    apply_edits(&mut doc, &plan.inverse, alice()).expect("inverse applies");
    assert_eq!(doc, original);
}

#[test]
fn the_store_undoes_with_the_inverse() {
    let mut store = Store::new(document());
    for (name, op) in one_of_each() {
        apply(&mut store, &mut Ids::default(), tx(vec![op]))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        crate::ports::DocStore::undo(&mut store, &person(ALICE)).expect("undo");
        assert_eq!(store.doc, document(), "{name}: undo did not restore");
    }
}
