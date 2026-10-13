//! `DocStore`.
//!
//! A store is written to only through `vellum_core::ops::apply`, which checks each transaction against the
//! store's document and hands it over as one commit. The suite writes the same way, so it covers the store and
//! the write path together; the op layer's own tests (`vellum-core`, `ops::tests`) cover every op. Edits made on
//! two stores at the same time converging is left to the adapters' own tests, because it needs a real CRDT.
//! [`undo_is_yours`] is Invariant 3's property test, run by every store's tests.

use std::collections::BTreeMap;

use serde_json::json;
use vellum_core::ops::{self, Op, OpError, OpErrorKind, Transaction};
use vellum_core::ports::{ChangeSet, DocStore, Kept, SyncCursor, SyncError, SyncPayload, UndoTurn};
use vellum_types::{Actor, AgentId, Document, Origin, TurnId};

use crate::fakes::SequentialIds;

use crate::samples::{
    alice, all_ids, bob, document, find, insert_at, node, node_id, node_with, page_id, patch,
    rename, top_ids, tx,
};

fn fresh<S: DocStore>(make: &impl Fn(&Document, u64) -> S) -> S {
    make(&document(), 1)
}

fn ids(list: &[vellum_types::NodeId]) -> Vec<String> {
    list.iter().map(|id| id.to_string()).collect()
}

/// Writes `t` through the op layer, the only way in.
fn write(store: &mut impl DocStore, t: Transaction) -> Result<ChangeSet, OpError> {
    ops::apply(store, &mut SequentialIds::new(), t).map(|applied| applied.changes)
}

fn refused(store: &mut impl DocStore, t: Transaction) -> (usize, OpErrorKind) {
    let before = store.snapshot();
    let error = write(store, t).expect_err("the transaction should have been refused");
    assert_eq!(
        store.snapshot(),
        before,
        "DocStore: a refused transaction changed the document"
    );
    (error.op_index, error.kind)
}

/// Transactions apply whole or not at all, say what they changed, undo and redo per actor, and sync.
pub fn run<S: DocStore>(make: impl Fn(&Document, u64) -> S) {
    reads_back_what_it_was_given(&make);
    insert(&make);
    update(&make);
    delete(&make);
    refusals(&make);
    undo_and_redo(&make);
    undo_is_per_actor(&make);
    undo_keeps_later_work(&make);
    undo_turn(&make);
    undo_turn_with_later_work(&make);
    sync(&make);
    document_parts(&make);
}

