//! TEST DATA. The daemon's documents over fakes (guide Prompt 28): live and headless targets, one writer per
//! file, external changes as ops, self-writes ignored by hash, recovery copies.

mod common;

use std::sync::Arc;

use common::{Daemon, FakeEditor, decode, encode};
use vellum::daemon::{
    DocFile, EditorId, Engine, External, HeadlessTarget, LiveTarget, RECOVERY_INTERVAL_MS, Role,
};
use vellum_codec_json::JsonCodec;
use vellum_contracts::fakes::{InMemoryDocStore, ManualClock, MemFs, SequentialIds};
use vellum_contracts::samples::{self, alice, insert_at, node, rename, tx};
use vellum_contracts::suites;
use vellum_core::doc_api::NoServices;
use vellum_core::ports::{DocumentTarget, FileSystem, TargetError, TargetMode};
use vellum_types::{Actor, Document, FileReason};

const APP: &str = "designs/app.vellum";

fn name_of(doc: &Document, id: &str) -> String {
    samples::find(doc, id).expect("node").name().to_owned()
}

fn renamed(doc: &Document, id: &str, name: &str) -> Document {
    let mut store = InMemoryDocStore::new(doc, 9);
    vellum_core::ops::apply(
        &mut store,
        &mut SequentialIds::new(),
        tx(&alice(), vec![rename(id, name)]),
    )
    .expect("rename");
    vellum_core::ports::DocStore::snapshot(&store)
}

fn editor(id: &str) -> EditorId {
    EditorId(id.into())
}

fn headless(fs: Arc<MemFs>, doc: &Document) -> HeadlessTarget {
    fs.seed(APP, &encode(doc));
    let file = DocFile {
        fs,
        path: APP.into(),
        codec: Arc::new(JsonCodec::new()),
    };
    HeadlessTarget::open(file, |doc| Engine {
        store: Box::new(InMemoryDocStore::new(doc, 1)),
        ids: Box::new(SequentialIds::new()),
        services: std::rc::Rc::new(NoServices),
        clock: Arc::new(ManualClock::new(0)),
    })
    .expect("open")
    .0
}

#[test]
fn headless_target_passes_the_document_target_contract() {
    suites::target::run(|doc| headless(Arc::new(MemFs::new()), doc));
}

#[test]
fn live_target_passes_the_document_target_contract() {
    suites::target::run(|doc| LiveTarget::new(editor("e1"), FakeEditor::new(doc).link()));
}

#[test]
fn headless_target_saves_every_transaction_to_the_file() {
    let fs = Arc::new(MemFs::new());
    let mut target = headless(fs.clone(), &samples::document());
    target
        .transact(tx(&alice(), vec![rename("rect000003", "Saved")]))
        .expect("apply");
    assert_eq!(
        name_of(&decode(&fs.read(APP).unwrap()), "rect000003"),
        "Saved"
    );
    assert!(!target.is_dirty().unwrap());
}

#[test]
fn headless_target_keeps_a_change_it_could_not_save_and_retries() {
    let fs = Arc::new(MemFs::new());
    let mut target = headless(fs.clone(), &samples::document());
    fs.crash_after(0);
    let applied = target.transact(tx(&alice(), vec![rename("rect000003", "Later")]));
    assert!(
        applied.is_ok(),
        "the change is applied even when the disk refuses it"
    );
    assert!(
        target.is_dirty().unwrap(),
        "an unsaved change is dirty (save_failed)"
    );
    assert!(matches!(target.save(), Err(TargetError::File(_))));
}

#[test]
fn headless_target_saves_a_folder_document_as_a_folder() {
    let fs = Arc::new(MemFs::new());
    let codec = JsonCodec::new();
    let doc = samples::document();
    vellum_core::ports::DocumentFiles::write(
        &vellum_codec_json_dir::DirCodec::new(&codec),
        &*fs,
        "app.vellum",
        &doc,
    )
    .unwrap();
    let file = DocFile {
        fs: fs.clone(),
        path: "app.vellum".into(),
        codec: Arc::new(JsonCodec::new()),
    };
    assert!(file.is_folder());
    let (mut target, _) = HeadlessTarget::open(file.clone(), |doc| Engine {
        store: Box::new(InMemoryDocStore::new(doc, 1)),
        ids: Box::new(SequentialIds::new()),
        services: std::rc::Rc::new(NoServices),
        clock: Arc::new(ManualClock::new(0)),
    })
    .unwrap();
    target
        .transact(tx(&alice(), vec![rename("rect000001", "In a folder")]))
        .unwrap();
    assert!(file.is_folder(), "the save kept the directory format");
    assert_eq!(
        name_of(&file.read().unwrap().document, "rect000001"),
        "In a folder"
    );
}

