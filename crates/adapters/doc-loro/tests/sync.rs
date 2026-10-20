//! Updates travel between Loro stores: one after another, at the same time, and from a store saved and opened
//! again (for Prompt 63's sync).

use serde_json::json;
use vellum_contracts::fakes::{FakeJsonCodec, SequentialIds};
use vellum_contracts::samples::{
    alice, all_ids, bob, document, find, insert_at, node, node_id, patch, rename, top_ids, tx,
};
use vellum_core::ops::{self, Op, Transaction};
use vellum_core::ports::{Codec, DocStore, SyncCursor, SyncError};
use vellum_doc_loro::LoroDocStore;
use vellum_types::Document;

fn write(store: &mut LoroDocStore, t: Transaction) {
    ops::apply(store, &mut SequentialIds::new(), t).expect("the transaction applies");
}

fn exchange(left: &mut LoroDocStore, right: &mut LoroDocStore) {
    let to_right = left.export_updates(&right.cursor());
    let to_left = right.export_updates(&left.cursor());
    right.import_updates(&to_right).expect("import into right");
    left.import_updates(&to_left).expect("import into left");
}

#[test]
fn edits_made_at_the_same_time_converge() {
    let mut left = LoroDocStore::new(&document(), 1);
    let mut right = LoroDocStore::new(&document(), 2);

    write(&mut left, tx(&alice(), vec![rename("rect000001", "Left")]));
    write(
        &mut left,
        tx(
            &alice(),
            vec![insert_at("page00002", None, 0, node("rect", "left000001"))],
        ),
    );
    write(
        &mut right,
        tx(
            &bob(),
            vec![Op::Update {
                id: node_id("rect000001"),
                patch: patch(json!({ "locked": true })),
            }],
        ),
    );
    write(
        &mut right,
        tx(
            &bob(),
            vec![insert_at("page00002", None, 0, node("rect", "right00001"))],
        ),
    );
    write(
        &mut right,
        tx(
            &bob(),
            vec![Op::Delete {
                id: node_id("rect000003"),
            }],
        ),
    );

    exchange(&mut left, &mut right);
    let (l, r) = (left.snapshot(), right.snapshot());
    assert_eq!(l, r, "stores that edited at the same time did not converge");
    let rect = find(&l, "rect000001").expect("the node");
    assert_eq!(
        rect.name(),
        "Left",
        "a concurrent edit to another property was lost"
    );
    assert!(
        rect.locked(),
        "a concurrent edit to another property was lost"
    );
    let mut page = top_ids(&l, "page00002");
    page.sort();
    assert_eq!(page, ["left000001", "right00001"]);
    assert!(!all_ids(&l).contains(&"rect000003".to_string()));
}

#[test]
fn authenticated_import_checks_all_origins_and_is_atomic() {
    let mut client = LoroDocStore::new(&document(), 1);
    write(
        &mut client,
        tx(&bob(), vec![rename("rect000001", "TEST DATA: forged")]),
    );
    write(
        &mut client,
        tx(&alice(), vec![rename("rect000002", "TEST DATA: allowed")]),
    );
    let mut server = LoroDocStore::new(&document(), 9);
    let before = server.cursor();
    assert!(
        server
            .import_authenticated(&client.export_updates(&SyncCursor::empty()), &alice())
            .is_err()
    );
    assert_eq!(server.cursor(), before);
    assert_eq!(server.snapshot(), document());
}

#[test]
fn native_updates_cannot_write_invalid_document_content_or_panic() {
    use loro::{ExportMode, LoroDoc};
    let store = LoroDocStore::new(&document(), 1);
    let native = LoroDoc::new();
    native
        .import(&store.export_updates(&SyncCursor::empty()).bytes)
        .unwrap();
    native.set_peer_id(12).unwrap();
    native
        .get_map("document")
        .insert("shell", "invalid json")
        .unwrap();
    native.set_next_commit_message(
        &serde_json::to_string(&vellum_types::Origin::new(alice())).unwrap(),
    );
    native.commit();
    let malicious = vellum_core::ports::SyncPayload {
        adapter: "loro-1".into(),
        bytes: native.export(ExportMode::all_updates()).unwrap(),
    };
    let mut server = LoroDocStore::new(&document(), 9);
    let before = server.cursor();
    assert!(server.import_authenticated(&malicious, &alice()).is_err());
    assert_eq!(server.cursor(), before);
    assert_eq!(server.read_back(), document());
}