/// Pages, the shell (meta, imports, themes, styles, fonts) and component definitions go through a store as
/// nodes do: applied as planned, reported, synced and undone. An external change reaches them as ops
/// (ARCHITECTURE §4.5), so a store must hold all of them.
fn document_parts<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    use std::collections::BTreeMap;
    use vellum_core::ops::Shell;
    use vellum_types::{ComponentId, PageKind};

    let comp = ComponentId::new("comp000001").expect("an id");
    let component = |name: &str| -> Box<vellum_types::Component> {
        Box::new(
            serde_json::from_value(json!({"name": name, "root": {
                "type": "frame", "id": "croot00001", "name": "Root", "width": 10, "height": 10,
                "children": [{"type": "rect", "id": "crect00001", "name": "R", "width": 4, "height": 4}]
            }}))
            .expect("a component"),
        )
    };
    let mut shell = Shell::of(&document());
    shell.meta.name = "TEST DATA renamed".into();
    shell.themes = BTreeMap::from([("mode".into(), vec!["light".into(), "dark".into()])]);
    let instance = node_with(
        "instance",
        "inst000001",
        &[],
        json!({"componentId": "comp000001"}),
    );
    let steps: Vec<(&str, Vec<Op>)> = vec![
        (
            "insert a page",
            vec![Op::InsertPage {
                index: 1,
                page: Box::new(crate::samples::page(
                    "page00003",
                    vec![node("rect", "pgrect0001")],
                )),
            }],
        ),
        (
            "move a page",
            vec![Op::MovePage {
                id: page_id("page00003"),
                index: 0,
            }],
        ),
        (
            "rename a page",
            vec![Op::UpdatePage {
                id: page_id("page00001"),
                name: "TEST DATA renamed".into(),
                kind: PageKind::Canvas,
            }],
        ),
        (
            "set the shell",
            vec![Op::SetShell {
                shell: Box::new(shell),
            }],
        ),
        (
            "add a component",
            vec![Op::SetComponent {
                id: comp.clone(),
                component: Some(component("TEST DATA Chip")),
            }],
        ),
        (
            "change a component",
            vec![Op::UpdateComponent {
                id: comp.clone(),
                name: "TEST DATA Tag".into(),
                props: BTreeMap::new(),
                variants: Vec::new(),
            }],
        ),
        ("use it", vec![insert_at("page00002", None, 0, instance)]),
        (
            "remove a page",
            vec![Op::DeletePage {
                id: page_id("page00002"),
            }],
        ),
        (
            "remove the component",
            vec![Op::SetComponent {
                id: comp.clone(),
                component: None,
            }],
        ),
    ];

    let a = alice();
    let mut left = make(&document(), 1);
    let mut states = vec![left.snapshot()];
    for (name, ops) in &steps {
        let before = left.snapshot();
        let expected = ops::plan(&before, &tx(&a, ops.clone()), &mut SequentialIds::new())
            .unwrap_or_else(|e| panic!("{name}: {e}"))
            .document;
        let changes =
            write(&mut left, tx(&a, ops.clone())).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            left.snapshot(),
            expected,
            "DocStore: {name} did not apply as planned"
        );
        assert!(!changes.is_empty(), "DocStore: {name} reported no change");
        states.push(left.snapshot());
    }
    assert_eq!(states[1].pages[1].id.as_str(), "page00003");
    assert!(states[4].themes.contains_key("mode"));

    let mut right = make(&document(), 2);
    right
        .import_updates(&left.export_updates(&right.cursor()))
        .expect("import");
    assert_eq!(
        right.snapshot(),
        left.snapshot(),
        "DocStore: pages, shell and components did not sync"
    );

    for (step, (name, _)) in steps.iter().enumerate().rev() {
        left.undo(&a)
            .unwrap_or_else(|| panic!("DocStore: nothing to undo for {name}"));
        assert_eq!(
            left.snapshot(),
            states[step],
            "DocStore: undoing {name} did not restore"
        );
    }
    for (step, (name, _)) in steps.iter().enumerate() {
        left.redo(&a)
            .unwrap_or_else(|| panic!("DocStore: nothing to redo for {name}"));
        assert_eq!(
            left.snapshot(),
            states[step + 1],
            "DocStore: redoing {name} did not restore"
        );
    }
    right
        .import_updates(&left.export_updates(&right.cursor()))
        .expect("import undo and redo");
    assert_eq!(right.snapshot(), left.snapshot());
}

fn teal() -> Actor {
    Actor::Agent {
        agent: AgentId::new("01JA000000000000000000TEA1").expect("a ULID"),
    }
}

fn turn(n: u8) -> TurnId {
    TurnId::new(format!("01JA0000000000000000000T{n:02}")).expect("a ULID")
}

fn in_turn(actor: &Actor, turn: &TurnId, ops: Vec<Op>) -> Transaction {
    Transaction::new(Origin::new(actor.clone()).in_turn(turn.clone()), ops)
}

fn name_of(doc: &Document, id: &str) -> String {
    find(doc, id).expect("the node").name().to_string()
}

fn lock(id: &str) -> Op {
    Op::Update {
        id: node_id(id),
        patch: patch(json!({ "locked": true })),
    }
}

