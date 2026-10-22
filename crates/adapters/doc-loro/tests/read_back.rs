//! Loro's own state always reads back as the document the store hands out: after every commit, undo, redo,
//! undone turn and import, through the contract suite, the property test, component and variable edits.

use serde_json::json;
use vellum_contracts::fakes::{FakeJsonCodec, SequentialIds};
use vellum_contracts::samples::{alice, bob, document, node_id, tx};
use vellum_contracts::suites::doc_store;
use vellum_core::ops::{self, Op};
use vellum_core::ports::{
    ChangeSet, Codec, Commit, DocStore, SyncCursor, SyncError, SyncPayload, UndoTurn,
};
use vellum_doc_loro::LoroDocStore;
use vellum_types::{Actor, ComponentId, Document, TurnId, Variable, VariableId};

/// A Loro store that checks Loro against its own copy after every change.
#[derive(Debug)]
struct Checked(LoroDocStore);

impl Checked {
    fn check<T>(&self, value: T) -> T {
        assert_eq!(
            self.0.read_back(),
            self.0.snapshot(),
            "Loro does not read back as the store's document"
        );
        value
    }
}

impl DocStore for Checked {
    fn adapter_id(&self) -> &'static str {
        self.0.adapter_id()
    }
    fn snapshot(&self) -> Document {
        self.0.snapshot()
    }
    fn apply(&mut self, commit: Commit) -> ChangeSet {
        let changes = self.0.apply(commit);
        self.check(changes)
    }
    fn undo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        let changes = self.0.undo(actor);
        self.check(changes)
    }
    fn redo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        let changes = self.0.redo(actor);
        self.check(changes)
    }
    fn undo_turn(&mut self, turn: &TurnId, by: &Actor) -> UndoTurn {
        let result = self.0.undo_turn(turn, by);
        self.check(result)
    }
    fn cursor(&self) -> SyncCursor {
        self.0.cursor()
    }
    fn export_updates(&self, since: &SyncCursor) -> SyncPayload {
        self.0.export_updates(since)
    }
    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError> {
        let result = self.0.import_updates(payload);
        self.check(result)
    }
    fn import_updates_as(
        &mut self,
        payload: &SyncPayload,
        actor: &Actor,
    ) -> Result<ChangeSet, SyncError> {
        let result = self.0.import_updates_as(payload, actor);
        self.check(result)
    }
}

fn checked(doc: &Document, peer: u64) -> Checked {
    Checked(LoroDocStore::new(doc, peer))
}

#[test]
fn contract_suite_with_loro_checked_at_every_step() {
    doc_store::run(checked);
}

#[test]
fn property_test_with_loro_checked_at_every_step() {
    doc_store::undo_is_yours(checked, 0..60);
}

fn fixture(name: &str) -> Document {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/documents")
        .join(name);
    FakeJsonCodec
        .decode(&std::fs::read(path).expect("the fixture"))
        .expect("a document")
}

#[test]
fn components_and_variables_round_trip_through_loro() {
    let before = document();
    let mut writer = checked(&before, 1);
    let mut reader = checked(&before, 2);
    let mut ids = SequentialIds::default();
    let a = alice();
    let component = ComponentId::new("component1").expect("an id");
    ops::apply(
        &mut writer,
        &mut ids,
        tx(
            &a,
            vec![Op::CreateComponent {
                id: node_id("frame00001"),
                component_id: component.clone(),
            }],
        ),
    )
    .expect("make a component");
    let variable: Variable = serde_json::from_value(json!({
        "type": "color", "name": "TEST DATA/surface", "values": [{ "value": "#FFFFFF" }]
    }))
    .expect("a variable");
    let var_id = VariableId::new("va000001").expect("an id");
    ops::apply(
        &mut writer,
        &mut ids,
        tx(
            &bob(),
            vec![Op::SetVariables {
                set: [(var_id.clone(), variable)].into(),
                remove: Default::default(),
            }],
        ),
    )
    .expect("set a variable");
    let created = writer.snapshot();
    reader
        .import_updates(&writer.export_updates(&reader.cursor()))
        .expect("import");
    assert_eq!(reader.snapshot(), created);

    writer.undo(&a).expect("undo the component");
    writer.undo(&bob()).expect("undo the variable");
    assert_eq!(writer.snapshot(), before);
    reader
        .import_updates(&writer.export_updates(&reader.cursor()))
        .expect("import");
    assert_eq!(reader.snapshot(), before);
    writer.redo(&a).expect("redo");
    reader
        .import_updates(&writer.export_updates(&reader.cursor()))
        .expect("import");
    assert_eq!(reader.snapshot(), writer.snapshot());
}

#[test]
fn fixture_documents_with_components_take_edits_and_undo() {
    for name in [
        "design-system.vellum",
        "library.lib.vellum",
        "content.vellum",
    ] {
        let doc = fixture(name);
        let mut store = checked(&doc, 1);
        let mut other = checked(&doc, 2);
        // Rename every node a person can reach, then undo it all.
        let mut targets: Vec<_> = vellum_contracts::samples::all_ids(&doc);
        for c in doc.components.values() {
            targets.push(c.root.id().to_string());
        }
        for id in targets.iter().take(20) {
            ops::apply(
                &mut store,
                &mut SequentialIds::default(),
                tx(
                    &alice(),
                    vec![vellum_contracts::samples::rename(id, "TEST DATA renamed")],
                ),
            )
            .unwrap_or_else(|e| panic!("{name}: rename {id}: {e}"));
        }
        other
            .import_updates(&store.export_updates(&other.cursor()))
            .expect("import");
        assert_eq!(other.snapshot(), store.snapshot(), "{name}");
        while store.undo(&alice()).is_some() {}
        assert_eq!(
            store.snapshot(),
            doc,
            "{name}: undo did not restore the document"
        );
    }
}