#[test]
fn headless_render_without_a_renderer_says_so() {
    let mut target = headless(Arc::new(MemFs::new()), &samples::document());
    assert!(matches!(
        target.render("page00001", 1.),
        Err(TargetError::Render(_))
    ));
}

#[test]
fn a_tool_opens_a_file_headless_when_no_editor_holds_it() {
    let mut d = Daemon::new(&samples::document(), &[APP]);
    let target = d.docs.target(APP).expect("open headless");
    assert_eq!(target.mode(), TargetMode::Headless);
    target
        .transact(tx(&alice(), vec![rename("rect000003", "Agent")]))
        .unwrap();
    assert_eq!(name_of(&d.on_disk(APP), "rect000003"), "Agent");
    assert_eq!(d.watcher.watched(), vec![APP.to_owned()]);
}

#[test]
fn the_first_editor_is_the_writer_and_tools_forward_to_its_engine() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::new(&doc);
    let opened = d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    assert_eq!(opened.role, Role::Writer);
    assert_eq!(opened.document, doc);

    let target = d.docs.target(APP).unwrap();
    assert_eq!(target.mode(), TargetMode::Live);
    target
        .transact(tx(&alice(), vec![rename("rect000003", "Live")]))
        .unwrap();
    assert_eq!(
        name_of(&first.doc(), "rect000003"),
        "Live",
        "the editor's engine applied it"
    );
    assert_eq!(
        d.on_disk(APP),
        doc,
        "a live target never writes the file; the editor saves"
    );
    assert_eq!(
        d.docs.target(APP).unwrap().render("page00001", 1.).unwrap(),
        vellum_core::ports::ScriptValue::Text("editor drew page00001".into())
    );
}

#[test]
fn a_second_editor_attaches_to_the_writer_and_follows_it() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let (first, second) = (FakeEditor::new(&doc), FakeEditor::new(&doc));
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    first.edit(tx(&alice(), vec![rename("rect000001", "Unsaved")]));
    let opened = d
        .docs
        .open_editor(APP, editor("e2"), second.link())
        .unwrap();
    assert_eq!(opened.role, Role::Attached);
    assert_eq!(
        name_of(&opened.document, "rect000001"),
        "Unsaved",
        "an attached editor shows the writer's engine, unsaved changes included"
    );
    assert_eq!(d.docs.writer(APP), Some(&editor("e1")));

    d.docs
        .target(APP)
        .unwrap()
        .transact(tx(&alice(), vec![rename("rect000002", "Shared")]))
        .unwrap();
    assert_eq!(second.0.borrow().followed, 1);
    assert_eq!(name_of(&second.doc(), "rect000002"), "Shared");
    assert_eq!(first.0.borrow().actors.len(), 1, "only the writer applies");

    let again = d
        .docs
        .open_editor(APP, editor("e2"), second.link())
        .unwrap();
    assert_eq!(again.role, Role::Attached, "a reload keeps its role");
}

#[test]
fn when_the_writer_closes_the_next_editor_takes_over() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let (first, second) = (FakeEditor::new(&doc), FakeEditor::new(&doc));
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    d.docs
        .open_editor(APP, editor("e2"), second.link())
        .unwrap();
    d.docs.close_editor(APP, &editor("e1"));
    assert_eq!(d.docs.writer(APP), Some(&editor("e2")));
    assert!(second.0.borrow().promoted);
    d.docs
        .target(APP)
        .unwrap()
        .transact(tx(&alice(), vec![rename("rect000002", "Second")]))
        .unwrap();
    assert_eq!(name_of(&second.doc(), "rect000002"), "Second");
}