fn undo_keeps_later_work<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let (a, b) = (alice(), bob());

    // Another property of the same node, changed since, stays.
    let mut store = fresh(make);
    write(&mut store, tx(&a, vec![rename("rect000001", "Alice's")])).expect("alice");
    write(&mut store, tx(&b, vec![lock("rect000001")])).expect("bob");
    store.undo(&a).expect("alice undoes");
    let doc = store.snapshot();
    let rect = find(&doc, "rect000001").expect("the node");
    assert_eq!(rect.name(), "TEST DATA rect000001");
    assert!(
        rect.locked(),
        "DocStore: Alice's undo put back a property Bob changed after her (Invariant 3)"
    );

    // The same property, changed since: the later value stays.
    let mut store = fresh(make);
    write(&mut store, tx(&a, vec![rename("rect000001", "Alice's")])).expect("alice");
    write(&mut store, tx(&b, vec![rename("rect000001", "Bob's")])).expect("bob");
    store.undo(&a).expect("alice undoes");
    assert_eq!(
        name_of(&store.snapshot(), "rect000001"),
        "Bob's",
        "DocStore: Alice's undo overwrote Bob's later value (Invariant 3)"
    );
    store.undo(&b).expect("bob undoes");
    assert_eq!(
        name_of(&store.snapshot(), "rect000001"),
        "Alice's",
        "DocStore: Bob's undo did not put back the value he replaced"
    );

    // A frame someone drew into stays, with what they drew; the rest of the transaction is undone.
    let mut store = fresh(make);
    let frame = node_with("frame", "aframe0001", &[], json!({}));
    write(
        &mut store,
        tx(
            &a,
            vec![
                insert_at("page00002", None, 0, frame),
                rename("rect000003", "Alice's"),
            ],
        ),
    )
    .expect("alice");
    write(
        &mut store,
        tx(
            &b,
            vec![insert_at(
                "page00002",
                Some("aframe0001"),
                0,
                node("rect", "brect00001"),
            )],
        ),
    )
    .expect("bob draws in alice's frame");
    store.undo(&a).expect("alice undoes");
    let doc = store.snapshot();
    assert_eq!(
        all_ids(&doc)
            .iter()
            .filter(|id| id.as_str() == "aframe0001" || id.as_str() == "brect00001")
            .count(),
        2,
        "DocStore: Alice's undo removed a frame Bob had drawn into (Invariant 3)"
    );
    assert_eq!(name_of(&doc, "rect000003"), "TEST DATA rect000003");

    // A node someone moved since stays where they put it.
    let mut store = fresh(make);
    let to = |page: &str, index| vellum_core::ops::Place {
        parent: vellum_core::ops::Parent::Page(page_id(page)),
        index,
    };
    write(
        &mut store,
        tx(
            &a,
            vec![Op::Move {
                id: node_id("rect000001"),
                to: to("page00002", 0),
            }],
        ),
    )
    .expect("alice moves");
    write(
        &mut store,
        tx(
            &b,
            vec![Op::Move {
                id: node_id("rect000001"),
                to: to("page00001", 0),
            }],
        ),
    )
    .expect("bob moves it again");
    store.undo(&a).expect("alice undoes");
    assert_eq!(
        top_ids(&store.snapshot(), "page00001")[0],
        "rect000001",
        "DocStore: Alice's undo moved back a node Bob moved after her (Invariant 3)"
    );

    // Redo puts back what the undo reverted and still keeps later work.
    let mut store = fresh(make);
    write(&mut store, tx(&a, vec![rename("rect000001", "Alice's")])).expect("alice");
    store.undo(&a).expect("alice undoes");
    write(&mut store, tx(&b, vec![lock("rect000001")])).expect("bob");
    let redone = store.redo(&a).expect("alice redoes");
    assert_eq!(ids(&redone.updated), ["rect000001"]);
    assert_eq!(redone.origin.actor, a);
    let doc = store.snapshot();
    assert_eq!(name_of(&doc, "rect000001"), "Alice's");
    assert!(
        find(&doc, "rect000001").expect("node").locked(),
        "DocStore: Alice's redo put back a property Bob changed after her undo"
    );
    store.undo(&a).expect("alice undoes the redo");
    assert_eq!(
        name_of(&store.snapshot(), "rect000001"),
        "TEST DATA rect000001"
    );
    assert!(
        find(&store.snapshot(), "rect000001")
            .expect("node")
            .locked()
    );
}

fn undo_turn<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut store = fresh(make);
    let (a, agent) = (alice(), teal());
    let original = store.snapshot();
    assert_eq!(
        store.undo_turn(&turn(1), &a),
        UndoTurn::Nothing,
        "DocStore: a turn with no changes has nothing to undo"
    );

    write(
        &mut store,
        in_turn(
            &agent,
            &turn(1),
            vec![insert_at(
                "page00002",
                None,
                0,
                node_with(
                    "frame",
                    "tframe0001",
                    &[node("rect", "trect00001")],
                    json!({}),
                ),
            )],
        ),
    )
    .expect("teal, turn 1");
    write(&mut store, tx(&a, vec![rename("rect000002", "Alice's")])).expect("alice");
    write(
        &mut store,
        in_turn(&agent, &turn(1), vec![rename("rect000001", "Teal's")]),
    )
    .expect("teal, turn 1 again");
    write(
        &mut store,
        in_turn(&agent, &turn(2), vec![rename("rect000003", "Teal's")]),
    )
    .expect("teal, turn 2");

    let undone = match store.undo_turn(&turn(1), &a) {
        UndoTurn::Undone(changes) => changes,
        other => panic!("DocStore: a turn nothing depends on is undone outright, got {other:?}"),
    };
    assert_eq!(
        undone.origin.actor, a,
        "DocStore: undoing a turn is attributed to whoever asked"
    );
    assert_eq!(ids(&undone.removed), ["tframe0001", "trect00001"]);
    assert_eq!(ids(&undone.updated), ["rect000001"]);
    let doc = store.snapshot();
    assert!(top_ids(&doc, "page00002").is_empty());
    assert_eq!(
        name_of(&doc, "rect000001"),
        name_of(&original, "rect000001")
    );
    assert_eq!(
        name_of(&doc, "rect000002"),
        "Alice's",
        "DocStore: undoing a turn touched Alice's work"
    );
    assert_eq!(
        name_of(&doc, "rect000003"),
        "Teal's",
        "DocStore: undoing a turn touched another turn"
    );
    assert_eq!(
        store.undo_turn(&turn(1), &a),
        UndoTurn::Nothing,
        "DocStore: a turn was undone twice"
    );
    store.undo(&a).expect("alice's own undo still works");
    assert_eq!(
        name_of(&store.snapshot(), "rect000002"),
        "TEST DATA rect000002"
    );
    store.undo(&agent).expect("teal undoes turn 2");
    assert!(
        store.undo(&agent).is_none(),
        "DocStore: an undone turn came back on the agent's undo stack"
    );
}

