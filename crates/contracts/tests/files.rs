//! The `FileSystem` port, its in-memory fake with crash simulation, and `atomic_write` (guide Prompt 24).
//!
//! The simulation is pessimistic the way a real disk is: a file whose bytes were written but not synced comes
//! back from a crash with only part of them. `atomic_write` has to survive that at every step.

use vellum_contracts::fakes::MemFs;
use vellum_contracts::suites;
use vellum_core::files::{atomic_write, recover_temps};
use vellum_core::ports::{FileSystem, FsError};

#[test]
fn the_fake_passes_the_file_system_suite() {
    suites::files::run(&MemFs::new());
}

#[test]
fn atomic_write_creates_replaces_and_makes_the_folder() {
    let fs = MemFs::new();
    atomic_write(&fs, "app.assets/deep/file.bin", b"one").expect("create");
    assert_eq!(fs.read("app.assets/deep/file.bin").unwrap(), b"one");
    atomic_write(&fs, "app.assets/deep/file.bin", b"two, longer").expect("replace");
    assert_eq!(fs.read("app.assets/deep/file.bin").unwrap(), b"two, longer");
    assert_eq!(
        fs.list("app.assets/deep").unwrap(),
        vec!["file.bin".to_owned()],
        "a temporary file was left beside the target"
    );
}

#[test]
fn atomic_write_refuses_paths_outside_the_root() {
    let fs = MemFs::new();
    for bad in ["../x", "/x", "a//b", "a\\b", ""] {
        assert!(
            matches!(atomic_write(&fs, bad, b"x"), Err(FsError::InvalidPath(_))),
            "{bad:?} was accepted"
        );
    }
}

/// Every step of an overwrite can be the last one a crash lets through. Whatever is on the disk afterwards,
/// the target is the old file or the new file, whole. Never half of either.
#[test]
fn a_crash_at_any_step_leaves_the_old_file_or_the_new_one() {
    let old = vec![b'o'; 4096];
    let new = vec![b'n'; 8192];

    // How many steps does a clean overwrite take?
    let probe = MemFs::new();
    probe.seed("doc/page.json", &old);
    let before = probe.mutations();
    atomic_write(&probe, "doc/page.json", &new).expect("clean write");
    let steps = probe.mutations() - before;
    assert!(
        steps >= 4,
        "write, sync, rename and sync dir are four steps"
    );

    for crash_at in 0..=steps {
        let fs = MemFs::new();
        fs.seed("doc/page.json", &old);
        fs.crash_after(crash_at);
        let outcome = atomic_write(&fs, "doc/page.json", &new);
        let disk = fs.after_crash();
        let found = disk.read("doc/page.json").expect("the target must survive");
        assert!(
            found == old || found == new,
            "crash after {crash_at} of {steps} steps left a half file ({} bytes) ; outcome {outcome:?}",
            found.len()
        );
        if outcome.is_ok() {
            assert_eq!(found, new, "a write that reported success must be on disk");
        }
        // Leftovers are named and removable, and removing them changes nothing else.
        let removed = recover_temps(&disk, "doc").expect("recover");
        let names = disk.list("doc").unwrap();
        assert_eq!(
            names,
            vec!["page.json".to_owned()],
            "after recovery {removed:?}"
        );
        assert_eq!(disk.read("doc/page.json").unwrap(), found);
    }
}

#[test]
fn a_crash_while_creating_a_new_file_leaves_nothing_or_the_whole_file() {
    let new = vec![b'n'; 1000];
    let probe = MemFs::new();
    atomic_write(&probe, "fresh/a.json", &new).expect("clean write");
    let steps = probe.mutations();
    for crash_at in 0..=steps {
        let fs = MemFs::new();
        fs.crash_after(crash_at);
        let _ = atomic_write(&fs, "fresh/a.json", &new);
        let disk = fs.after_crash();
        match disk.read("fresh/a.json") {
            Ok(found) => assert_eq!(found, new, "crash after {crash_at} left a half file"),
            Err(FsError::NotFound(_)) => {}
            Err(other) => panic!("unexpected {other:?}"),
        }
    }
}

/// The simulation is only worth having if it catches the classic mistake: renaming a file whose bytes were
/// never synced.
#[test]
fn the_simulation_catches_a_write_that_forgets_to_sync() {
    let fs = MemFs::new();
    fs.seed("doc/a.json", b"old old old old");
    fs.write("doc/.a.tmp", b"new new new new new new").unwrap();
    fs.rename("doc/.a.tmp", "doc/a.json").unwrap();
    let disk = fs.after_crash();
    let found = disk.read("doc/a.json").unwrap();
    assert!(
        found != b"old old old old" && found != b"new new new new new new",
        "the fake let an unsynced write through whole"
    );
}

#[test]
fn recover_temps_removes_only_our_temporary_files() {
    let fs = MemFs::new();
    fs.seed("d/keep.json", b"1");
    fs.seed("d/.keep.json.vellum-tmp", b"half");
    fs.seed("d/.hidden-but-theirs", b"2");
    let removed = recover_temps(&fs, "d").unwrap();
    assert_eq!(removed, vec![".keep.json.vellum-tmp".to_owned()]);
    assert_eq!(
        fs.list("d").unwrap(),
        vec![".hidden-but-theirs".to_owned(), "keep.json".to_owned()]
    );
}
