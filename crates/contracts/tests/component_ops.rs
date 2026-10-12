//! TEST DATA: component definitions must survive the fake store's sync wire format and actor undo.
use vellum_contracts::{
    fakes::{InMemoryDocStore, SequentialIds},
    samples,
};
use vellum_core::{
    ops::{self, Op},
    ports::{DocStore, SyncCursor},
};
use vellum_types::ComponentId;

#[test]
fn component_commit_sync_undo_and_redo_restore_both_page_and_definition() {
    let before = samples::document();
    let mut writer = InMemoryDocStore::new(&before, 1);
    let mut reader = InMemoryDocStore::new(&before, 2);
    let actor = samples::alice();
    ops::apply(
        &mut writer,
        &mut SequentialIds::default(),
        samples::tx(
            &actor,
            vec![Op::CreateComponent {
                id: samples::node_id("frame00001"),
                component_id: ComponentId::new("component1").unwrap(),
            }],
        ),
    )
    .unwrap();
    let created = writer.snapshot();
    reader
        .import_updates(&writer.export_updates(&SyncCursor::empty()))
        .unwrap();
    assert_eq!(reader.snapshot(), created);
    writer.undo(&actor).unwrap();
    assert_eq!(writer.snapshot(), before);
    reader
        .import_updates(&writer.export_updates(&SyncCursor::empty()))
        .unwrap();
    assert_eq!(reader.snapshot(), before);
    writer.redo(&actor).unwrap();
    assert_eq!(writer.snapshot(), created);
    reader
        .import_updates(&writer.export_updates(&SyncCursor::empty()))
        .unwrap();
    assert_eq!(reader.snapshot(), created);
}