fn undo_turn_with_later_work<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut store = fresh(make);
    let (a, agent) = (alice(), teal());
    write(
        &mut store,
        in_turn(
            &agent,
            &turn(1),
            vec![
                insert_at(
                    "page00002",
                    None,
                    0,
                    node_with("frame", "tframe0001", &[], json!({})),
                ),
                rename("rect000001", "Teal's"),
                rename("rect000002", "Teal's"),
            ],
        ),
    )
    .expect("teal");
    write(
        &mut store,
        tx(
            &a,
            vec![
                insert_at(
                    "page00002",
                    Some("tframe0001"),
                    0,
                    node("rect", "arect00001"),
                ),
                rename("rect000002", "Alice's"),
            ],
        ),
    )
    .expect("alice builds on the turn");

    let before = store.snapshot();
    let revert = match store.undo_turn(&turn(1), &a) {
        UndoTurn::Revert(revert) => revert,
        other => panic!(
            "DocStore: later work depends on the turn, so undoing it must be a Revert, got {other:?}"
        ),
    };
    assert_eq!(
        store.snapshot(),
        before,
        "DocStore: building a Revert changed the document"
    );
    assert_eq!(revert.origin().actor, a);
    assert_eq!(
        revert.kept(),
        [
            Kept::Node(node_id("rect000002")),
            Kept::Node(node_id("tframe0001"))
        ],
        "DocStore: a Revert names what it keeps"
    );

    let applied = ops::revert(&mut store, revert).expect("apply the revert");
    assert_eq!(applied.changes.origin.actor, a);
    let doc = store.snapshot();
    assert_eq!(
        name_of(&doc, "rect000001"),
        "TEST DATA rect000001",
        "DocStore: the Revert did not revert what nothing depends on"
    );
    assert_eq!(
        name_of(&doc, "rect000002"),
        "Alice's",
        "DocStore: the Revert dropped Alice's later change (Invariant 3)"
    );
    assert!(
        all_ids(&doc).contains(&"arect00001".to_string()),
        "DocStore: the Revert removed the frame Alice drew into"
    );

    // The Revert is a new transaction of Alice's: her undo takes it back.
    store.undo(&a).expect("alice undoes the revert");
    assert_eq!(
        store.snapshot(),
        before,
        "DocStore: undoing a Revert did not bring the turn's changes back"
    );
}

fn reads_back_what_it_was_given<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let store = fresh(make);
    assert_eq!(
        store.snapshot(),
        document(),
        "DocStore: the snapshot is not the document it was given"
    );
    assert!(
        !store.adapter_id().is_empty(),
        "DocStore: the adapter id is empty"
    );
    assert_eq!(store.adapter_id(), store.adapter_id());
}

fn insert<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut store = fresh(make);
    let origin = Origin::new(alice())
        .in_session(vellum_types::SessionId::new("01JA00000000000000000SE551").expect("a ULID"));
    let changes = write(
        &mut store,
        Transaction {
            origin: origin.clone(),
            ops: vec![insert_at("page00001", None, 2, node("rect", "new0000001"))],
        },
    )
    .expect("insert at the end");
    assert_eq!(
        changes.origin, origin,
        "DocStore: the change set lost its origin (Invariant 2)"
    );
    assert_eq!(ids(&changes.inserted), ["new0000001"]);
    assert!(changes.updated.is_empty() && changes.removed.is_empty());
    assert_eq!(
        top_ids(&store.snapshot(), "page00001"),
        ["frame00001", "rect000003", "new0000001"]
    );

    write(
        &mut store,
        tx(
            &alice(),
            vec![insert_at("page00001", None, 0, node("rect", "new0000002"))],
        ),
    )
    .expect("insert at the start");
    assert_eq!(top_ids(&store.snapshot(), "page00001")[0], "new0000002");

    let group = node_with(
        "group",
        "group00001",
        &[node("rect", "kid0000001"), node("rect", "kid0000002")],
        json!({}),
    );
    let changes = write(
        &mut store,
        tx(
            &alice(),
            vec![insert_at("page00001", Some("frame00001"), 1, group)],
        ),
    )
    .expect("insert a subtree into a frame");
    assert_eq!(
        ids(&changes.inserted),
        ["group00001", "kid0000001", "kid0000002"],
        "DocStore: a change set lists every node of an inserted subtree, sorted"
    );
    let doc = store.snapshot();
    let frame = find(&doc, "frame00001").expect("the frame");
    let order: Vec<_> = frame
        .children()
        .iter()
        .map(|n| n.id().to_string())
        .collect();
    assert_eq!(
        order,
        ["rect000001", "group00001", "rect000002"],
        "DocStore: the subtree went to the wrong place"
    );

    let changes = write(
        &mut store,
        tx(
            &alice(),
            vec![
                insert_at("page00002", None, 0, node("rect", "multi00001")),
                insert_at("page00002", Some("multi00001"), 0, node("rect", "never")),
            ],
        ),
    );
    assert!(
        changes.is_err(),
        "DocStore: a rectangle cannot hold children"
    );
}

