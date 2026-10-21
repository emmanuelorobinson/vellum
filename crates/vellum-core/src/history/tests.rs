//! History on its own. The contract suite (`vellum-contracts`, `suites::doc_store`) covers it through stores.

use serde_json::json;
use vellum_types::{Actor, Origin, TurnId};

use super::{Footprint, History, TurnUndo};
use crate::ops::apply_edits;
use crate::ops::tests::common::{
    ALICE, BOB, Ids, alice, document, find, id, on_page, patch, person, shape, var_id, variable,
};
use crate::ops::{Op, Transaction, plan};

/// Plans and applies `ops` by `origin` on `doc`, recording it in `history`.
fn commit(history: &mut History, doc: &mut vellum_types::Document, origin: Origin, ops: Vec<Op>) {
    let planned = plan(
        doc,
        &Transaction::new(origin.clone(), ops),
        &mut Ids::default(),
    )
    .expect("the ops apply");
    let (inverse, _) = apply_edits(doc, &planned.edits, origin.clone()).expect("the edits apply");
    history.record(&origin, &planned.edits, inverse);
}

#[test]
fn between_finds_properties_inserts_removals_and_moves() {
    let before = document();
    let mut after = before.clone();
    let mut history = History::new();
    commit(
        &mut history,
        &mut after,
        alice(),
        vec![
            Op::Update {
                id: id("rect000001"),
                patch: patch(json!({ "name": "Renamed", "opacity": 0.5 })),
            },
            Op::Insert {
                to: on_page("canvas0001", 0),
                node: Box::new(shape("rect", "fresh00001", json!({}))),
            },
            Op::Delete {
                id: id("group00001"),
            },
            Op::Move {
                id: id("rect000002"),
                to: on_page("canvas0001", 0),
            },
        ],
    );
    let (found, changes) = Footprint::between(&before, &after, alice());
    assert_eq!(
        found.props[&id("rect000001")].iter().collect::<Vec<_>>(),
        ["name", "opacity"]
    );
    assert!(found.inserted.contains(&id("fresh00001")));
    assert!(found.removed.contains(&id("group00001")) && found.removed.contains(&id("ellipse001")));
    assert!(found.moved.contains(&id("rect000002")));
    assert!(found.parents.contains(&id("frame00001")));
    assert_eq!(
        changes.updated,
        [id("rect000001"), id("rect000002")],
        "a sibling coming or going is not a move: {changes:?}"
    );
    assert_eq!(changes.inserted, [id("fresh00001")]);
    assert_eq!(changes.removed, [id("ellipse001"), id("group00001")]);
}

#[test]
fn footprint_of_edits_matches_the_one_found_by_comparing() {
    let before = document();
    let mut after = before.clone();
    let origin = alice();
    let ops = vec![
        Op::Update {
            id: id("rect000003"),
            patch: patch(json!({ "name": "Changed" })),
        },
        Op::SetVariables {
            set: [(
                var_id("number0001"),
                variable(json!({"type": "number", "name": "space/md", "values": [{"value": 24}]})),
            )]
            .into(),
            remove: Default::default(),
        },
    ];
    let planned = plan(
        &before,
        &Transaction::new(origin.clone(), ops),
        &mut Ids::default(),
    )
    .expect("plan");
    let (inverse, _) = apply_edits(&mut after, &planned.edits, origin.clone()).expect("apply");
    let of = Footprint::of(&planned.edits, &inverse);
    let (between, _) = Footprint::between(&before, &after, origin);
    assert_eq!(of.props, between.props);
    assert_eq!(of.variables, between.variables);
}

#[test]
fn a_remote_change_is_later_work_but_never_undone_here() {
    let mut doc = document();
    let mut history = History::new();
    commit(
        &mut history,
        &mut doc,
        alice(),
        vec![Op::Update {
            id: id("rect000001"),
            patch: patch(json!({ "name": "Alice's" })),
        }],
    );
    let before = doc.clone();
    let mut theirs = doc.clone();
    let mut elsewhere = History::new();
    commit(
        &mut elsewhere,
        &mut theirs,
        Origin::new(person(BOB)),
        vec![Op::Update {
            id: id("rect000001"),
            patch: patch(json!({ "name": "Bob's" })),
        }],
    );
    history.record_remote(&before, &theirs, Origin::new(person(BOB)));
    doc = theirs;
    history
        .undo(&mut doc, &person(ALICE))
        .expect("alice undoes");
    assert_eq!(find(&doc, "rect000001").expect("node").name(), "Bob's");
    assert!(
        history.undo(&mut doc, &person(BOB)).is_none(),
        "a change from elsewhere was undone here"
    );
}

#[test]
fn undo_turn_with_nothing_kept_leaves_the_history() {
    let mut doc = document();
    let original = doc.clone();
    let mut history = History::new();
    let agent = Actor::Agent {
        agent: vellum_types::AgentId::new("01JA000000000000000000TEA1").expect("a ULID"),
    };
    let turn = TurnId::new("01JA0000000000000000000T01").expect("a ULID");
    let origin = Origin::new(agent.clone()).in_turn(turn.clone());
    for name in ["One", "Two"] {
        commit(
            &mut history,
            &mut doc,
            origin.clone(),
            vec![Op::Update {
                id: id("rect000001"),
                patch: patch(json!({ "name": name })),
            }],
        );
    }
    match history.undo_turn(&mut doc, &turn, &person(ALICE)) {
        TurnUndo::Undone(step) => assert_eq!(step.changes.origin.actor, person(ALICE)),
        other => panic!("expected the turn undone, got {other:?}"),
    }
    assert_eq!(doc, original);
    assert_eq!(
        history.undo_turn(&mut doc, &turn, &person(ALICE)),
        TurnUndo::Nothing
    );
    assert!(history.undo(&mut doc, &agent).is_none());
}

#[test]
fn slide_order_uses_attributed_history_and_survives_undo_redo() {
    use vellum_types::{Page, PageKind};
    let mut doc = document();
    let original = doc.clone();
    let mut history = History::new();
    commit(
        &mut history,
        &mut doc,
        alice(),
        vec![Op::InsertPage {
            index: 2,
            page: Box::new(Page {
                id: crate::ops::tests::common::page_id("deck000001"),
                name: "TEST DATA slides".into(),
                kind: PageKind::Slides,
                children: vec![
                    shape("frame", "slide00002", json!({})),
                    shape("note", "notes00001", json!({"content":"TEST DATA"})),
                    shape("frame", "slide00003", json!({})),
                ],
            }),
        }],
    );
    let target = vec!["deck000001".into()];
    assert_eq!(
        crate::export::pdf_targets(&doc, &target).unwrap(),
        ["slide00002", "slide00003"]
    );
    commit(
        &mut history,
        &mut doc,
        alice(),
        vec![Op::Reorder {
            id: id("slide00003"),
            index: 0,
        }],
    );
    assert_eq!(
        crate::export::pdf_targets(&doc, &target).unwrap(),
        ["slide00003", "slide00002"]
    );
    assert!(history.undo(&mut doc, &person(ALICE)).is_some());
    assert_eq!(
        crate::export::pdf_targets(&doc, &target).unwrap(),
        ["slide00002", "slide00003"]
    );
    assert!(history.redo(&mut doc, &person(ALICE)).is_some());
    assert_eq!(
        crate::export::pdf_targets(&doc, &target).unwrap(),
        ["slide00003", "slide00002"]
    );
    assert!(history.undo(&mut doc, &person(ALICE)).is_some());
    assert!(history.undo(&mut doc, &person(ALICE)).is_some());
    assert_eq!(doc, original);
}
