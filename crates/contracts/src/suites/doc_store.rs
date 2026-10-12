//! `DocStore`.
//!
//! A store is written to only through `vellum_core::ops::apply`, which checks each transaction against the
//! store's document and hands it over as one commit. The suite writes the same way, so it covers the store and
//! the write path together; the op layer's own tests (`vellum-core`, `ops::tests`) cover every op. Two things
//! are left to the adapters' own tests because they need a real CRDT: edits made on two stores at the same
//! time converging (Prompt 14), and undo across dependent changes (Invariant 3's property test).

use serde_json::json;
use vellum_core::ops::{self, Op, OpError, OpErrorKind, Transaction};
use vellum_core::ports::{ChangeSet, DocStore, SyncCursor, SyncError, SyncPayload};
use vellum_types::{Actor, Document, Origin};

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
    sync(&make);
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