fn update<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut store = fresh(make);
    let changes = write(
        &mut store,
        tx(&alice(), vec![rename("rect000001", "Renamed")]),
    )
    .expect("rename");
    assert_eq!(ids(&changes.updated), ["rect000001"]);
    assert!(changes.inserted.is_empty() && changes.removed.is_empty());
    assert_eq!(
        find(&store.snapshot(), "rect000001").expect("node").name(),
        "Renamed"
    );

    write(
        &mut store,
        tx(
            &alice(),
            vec![Op::Update {
                id: node_id("frame00001"),
                patch: patch(json!({ "locked": true })),
            }],
        ),
    )
    .expect("lock the frame");
    let doc = store.snapshot();
    let frame = find(&doc, "frame00001").expect("frame");
    assert!(frame.locked());
    assert_eq!(
        frame.children().len(),
        2,
        "DocStore: updating a frame lost its children"
    );

    let two = write(
        &mut store,
        tx(
            &alice(),
            vec![
                rename("rect000001", "One"),
                rename("rect000001", "Two"),
                rename("rect000002", "Three"),
            ],
        ),
    )
    .expect("several updates");
    assert_eq!(
        ids(&two.updated),
        ["rect000001", "rect000002"],
        "DocStore: a node updated twice is listed once"
    );
    assert_eq!(
        find(&store.snapshot(), "rect000001").expect("node").name(),
        "Two",
        "DocStore: ops did not run in order"
    );
}

fn delete<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut store = fresh(make);
    let changes = write(
        &mut store,
        tx(
            &alice(),
            vec![Op::Delete {
                id: node_id("frame00001"),
            }],
        ),
    )
    .expect("delete a frame");
    assert_eq!(
        ids(&changes.removed),
        ["frame00001", "rect000001", "rect000002"]
    );
    assert_eq!(all_ids(&store.snapshot()), ["rect000003"]);

    let changes = write(
        &mut store,
        tx(
            &alice(),
            vec![
                insert_at("page00002", None, 0, node("rect", "gone000001")),
                Op::Delete {
                    id: node_id("gone000001"),
                },
            ],
        ),
    )
    .expect("insert then delete in one transaction");
    assert!(
        changes.inserted.is_empty() && changes.removed.is_empty(),
        "DocStore: a node made and removed in one transaction never existed: {changes:?}"
    );
}