#[test]
fn when_the_last_editor_closes_the_file_is_headless_again() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::new(&doc);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    first.edit(tx(&alice(), vec![rename("rect000001", "Discarded")]));
    d.docs.close_editor(APP, &editor("e1"));
    assert!(d.docs.paths().is_empty());
    assert!(d.watcher.watched().is_empty());
    let target = d.docs.target(APP).unwrap();
    assert_eq!(target.mode(), TargetMode::Headless);
    assert_eq!(
        target.snapshot().unwrap(),
        doc,
        "closed without saving: the file is the truth"
    );
}

#[test]
fn a_writer_that_stops_answering_hands_back_to_the_file() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::new(&doc);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    first.0.borrow_mut().gone = true;
    let result = d
        .docs
        .target(APP)
        .unwrap()
        .transact(tx(&alice(), vec![rename("rect000001", "Lost")]));
    assert_eq!(result, Err(TargetError::Gone));
    let target = d.docs.target(APP).unwrap();
    assert_eq!(target.mode(), TargetMode::Headless);
    assert_eq!(target.snapshot().unwrap(), doc);
}

#[test]
fn offline_editors_open_read_only() {
    assert_eq!(Role::OFFLINE, Role::ReadOnly);
}

#[test]
fn an_external_change_is_applied_as_ops_by_the_file() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::new(&doc);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    let theirs = renamed(&doc, "rect000003", "From git");
    d.write_outside(APP, &theirs);
    let report = d.docs.poll();
    assert!(
        matches!(&report[..], [(p, External::Applied { conflicts: 0, .. })] if p == APP),
        "{report:?}"
    );
    assert_eq!(first.doc(), theirs, "the open editor updated in place");
    assert_eq!(
        first.0.borrow().actors,
        vec![Actor::File {
            reason: FileReason::ExternalChange
        }]
    );
    assert!(
        !vellum::daemon::EditorLink::is_dirty(&mut first.clone()).unwrap(),
        "a clean editor stays clean"
    );
    d.watcher.touch(APP);
    assert_eq!(
        d.docs.poll(),
        vec![(APP.to_owned(), External::Unchanged)],
        "applied once"
    );
}

#[test]
fn an_external_change_reaches_a_headless_file_without_writing_it_back() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    d.docs.target(APP).unwrap();
    let theirs = renamed(&doc, "rect000003", "Checked out");
    // Not the canonical bytes: a write back would be visible.
    let mut bytes = encode(&theirs);
    bytes.extend_from_slice(b"\n\n");
    d.fs.write(APP, &bytes).unwrap();
    d.watcher.touch(APP);
    assert!(matches!(
        &d.docs.poll()[..],
        [(_, External::Applied { .. })]
    ));
    assert_eq!(d.docs.target(APP).unwrap().snapshot().unwrap(), theirs);
    assert_eq!(
        d.fs.read(APP).unwrap(),
        bytes,
        "the daemon did not rewrite the file"
    );
}

#[test]
fn the_daemons_own_saves_are_ignored_by_hash() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    d.docs
        .target(APP)
        .unwrap()
        .transact(tx(&alice(), vec![rename("rect000003", "Mine")]))
        .unwrap();
    d.watcher.touch(APP);
    assert_eq!(d.docs.poll(), vec![(APP.to_owned(), External::Unchanged)]);
}

#[test]
fn an_editors_saves_are_ignored_by_hash() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::saving_to(&doc, d.fs.clone(), APP);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    first.edit(tx(
        &alice(),
        vec![rename("rect000001", "Saved by the editor")],
    ));
    d.docs.save(APP).unwrap();
    // The person keeps editing after the save; the watcher then reports the save.
    first.edit(tx(&alice(), vec![rename("rect000002", "After the save")]));
    let asked = first.0.borrow().snapshots;
    d.watcher.touch(APP);
    assert_eq!(d.docs.poll(), vec![(APP.to_owned(), External::Unchanged)]);
    assert!(
        first.0.borrow().actors.is_empty(),
        "nothing was applied back to the editor"
    );
    assert_eq!(
        first.0.borrow().snapshots,
        asked,
        "a self-write is recognised by its hash, without a round trip to the editor"
    );
}

