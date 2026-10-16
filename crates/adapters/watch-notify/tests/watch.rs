//! The `FileWatcher` contract on the real disk with native events (guide Prompt 28).

use std::path::PathBuf;
use std::time::Duration;

use vellum_contracts::suites::watch::{self, WatchDisk};
use vellum_core::ports::FileWatcher;
use vellum_watch_notify::NotifyWatcher;

/// A scratch folder under the system temp dir, removed afterwards.
struct Disk(PathBuf);

impl Disk {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("vellum-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("make the folder");
        Self(path)
    }
}

impl Drop for Disk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl WatchDisk for Disk {
    fn path(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
    fn write(&mut self, name: &str, bytes: &[u8]) {
        std::fs::write(self.0.join(name), bytes).expect("write");
    }
    fn replace(&mut self, name: &str, bytes: &[u8]) {
        let temp = self.0.join(format!(".{name}.tmp"));
        std::fs::write(&temp, bytes).expect("write the temporary file");
        std::fs::rename(&temp, self.0.join(name)).expect("rename over");
    }
    fn mkdir(&mut self, name: &str) {
        std::fs::create_dir_all(self.0.join(name)).expect("mkdir");
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(50));
    }
    fn patience(&self) -> usize {
        // Ten seconds: FSEvents and a loaded CI machine can both be slow.
        200
    }
    fn quiet(&self) -> usize {
        // Longer than the debounce, so anything still coming has come.
        10
    }
}

#[test]
fn notify_watcher_passes_the_contract_on_a_real_disk() {
    let mut disk = Disk::new("contract");
    let mut watcher = NotifyWatcher::new(Duration::from_millis(100)).expect("start the watcher");
    watch::run(&mut watcher, &mut disk);
}

#[test]
fn a_document_that_appears_by_rename_is_reported() {
    let mut disk = Disk::new("appears");
    let mut watcher = NotifyWatcher::new(Duration::from_millis(100)).expect("start the watcher");
    let path = disk.path("later.vellum");
    watcher
        .watch(&path)
        .expect("watch a file that is not there yet");
    disk.replace("later.vellum", b"{}");
    let mut seen = Vec::new();
    for _ in 0..disk.patience() {
        disk.pause();
        seen = vellum_core::ports::FileWatcher::changed(&mut watcher);
        if !seen.is_empty() {
            break;
        }
    }
    assert_eq!(seen, vec![path]);
}

#[test]
fn watching_inside_a_missing_folder_is_an_error() {
    let disk = Disk::new("missing");
    let mut watcher = NotifyWatcher::new(Duration::from_millis(100)).expect("start the watcher");
    assert!(
        vellum_core::ports::FileWatcher::watch(&mut watcher, &disk.path("nope/app.vellum"))
            .is_err()
    );
}