fn refusals<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut store = fresh(make);
    let a = alice();

    let (index, kind) = refused(&mut store, tx(&a, vec![]));
    assert_eq!((index, kind), (0, OpErrorKind::Empty));

    let (_, kind) = refused(
        &mut store,
        tx(
            &a,
            vec![insert_at("nopage", None, 0, node("rect", "x000000001"))],
        ),
    );
    assert_eq!(kind, OpErrorKind::UnknownPage(page_id("nopage")));

    let (_, kind) = refused(&mut store, tx(&a, vec![rename("nonode", "x")]));
    assert_eq!(kind, OpErrorKind::UnknownNode(node_id("nonode")));
    let (_, kind) = refused(
        &mut store,
        tx(
            &a,
            vec![Op::Delete {
                id: node_id("nonode"),
            }],
        ),
    );
    assert_eq!(kind, OpErrorKind::UnknownNode(node_id("nonode")));
    let (_, kind) = refused(
        &mut store,
        tx(
            &a,
            vec![insert_at(
                "page00001",
                Some("nonode"),
                0,
                node("rect", "x000000001"),
            )],
        ),
    );
    assert_eq!(kind, OpErrorKind::UnknownNode(node_id("nonode")));

    let (_, kind) = refused(
        &mut store,
        tx(
            &a,
            vec![insert_at(
                "page00001",
                Some("rect000003"),
                0,
                node("rect", "x000000001"),
            )],
        ),
    );
    assert_eq!(kind, OpErrorKind::NotAContainer(node_id("rect000003")));

    let (_, kind) = refused(
        &mut store,
        tx(
            &a,
            vec![insert_at("page00002", None, 0, node("rect", "rect000001"))],
        ),
    );
    assert_eq!(
        kind,
        OpErrorKind::DuplicateId(node_id("rect000001")),
        "DocStore: an id taken on another page was reused"
    );
    let twins = node_with(
        "group",
        "twin000001",
        &[node("rect", "twin000001")],
        json!({}),
    );
    let (_, kind) = refused(
        &mut store,
        tx(&a, vec![insert_at("page00002", None, 0, twins)]),
    );
    assert_eq!(
        kind,
        OpErrorKind::DuplicateId(node_id("twin000001")),
        "DocStore: an id was reused inside one subtree"
    );

    let (_, kind) = refused(
        &mut store,
        tx(
            &a,
            vec![insert_at("page00001", None, 3, node("rect", "x000000001"))],
        ),
    );
    assert_eq!(kind, OpErrorKind::IndexOutOfRange { index: 3, len: 2 });

    let (_, kind) = refused(
        &mut store,
        tx(
            &a,
            vec![Op::Update {
                id: node_id("rect000001"),
                patch: patch(json!({ "width": "not a size" })),
            }],
        ),
    );
    assert!(
        matches!(kind, OpErrorKind::InvalidPatch { .. }),
        "DocStore: a patch that leaves no valid node was accepted"
    );

    // All or nothing, and the error says which op.
    let (index, kind) = refused(
        &mut store,
        tx(
            &a,
            vec![
                rename("rect000001", "Changed"),
                Op::Delete {
                    id: node_id("rect000002"),
                },
                rename("nonode", "x"),
            ],
        ),
    );
    assert_eq!(
        (index, kind),
        (2, OpErrorKind::UnknownNode(node_id("nonode")))
    );
    assert!(
        store.undo(&a).is_none(),
        "DocStore: a refused transaction left something to undo"
    );
}

fn undo_and_redo<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut store = fresh(make);
    let a = alice();
    assert!(
        store.undo(&a).is_none() && store.redo(&a).is_none(),
        "DocStore: nothing to undo or redo yet"
    );

    let original = store.snapshot();
    write(
        &mut store,
        tx(
            &a,
            vec![insert_at("page00001", None, 2, node("rect", "new0000001"))],
        ),
    )
    .expect("insert");
    let after = store.snapshot();
    let undone = store.undo(&a).expect("undo the insert");
    assert_eq!(ids(&undone.removed), ["new0000001"]);
    assert_eq!(
        undone.origin.actor, a,
        "DocStore: an undo is attributed to the person who pressed it"
    );
    assert_eq!(
        store.snapshot(),
        original,
        "DocStore: undo did not restore the document"
    );
    assert!(
        store.undo(&a).is_none(),
        "DocStore: undo went past the start"
    );
    let redone = store.redo(&a).expect("redo");
    assert_eq!(ids(&redone.inserted), ["new0000001"]);
    assert_eq!(
        store.snapshot(),
        after,
        "DocStore: redo did not restore the change"
    );
    assert!(store.redo(&a).is_none());

    // Update and delete come back exactly: values, and position among siblings.
    store.undo(&a).expect("undo again");
    write(&mut store, tx(&a, vec![rename("rect000001", "Renamed")])).expect("rename");
    store.undo(&a).expect("undo the rename");
    assert_eq!(
        store.snapshot(),
        original,
        "DocStore: undoing an update lost the old value"
    );
    write(
        &mut store,
        tx(
            &a,
            vec![Op::Delete {
                id: node_id("frame00001"),
            }],
        ),
    )
    .expect("delete");
    store.undo(&a).expect("undo the delete");
    assert_eq!(
        store.snapshot(),
        original,
        "DocStore: undoing a delete did not put the subtree back in place"
    );

    // A new change forgets what could have been redone.
    write(&mut store, tx(&a, vec![rename("rect000001", "First")])).expect("rename");
    store.undo(&a).expect("undo");
    write(&mut store, tx(&a, vec![rename("rect000001", "Second")])).expect("rename");
    assert!(
        store.redo(&a).is_none(),
        "DocStore: a new change left the old redo"
    );
}