#[test]
fn an_external_change_keeps_unsaved_edits() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::new(&doc);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    first.edit(tx(&alice(), vec![rename("rect000001", "Unsaved")]));
    d.write_outside(APP, &renamed(&doc, "rect000003", "Outside"));
    assert!(matches!(
        &d.docs.poll()[..],
        [(_, External::Applied { conflicts: 0, .. })]
    ));
    let now = first.doc();
    assert_eq!(name_of(&now, "rect000001"), "Unsaved");
    assert_eq!(name_of(&now, "rect000003"), "Outside");
}

#[test]
fn the_same_property_changed_on_both_sides_is_a_conflict() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::new(&doc);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    first.edit(tx(&alice(), vec![rename("rect000001", "Ours")]));
    d.write_outside(APP, &renamed(&doc, "rect000001", "Theirs"));
    let report = d.docs.poll();
    assert!(
        matches!(&report[..], [(_, External::Applied { conflicts, .. })] if *conflicts >= 1),
        "{report:?}"
    );
}

#[test]
fn an_unreadable_file_applies_nothing_until_it_is_fixed() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    d.docs.target(APP).unwrap();
    d.fs.write(APP, b"<<<<<<< HEAD\n{}\n=======\n{}\n>>>>>>> branch\n")
        .unwrap();
    d.watcher.touch(APP);
    assert!(matches!(&d.docs.poll()[..], [(_, External::Unreadable(_))]));
    assert_eq!(d.docs.target(APP).unwrap().snapshot().unwrap(), doc);
    let fixed = renamed(&doc, "rect000002", "Resolved");
    d.write_outside(APP, &fixed);
    assert!(matches!(
        &d.docs.poll()[..],
        [(_, External::Applied { .. })]
    ));
    assert_eq!(d.docs.target(APP).unwrap().snapshot().unwrap(), fixed);
}

#[test]
fn an_atomic_save_by_rename_is_an_external_change() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    d.docs.target(APP).unwrap();
    let theirs = renamed(&doc, "rect000002", "Renamed in");
    d.fs.write("designs/.app.vellum.tmp", &encode(&theirs))
        .unwrap();
    d.fs.rename("designs/.app.vellum.tmp", APP).unwrap();
    d.watcher.touch("designs/.app.vellum.tmp");
    d.watcher.touch(APP);
    assert!(matches!(
        &d.docs.poll()[..],
        [(_, External::Applied { .. })]
    ));
    assert_eq!(d.docs.target(APP).unwrap().snapshot().unwrap(), theirs);
}

#[test]
fn a_deleted_file_is_reported_and_the_document_stays_open() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    d.docs.target(APP).unwrap();
    d.fs.remove_file(APP).unwrap();
    d.watcher.touch(APP);
    assert_eq!(d.docs.poll(), vec![(APP.to_owned(), External::Missing)]);
    assert_eq!(d.docs.target(APP).unwrap().snapshot().unwrap(), doc);
}

#[test]
fn an_interrupted_folder_save_applies_nothing() {
    let doc = samples::document();
    let fs = Arc::new(MemFs::new());
    let codec = JsonCodec::new();
    vellum_core::ports::DocumentFiles::write(
        &vellum_codec_json_dir::DirCodec::new(&codec),
        &*fs,
        APP,
        &doc,
    )
    .unwrap();
    let mut d = Daemon::new(&doc, &[]);
    d.fs = fs.clone();
    d.docs = vellum::daemon::Documents::new(
        common::env(fs.clone(), d.clock.clone()),
        Box::new(d.watcher.clone()),
        vellum::daemon::Recovery::new(d.recovery.clone(), Arc::new(JsonCodec::new()), common::hash),
    );
    d.docs.target(APP).unwrap();
    fs.write(&format!("{APP}/{}", vellum_codec_json_dir::SAVING), b"")
        .unwrap();
    d.watcher.touch(&format!("{APP}/index.json"));
    assert_eq!(d.docs.poll(), vec![(APP.to_owned(), External::Interrupted)]);
}

