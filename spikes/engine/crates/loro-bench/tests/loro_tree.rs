//! The spike document as a Loro movable tree (guide Prompt 5, ADR-004). No lane: plain Rust.

use loro::{ExportMode, LoroDoc, UndoManager};
use spike_loro::{BATCH_SIZE, apply_batch, from_document, load, node_count, save, to_json};
use spike_scene::generate;

#[test]
fn every_scene_node_becomes_one_tree_node() {
    let scene = generate(1_000);
    let doc = from_document(&scene);
    assert_eq!(node_count(&doc), scene.node_count());
}

#[test]
fn building_is_deterministic() {
    let scene = generate(1_000);
    assert_eq!(save(&from_document(&scene)), save(&from_document(&scene)));
}

#[test]
fn a_snapshot_round_trips() {
    let doc = from_document(&generate(1_000));
    let loaded = load(&save(&doc)).expect("load");
    assert_eq!(loaded.get_deep_value(), doc.get_deep_value());
    assert_eq!(to_json(&loaded), to_json(&doc));
}

#[test]
fn a_batch_changes_exactly_batch_size_nodes_in_one_commit() {
    let doc = from_document(&generate(1_000));
    let before = spike_loro::node_states(&doc);
    let frontiers = doc.oplog_frontiers();
    apply_batch(&doc, 7);
    let after = spike_loro::node_states(&doc);
    assert_eq!(BATCH_SIZE, 200);
    let changed = before
        .iter()
        .filter(|(id, state)| after.get(*id) != Some(*state))
        .count();
    assert_eq!(changed, BATCH_SIZE);
    assert_eq!(node_count(&doc), 1_000);
    // One transaction: one change since the frontiers taken before the batch.
    let update = doc.export(ExportMode::updates(&doc.state_vv())).unwrap();
    assert!(!update.is_empty());
    assert_ne!(doc.oplog_frontiers(), frontiers);
}

#[test]
fn a_remote_peer_converges_by_importing_the_batch_update() {
    let local = from_document(&generate(1_000));
    let remote = load(&save(&local)).expect("load");
    remote.set_peer_id(2).unwrap();
    let vv = local.oplog_vv();
    apply_batch(&local, 3);
    let update = local.export(ExportMode::updates(&vv)).unwrap();
    remote.import(&update).expect("import");
    assert_eq!(remote.get_deep_value(), local.get_deep_value());
}

#[test]
fn undo_reverts_a_whole_batch() {
    let doc: LoroDoc = from_document(&generate(1_000));
    let before = doc.get_deep_value();
    let mut undo = UndoManager::new(&doc);
    apply_batch(&doc, 5);
    assert_ne!(doc.get_deep_value(), before);
    assert!(undo.undo().unwrap());
    assert_eq!(doc.get_deep_value(), before);
}