fn undo_is_per_actor<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut store = fresh(make);
    let (a, b): (Actor, Actor) = (alice(), bob());
    write(
        &mut store,
        tx(
            &a,
            vec![insert_at("page00002", None, 0, node("rect", "alice00001"))],
        ),
    )
    .expect("alice");
    write(
        &mut store,
        tx(
            &b,
            vec![insert_at("page00002", None, 1, node("rect", "bob0000001"))],
        ),
    )
    .expect("bob");
    write(&mut store, tx(&a, vec![rename("rect000001", "Alice's")])).expect("alice again");

    store.undo(&a).expect("alice undoes her rename");
    assert_eq!(
        find(&store.snapshot(), "rect000001").expect("node").name(),
        "TEST DATA rect000001"
    );
    store.undo(&a).expect("alice undoes her insert");
    let doc = store.snapshot();
    assert_eq!(
        top_ids(&doc, "page00002"),
        ["bob0000001"],
        "DocStore: Alice's undo touched Bob's work"
    );
    assert!(
        store.undo(&a).is_none(),
        "DocStore: Alice undid something that was not hers"
    );

    store.undo(&b).expect("bob undoes his insert");
    assert!(top_ids(&store.snapshot(), "page00002").is_empty());
    assert!(
        store.redo(&a).is_some(),
        "DocStore: Bob's undo cleared Alice's redo"
    );
}

fn sync<S: DocStore>(make: &impl Fn(&Document, u64) -> S) {
    let mut left = make(&document(), 1);
    let mut right = make(&document(), 2);
    let a = alice();

    write(
        &mut left,
        tx(
            &a,
            vec![insert_at("page00002", None, 0, node("rect", "synced0001"))],
        ),
    )
    .expect("left insert");
    write(&mut left, tx(&a, vec![rename("rect000001", "Synced")])).expect("left rename");

    let payload = left.export_updates(&right.cursor());
    assert_eq!(
        payload.adapter,
        left.adapter_id(),
        "DocStore: a payload must name its adapter"
    );
    let imported = right.import_updates(&payload).expect("import");
    assert_eq!(ids(&imported.inserted), ["synced0001"]);
    assert_eq!(ids(&imported.updated), ["rect000001"]);
    assert_eq!(
        right.snapshot(),
        left.snapshot(),
        "DocStore: an import did not bring the stores level"
    );

    let again = right.import_updates(&payload).expect("import again");
    assert!(
        again.is_empty(),
        "DocStore: importing the same updates twice changed something: {again:?}"
    );
    assert_eq!(right.snapshot(), left.snapshot());

    // Everything from nothing, into a store that started from the same document.
    let mut third = make(&document(), 3);
    third
        .import_updates(&left.export_updates(&SyncCursor::empty()))
        .expect("import everything");
    assert_eq!(third.snapshot(), left.snapshot());

    // And the other way, one after the other.
    write(
        &mut right,
        tx(
            &bob(),
            vec![Op::Delete {
                id: node_id("rect000003"),
            }],
        ),
    )
    .expect("right delete");
    left.import_updates(&right.export_updates(&left.cursor()))
        .expect("import into left");
    assert_eq!(
        left.snapshot(),
        right.snapshot(),
        "DocStore: updates did not travel back"
    );
    assert!(
        !all_ids(&left.snapshot()).contains(&"rect000003".to_string()),
        "DocStore: a delete did not travel"
    );

    // What cannot be read is refused, and the store stays as it was.
    let before = left.snapshot();
    let foreign = SyncPayload {
        adapter: "someone-else".into(),
        bytes: payload.bytes.clone(),
    };
    assert!(
        matches!(
            left.import_updates(&foreign),
            Err(SyncError::AdapterMismatch { .. })
        ),
        "DocStore: updates from another adapter were accepted"
    );
    let garbage = SyncPayload {
        adapter: left.adapter_id().into(),
        bytes: b"not updates".to_vec(),
    };
    assert!(
        matches!(left.import_updates(&garbage), Err(SyncError::Malformed(_))),
        "DocStore: damaged updates were accepted"
    );
    assert_eq!(
        left.snapshot(),
        before,
        "DocStore: a refused import changed the document"
    );
}

/// A small, seeded generator: the property test is the same on every run and every machine.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Containers on the sample document's pages: where generated nodes may go.
fn frames(doc: &Document) -> Vec<String> {
    all_ids(doc)
        .into_iter()
        .filter(|id| find(doc, id).is_some_and(|n| n.kind() == vellum_types::NodeKind::Frame))
        .collect()
}

fn property(doc: &Document, id: &str, key: &str) -> Option<serde_json::Value> {
    find(doc, id)?
        .property_patch()
        .properties()
        .get(key)
        .cloned()
}