#[test]
fn unsaved_changes_are_copied_every_few_seconds_and_removed_on_save() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::saving_to(&doc, d.fs.clone(), APP);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    assert!(d.docs.tick().is_empty(), "nothing unsaved, nothing copied");

    first.edit(tx(&alice(), vec![rename("rect000001", "Unsaved")]));
    d.clock.advance(RECOVERY_INTERVAL_MS - 1);
    assert!(
        d.docs.tick().is_empty(),
        "the interval has not passed since the last look"
    );
    d.clock.advance(1);
    assert_eq!(d.docs.tick(), vec![APP.to_owned()]);
    let (copies, damaged) = d.docs.recoveries();
    assert!(damaged.is_empty());
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].path, APP);
    assert_eq!(
        copies[0].saved_at,
        vellum_core::ports::Clock::now(&*d.clock)
    );
    assert_eq!(name_of(&copies[0].document, "rect000001"), "Unsaved");
    assert!(
        d.recovery
            .list("")
            .unwrap()
            .iter()
            .any(|n| n.ends_with(".vellum.recovery")),
        "the copy matches the *.vellum.recovery ignore pattern"
    );

    d.clock.advance(1_000);
    assert!(d.docs.tick().is_empty(), "not again within the interval");

    d.docs.save(APP).unwrap();
    assert!(d.docs.recoveries().0.is_empty(), "a save removes the copy");
}

#[test]
fn a_crashed_sessions_copy_is_listed_on_open_and_can_be_restored() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::new(&doc);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    first.edit(tx(
        &alice(),
        vec![insert_at("page00002", None, 0, node("rect", "rect000009"))],
    ));
    d.clock.advance(RECOVERY_INTERVAL_MS);
    assert_eq!(d.docs.tick(), vec![APP.to_owned()]);
    first.0.borrow_mut().gone = true; // the app quit unexpectedly

    let second = FakeEditor::saving_to(&doc, d.fs.clone(), APP);
    let opened = d
        .docs
        .open_editor(APP, editor("e2"), second.link())
        .unwrap();
    assert_eq!(opened.role, Role::Writer);
    assert_eq!(opened.document, doc, "the file opens as it is on disk");
    let copy = opened.recovery.expect("the copy is offered on open (A06)");
    assert!(samples::find(&copy.document, "rect000009").is_some());

    d.docs.restore(APP).unwrap();
    assert!(samples::find(&second.doc(), "rect000009").is_some());
    assert_eq!(
        second.0.borrow().actors,
        vec![Actor::File {
            reason: FileReason::Recovery
        }]
    );
    assert!(d.docs.recoveries().0.len() == 1, "kept until saved");
    d.docs.save(APP).unwrap();
    assert!(d.docs.recoveries().0.is_empty());
}

#[test]
fn discarding_a_backup_or_closing_on_purpose_removes_the_copy() {
    let doc = samples::document();
    let mut d = Daemon::new(&doc, &[APP]);
    let first = FakeEditor::new(&doc);
    d.docs.open_editor(APP, editor("e1"), first.link()).unwrap();
    first.edit(tx(&alice(), vec![rename("rect000001", "Unsaved")]));
    d.clock.advance(RECOVERY_INTERVAL_MS);
    d.docs.tick();
    d.docs.discard_recovery(APP).unwrap();
    assert!(d.docs.recoveries().0.is_empty());

    d.clock.advance(RECOVERY_INTERVAL_MS);
    assert_eq!(d.docs.tick().len(), 1);
    d.docs.close_editor(APP, &editor("e1"));
    assert!(d.docs.recoveries().0.is_empty());
}

#[test]
fn a_damaged_copy_is_listed_not_dropped() {
    let d = Daemon::new(&samples::document(), &[APP]);
    d.recovery
        .write("0123.json", b"{\"path\":\"x\",\"savedAt\":1}")
        .unwrap();
    d.recovery
        .write("0123.vellum.recovery", b"not json")
        .unwrap();
    let (copies, damaged) = d.docs.recoveries();
    assert!(copies.is_empty());
    assert_eq!(damaged.len(), 1);
    assert_eq!(damaged[0].key, "0123");
}
