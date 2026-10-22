//! TEST DATA. Live editors as replicas of the daemon's engine (ADR-033): an editor starts from the daemon's
//! genesis document and its updates, pushes its person's edits and pulls agents' edits as sync updates. The
//! daemon's target stays the one writer, stops saving on its own while an editor holds the file, and leaves
//! the file to the editor's Save.

mod common;

use common::{Daemon, decode, encode};
use vellum::daemon::{EditorId, External, RECOVERY_INTERVAL_MS, Role};
use vellum_contracts::fakes::{InMemoryDocStore, SequentialIds};
use vellum_contracts::samples::{self, alice, bob, rename, tx};
use vellum_core::ops;
use vellum_core::ports::{DocStore, FileSystem, TargetError, TargetMode};
use vellum_types::{Actor, Document, Origin};

const APP: &str = "designs/app.vellum";

fn agent() -> Actor {
    Actor::Agent {
        agent: samples::teal(),
    }
}

fn name_of(doc: &Document, id: &str) -> String {
    samples::find(doc, id).expect("node").name().to_owned()
}

/// The editor's side: an engine built from what the daemon handed it.
struct Replica {
    store: InMemoryDocStore,
    actor: Actor,
}

impl Replica {
    fn start(daemon: &mut Daemon, editor: &str, actor: Actor, peer: u64) -> (Self, Role) {
        let (start, _recovery) = daemon
            .docs
            .attach_editor(APP, EditorId(editor.into()), actor.clone())
            .expect("attach");
        let mut store = InMemoryDocStore::new(&start.genesis, peer);
        store.import_updates(&start.updates).expect("catch up");
        (Self { store, actor }, start.role)
    }

    fn edit(&mut self, id: &str, name: &str) -> vellum_core::ports::SyncPayload {
        let before = self.store.cursor();
        ops::apply(
            &mut self.store,
            &mut SequentialIds::new(),
            ops::Transaction::new(Origin::new(self.actor.clone()), vec![rename(id, name)]),
        )
        .expect("person edit");
        self.store.export_updates(&before)
    }

    fn pull(&mut self, daemon: &mut Daemon) {
        let updates = daemon
            .docs
            .editor_pull(APP, &self.store.cursor())
            .expect("pull");
        self.store.import_updates(&updates).expect("import");
    }
}

#[test]
fn an_editor_starts_from_the_daemons_engine_and_the_first_one_is_the_writer() {
    let mut daemon = Daemon::new(&samples::document(), &[APP]);
    daemon
        .docs
        .target(APP)
        .unwrap()
        .transact(tx(
            &agent(),
            vec![rename("rect000003", "Before the editor")],
        ))
        .unwrap();
    let (first, role) = Replica::start(&mut daemon, "e1", alice(), 11);
    assert_eq!(role, Role::Writer);
    assert_eq!(
        first.store.snapshot(),
        daemon.docs.target(APP).unwrap().snapshot().unwrap()
    );
    let (second, role) = Replica::start(&mut daemon, "e2", bob(), 12);
    assert_eq!(
        role,
        Role::Attached,
        "a second editor joins the same engine"
    );
    assert_eq!(second.store.snapshot(), first.store.snapshot());
    assert_eq!(
        daemon.docs.target(APP).unwrap().mode(),
        TargetMode::Live,
        "agents see that a person has the file open"
    );
}

#[test]
fn agent_edits_wait_for_the_editors_save_and_reach_it_as_updates() {
    let mut daemon = Daemon::new(&samples::document(), &[APP]);
    let (mut editor, _) = Replica::start(&mut daemon, "e1", alice(), 11);
    daemon
        .docs
        .target(APP)
        .unwrap()
        .transact(tx(&agent(), vec![rename("rect000003", "By an agent")]))
        .unwrap();
    assert_eq!(
        name_of(&daemon.on_disk(APP), "rect000003"),
        name_of(&samples::document(), "rect000003"),
        "the editor saves, not the daemon"
    );
    assert!(daemon.docs.target(APP).unwrap().is_dirty().unwrap());
    editor.pull(&mut daemon);
    assert_eq!(
        name_of(&editor.store.snapshot(), "rect000003"),
        "By an agent"
    );
    let nothing = daemon
        .docs
        .editor_pull(APP, &editor.store.cursor())
        .unwrap();
    editor.store.import_updates(&nothing).unwrap();
    assert_eq!(
        name_of(&editor.store.snapshot(), "rect000003"),
        "By an agent"
    );
}

#[test]
fn a_person_edit_reaches_the_daemon_and_only_as_that_person() {
    let mut daemon = Daemon::new(&samples::document(), &[APP]);
    let (mut editor, _) = Replica::start(&mut daemon, "e1", alice(), 11);
    let (mut other, _) = Replica::start(&mut daemon, "e2", bob(), 12);
    let update = editor.edit("rect000001", "By a person");
    let changes = daemon
        .docs
        .editor_push(APP, &EditorId("e1".into()), &update)
        .expect("push");
    assert!(!changes.is_empty());
    assert_eq!(
        name_of(
            &daemon.docs.target(APP).unwrap().snapshot().unwrap(),
            "rect000001"
        ),
        "By a person"
    );
    // The other editor's connection may not hand over e1's edits as its own.
    let forged = other.edit("rect000002", "Forged");
    assert!(matches!(
        daemon
            .docs
            .editor_push(APP, &EditorId("e1".into()), &forged),
        Err(TargetError::Sync(_))
    ));
    other.pull(&mut daemon);
    assert_eq!(
        name_of(&other.store.snapshot(), "rect000001"),
        "By a person"
    );
    assert!(matches!(
        daemon
            .docs
            .editor_push(APP, &EditorId("nobody".into()), &update),
        Err(TargetError::Gone)
    ));
}