/// Invariant 3's property test: three actors interleave transactions (renames, opacity and rotation changes,
/// inserts, moves, deletes), then Alice undoes and redoes at random. Every property last changed by Bob or
/// Carol keeps their value, and every node they made is still there. Setting a property to the value it already
/// has is not a change.
pub fn undo_is_yours<S: DocStore>(make: impl Fn(&Document, u64) -> S, seeds: std::ops::Range<u64>) {
    let carol = Actor::Agent {
        agent: AgentId::new("01JA00000000000000000CA401").expect("a ULID"),
    };
    let actors = [alice(), bob(), carol];
    for seed in seeds {
        let mut rng = SplitMix(seed);
        let mut store = make(&document(), 1);
        // (node, property) -> (actor index, value) of the latest write.
        let mut written: BTreeMap<(String, String), (usize, serde_json::Value)> = BTreeMap::new();
        let mut made_by: BTreeMap<String, usize> = BTreeMap::new();
        let mut next_id = 0;
        for _ in 0..24 {
            let who = rng.below(3);
            let doc = store.snapshot();
            let ids = all_ids(&doc);
            let mut ops = Vec::new();
            let mut writes = Vec::new();
            let mut made = Vec::new();
            for _ in 0..1 + rng.below(3) {
                let pick = rng.below(7);
                let pick = if ids.is_empty() { 3 } else { pick };
                let target = ids.get(rng.below(ids.len())).cloned().unwrap_or_default();
                match pick {
                    0..=2 => {
                        let (key, value) = match rng.below(3) {
                            0 => (
                                "name",
                                json!(format!("TEST DATA {seed}-{}", rng.below(1000))),
                            ),
                            1 => ("opacity", json!((1 + rng.below(8)) as f64 / 10.0)),
                            _ => ("rotation", json!((1 + rng.below(35)) as f64 * 10.0)),
                        };
                        ops.push(Op::Update {
                            id: node_id(&target),
                            patch: patch(json!({ key: value })),
                        });
                        writes.push((target, key.to_string(), value));
                    }
                    3 | 4 => {
                        next_id += 1;
                        let id = format!("gen{seed:03}{next_id:04}");
                        let kind = if rng.below(2) == 0 { "frame" } else { "rect" };
                        let homes = frames(&doc);
                        let parent = (rng.below(2) == 0 && !homes.is_empty())
                            .then(|| homes[rng.below(homes.len())].clone());
                        ops.push(insert_at(
                            "page00002",
                            parent.as_deref(),
                            0,
                            node(kind, &id),
                        ));
                        made.push(id);
                    }
                    5 => {
                        ops.push(Op::Move {
                            id: node_id(&target),
                            to: vellum_core::ops::Place {
                                parent: vellum_core::ops::Parent::Page(page_id("page00002")),
                                index: 0,
                            },
                        });
                    }
                    _ => ops.push(Op::Delete {
                        id: node_id(&target),
                    }),
                }
            }
            if write(&mut store, tx(&actors[who], ops)).is_ok() {
                // A write that leaves the value as it was is no change, and not anyone's to keep.
                for (id, key, value) in writes {
                    if property(&doc, &id, &key).as_ref() != Some(&value) {
                        written.insert((id, key), (who, value));
                    }
                }
                for id in made {
                    made_by.insert(id, who);
                }
            }
        }

        let before = store.snapshot();
        let present = all_ids(&before);
        for _ in 0..1 + rng.below(12) {
            if rng.below(4) == 0 {
                store.redo(&actors[0]);
            } else {
                store.undo(&actors[0]);
            }
        }
        let after = store.snapshot();
        let now = all_ids(&after);
        for id in &present {
            if made_by.get(id).is_some_and(|who| *who != 0) {
                assert!(
                    now.contains(id),
                    "DocStore: seed {seed}: Alice's undo removed {id}, which someone else made (Invariant 3)"
                );
            }
        }
        for ((id, key), (who, value)) in &written {
            if *who == 0 || !present.contains(id) {
                continue;
            }
            assert!(
                now.contains(id),
                "DocStore: seed {seed}: Alice's undo removed {id}, which someone else changed (Invariant 3)"
            );
            assert_eq!(
                property(&before, id, key).as_ref(),
                Some(value),
                "DocStore: seed {seed}: {key} of {id} does not hold the value last written"
            );
            assert_eq!(
                property(&after, id, key).as_ref(),
                Some(value),
                "DocStore: seed {seed}: Alice's undo changed {key} of {id}, last written by someone else \
                 (Invariant 3)"
            );
        }
    }
}