#[test]
fn a_move_and_a_delete_at_the_same_time_converge() {
    let mut left = LoroDocStore::new(&document(), 1);
    let mut right = LoroDocStore::new(&document(), 2);
    write(
        &mut left,
        tx(
            &alice(),
            vec![Op::Move {
                id: node_id("rect000001"),
                to: vellum_core::ops::Place {
                    parent: vellum_core::ops::Parent::Page(vellum_contracts::samples::page_id(
                        "page00002",
                    )),
                    index: 0,
                },
            }],
        ),
    );
    write(
        &mut right,
        tx(
            &bob(),
            vec![Op::Delete {
                id: node_id("frame00001"),
            }],
        ),
    );
    exchange(&mut left, &mut right);
    assert_eq!(left.snapshot(), right.snapshot());
    // Loro's movable tree: the move out of the frame was concurrent with the frame's removal, and both hold.
    assert_eq!(top_ids(&left.snapshot(), "page00002"), ["rect000001"]);
}

#[test]
fn a_store_opened_from_saved_updates_reads_the_same_document() {
    let mut store = LoroDocStore::new(&document(), 1);
    write(
        &mut store,
        tx(&alice(), vec![rename("rect000002", "Saved")]),
    );
    let everything = store.export_updates(&SyncCursor::empty());
    let mut reopened = LoroDocStore::new(&document(), 7);
    reopened.import_updates(&everything).expect("import");
    assert_eq!(reopened.snapshot(), store.snapshot());
    assert!(
        reopened
            .import_updates(&everything)
            .expect("again")
            .is_empty(),
        "importing the same updates twice changed something"
    );
}

#[test]
fn updates_for_another_document_are_refused() {
    let mut other = document();
    other.meta.name = "TEST DATA: another document".into();
    let mut theirs = LoroDocStore::new(&other, 2);
    write(
        &mut theirs,
        tx(&bob(), vec![rename("rect000001", "Theirs")]),
    );
    let mut ours = LoroDocStore::new(&document(), 1);
    let before = ours.snapshot();
    assert_eq!(
        ours.import_updates(&theirs.export_updates(&SyncCursor::empty())),
        Err(SyncError::DifferentDocument)
    );
    assert_eq!(ours.snapshot(), before);
}

#[test]
fn remote_changes_count_as_later_work_for_undo() {
    let mut left = LoroDocStore::new(&document(), 1);
    let mut right = LoroDocStore::new(&document(), 2);
    write(
        &mut left,
        tx(&alice(), vec![rename("rect000001", "Alice's")]),
    );
    exchange(&mut left, &mut right);
    write(&mut right, tx(&bob(), vec![rename("rect000001", "Bob's")]));
    exchange(&mut left, &mut right);
    left.undo(&alice()).expect("alice undoes");
    assert_eq!(
        find(&left.snapshot(), "rect000001").expect("node").name(),
        "Bob's",
        "Alice's undo overwrote Bob's later change made on another device (Invariant 3)"
    );
    exchange(&mut left, &mut right);
    assert_eq!(left.snapshot(), right.snapshot());
}

fn fixture(name: &str) -> Document {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/documents")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    FakeJsonCodec
        .decode(&bytes)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn the_snapshot_is_what_the_codec_decodes() {
    for name in [
        "minimal.vellum",
        "shapes.vellum",
        "content.vellum",
        "design-system.vellum",
        "library.lib.vellum",
    ] {
        let doc = fixture(name);
        let store = LoroDocStore::new(&doc, 1);
        assert_eq!(
            store.snapshot(),
            doc,
            "{name}: the snapshot is not the decoded document"
        );
        // A second store reads the document from Loro alone, through sync.
        let mut other = LoroDocStore::new(&Document::new("TEST DATA: empty"), 2);
        other
            .import_updates(&store.export_updates(&SyncCursor::empty()))
            .expect_err("a store opened from another document refuses the updates");
        let mut same = LoroDocStore::new(&doc, 3);
        same.import_updates(&store.export_updates(&SyncCursor::empty()))
            .expect("import");
        assert_eq!(same.snapshot(), doc, "{name}");
        assert_eq!(
            store.read_back(),
            doc,
            "{name}: Loro's state does not read back as the document"
        );
    }
}

#[test]
fn missing_dependencies_are_rejected_without_parked_changes() {
    let mut client = LoroDocStore::new(&document(), 1);
    write(
        &mut client,
        tx(&alice(), vec![rename("rect000001", "TEST DATA: first")]),
    );
    let cursor = client.cursor();
    write(
        &mut client,
        tx(
            &alice(),
            vec![rename("rect000002", "TEST DATA: depends on first")],
        ),
    );
    let mut server = LoroDocStore::new(&document(), 9);
    let before = server.cursor();
    assert!(
        server
            .import_authenticated(&client.export_updates(&cursor), &alice())
            .is_err()
    );
    assert_eq!(server.cursor(), before);
    assert_eq!(server.snapshot(), document());
    server
        .import_authenticated(&client.export_updates(&SyncCursor::empty()), &alice())
        .unwrap();
    assert_eq!(server.snapshot(), client.snapshot());
}