#[test]
fn the_editors_save_is_the_file_and_its_backup_goes() {
    let mut daemon = Daemon::new(&samples::document(), &[APP]);
    let (mut editor, _) = Replica::start(&mut daemon, "e1", alice(), 11);
    let update = editor.edit("rect000001", "Saved by the editor");
    daemon
        .docs
        .editor_push(APP, &EditorId("e1".into()), &update)
        .unwrap();
    daemon.clock.advance(RECOVERY_INTERVAL_MS);
    assert_eq!(
        daemon.docs.tick(),
        vec![APP.to_owned()],
        "unsaved work is backed up"
    );
    // The host writes the editor's canonical bytes, then tells the daemon.
    daemon
        .fs
        .write(APP, &encode(&editor.store.snapshot()))
        .unwrap();
    daemon.watcher.touch(APP);
    daemon.docs.editor_wrote(APP).expect("saved");
    assert!(!daemon.docs.target(APP).unwrap().is_dirty().unwrap());
    assert!(
        daemon.docs.recoveries().0.is_empty(),
        "a save removes the backup"
    );
    assert!(
        daemon
            .docs
            .poll()
            .iter()
            .all(|(_, outcome)| *outcome == External::Unchanged),
        "the watcher's report of the editor's own save is ignored"
    );
    assert_eq!(
        name_of(&decode(&daemon.fs.read(APP).unwrap()), "rect000001"),
        "Saved by the editor"
    );
}

#[test]
fn closing_the_last_editor_without_saving_leaves_the_file_as_it_was() {
    let mut daemon = Daemon::new(&samples::document(), &[APP]);
    let (mut editor, _) = Replica::start(&mut daemon, "e1", alice(), 11);
    let update = editor.edit("rect000001", "Discarded");
    daemon
        .docs
        .editor_push(APP, &EditorId("e1".into()), &update)
        .unwrap();
    assert!(
        !daemon.docs.close_headless(APP),
        "an editor still holds the file"
    );
    daemon.docs.detach_editor(APP, &EditorId("e1".into()));
    let doc = daemon.docs.target(APP).unwrap().snapshot().unwrap();
    assert_eq!(
        name_of(&doc, "rect000001"),
        name_of(&samples::document(), "rect000001"),
        "unsaved person edits are not kept or saved by the daemon"
    );
    // With nobody left, the daemon saves agents' work again.
    daemon
        .docs
        .target(APP)
        .unwrap()
        .transact(tx(&agent(), vec![rename("rect000003", "Headless again")]))
        .unwrap();
    assert_eq!(
        name_of(&daemon.on_disk(APP), "rect000003"),
        "Headless again"
    );
}

#[test]
fn a_clean_last_editor_hands_the_file_back_to_headless_saves() {
    let mut daemon = Daemon::new(&samples::document(), &[APP]);
    let (_editor, _) = Replica::start(&mut daemon, "e1", alice(), 11);
    daemon.docs.detach_editor(APP, &EditorId("e1".into()));
    assert_eq!(
        daemon.docs.target(APP).unwrap().mode(),
        TargetMode::Headless
    );
    assert!(daemon.docs.close_headless(APP));
}

#[test]
fn revert_recovery_and_save_go_through_the_daemons_engine_with_a_revision() {
    let mut daemon = Daemon::new(&samples::document(), &[APP]);
    let (mut editor, _) = Replica::start(&mut daemon, "e1", alice(), 11);
    let r0 = daemon.docs.editor_revision(APP).unwrap();
    let update = editor.edit("rect000001", "Unsaved");
    daemon
        .docs
        .editor_push(APP, &EditorId("e1".into()), &update)
        .unwrap();
    let r1 = daemon.docs.editor_revision(APP).unwrap();
    assert!(r1 > r0, "a person's edit moves the revision");
    // Revert: the engine goes back to the file, as one change by the file, and is clean.
    let reverted = daemon
        .docs
        .editor_replace(APP, &daemon.on_disk(APP), vellum::daemon::Replace::Revert)
        .unwrap();
    assert!(reverted > r1);
    assert!(!daemon.docs.target(APP).unwrap().is_dirty().unwrap());
    editor.pull(&mut daemon);
    assert_eq!(
        name_of(&editor.store.snapshot(), "rect000001"),
        name_of(&samples::document(), "rect000001")
    );
    // Recovery: a backup's content comes back as unsaved work.
    let mut backup = samples::document();
    backup = {
        let mut store = InMemoryDocStore::new(&backup, 9);
        ops::apply(
            &mut store,
            &mut SequentialIds::new(),
            tx(&alice(), vec![rename("rect000002", "From the backup")]),
        )
        .unwrap();
        store.snapshot()
    };
    daemon
        .docs
        .editor_replace(APP, &backup, vellum::daemon::Replace::Recovery)
        .unwrap();
    assert!(daemon.docs.target(APP).unwrap().is_dirty().unwrap());
    // Save writes the engine's document and leaves it clean.
    daemon.docs.save(APP).unwrap();
    assert_eq!(
        name_of(&daemon.on_disk(APP), "rect000002"),
        "From the backup"
    );
    assert!(!daemon.docs.target(APP).unwrap().is_dirty().unwrap());
    let (revision, doc) = daemon.docs.editor_snapshot(APP).unwrap();
    assert_eq!(doc, daemon.on_disk(APP));
    assert!(revision >= reverted);
}
