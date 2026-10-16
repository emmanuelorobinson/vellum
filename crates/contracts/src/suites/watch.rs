//! `FileWatcher`.

use vellum_core::ports::FileWatcher;

/// A disk the suite can change, and a way to wait for a watcher to notice. A real disk sleeps between polls; a
/// fake one has nothing to wait for.
pub trait WatchDisk {
    /// The native path of `name`, a relative name inside the disk's scratch folder.
    fn path(&self, name: &str) -> String;
    /// Creates or overwrites a file in place.
    fn write(&mut self, name: &str, bytes: &[u8]);
    /// Writes `bytes` to a temporary file beside `name` and renames it over `name`: an atomic save.
    fn replace(&mut self, name: &str, bytes: &[u8]);
    /// Makes a folder.
    fn mkdir(&mut self, name: &str);
    /// Waits a little, so events can arrive.
    fn pause(&mut self);
    /// How many pauses to wait for an event before giving up.
    fn patience(&self) -> usize;
    /// How many pauses to wait before deciding nothing more is coming.
    fn quiet(&self) -> usize;
}

fn settle(watcher: &mut dyn FileWatcher, disk: &mut dyn WatchDisk) -> Vec<String> {
    for _ in 0..disk.patience() {
        disk.pause();
        let mut changed = watcher.changed();
        if !changed.is_empty() {
            // Let the rest of the burst and any late duplicates arrive; a document is reported once.
            for _ in 0..disk.quiet() {
                disk.pause();
                for path in watcher.changed() {
                    assert!(
                        !changed.contains(&path),
                        "FileWatcher: one burst of writes must be reported once, {path} came twice"
                    );
                    changed.push(path);
                }
            }
            changed.sort();
            return changed;
        }
    }
    Vec::new()
}

fn nothing(watcher: &mut dyn FileWatcher, disk: &mut dyn WatchDisk, why: &str) {
    for _ in 0..disk.quiet() {
        disk.pause();
        let changed = watcher.changed();
        assert!(changed.is_empty(), "FileWatcher: {why}, got {changed:?}");
    }
}

/// Watched documents are reported once per burst of writes, by the name they were watched under, whether they
/// were written in place, replaced by a rename or (as a folder) had a file inside them change. Unwatched files are
/// never reported.
pub fn run(watcher: &mut dyn FileWatcher, disk: &mut dyn WatchDisk) {
    disk.write("one.vellum", b"1");
    disk.write("other.vellum", b"1");
    disk.mkdir("folder.vellum");
    disk.mkdir("folder.vellum/pages");
    disk.write("folder.vellum/index.json", b"1");
    let (one, folder) = (disk.path("one.vellum"), disk.path("folder.vellum"));
    watcher.watch(&one).expect("watch a file");
    watcher.watch(&one).expect("watching twice is fine");
    watcher.watch(&folder).expect("watch a folder");
    // Whatever creating the files above produced is not part of the contract.
    for _ in 0..disk.quiet() {
        disk.pause();
        let _ = watcher.changed();
    }

    disk.write("one.vellum", b"2");
    disk.write("one.vellum", b"3");
    assert_eq!(
        settle(watcher, disk),
        vec![one.clone()],
        "FileWatcher: writes to a watched file must be reported, once, by its watched name"
    );
    nothing(watcher, disk, "nothing changed since the last report");

    disk.write("other.vellum", b"2");
    nothing(watcher, disk, "an unwatched file must never be reported");

    disk.replace("one.vellum", b"4");
    assert_eq!(
        settle(watcher, disk),
        vec![one.clone()],
        "FileWatcher: a temporary file renamed over a watched file is a change to it"
    );
    disk.replace("one.vellum", b"5");
    assert_eq!(
        settle(watcher, disk),
        vec![one.clone()],
        "FileWatcher: the watch must follow the name after a rename, not the replaced file"
    );

    disk.write("folder.vellum/pages/p.json", b"1");
    assert_eq!(
        settle(watcher, disk),
        vec![folder.clone()],
        "FileWatcher: a file changing inside a watched folder is a change to the folder document"
    );

    disk.write("one.vellum", b"6");
    disk.write("folder.vellum/index.json", b"2");
    let mut both = vec![one.clone(), folder.clone()];
    both.sort();
    assert_eq!(
        settle(watcher, disk),
        both,
        "FileWatcher: changes to two documents must be reported together, sorted"
    );

    watcher.unwatch(&one);
    watcher.unwatch(&one);
    disk.write("one.vellum", b"7");
    nothing(watcher, disk, "an unwatched document must not be reported");
}

/// A [`WatchDisk`] for watchers that hear only what they are told ([`crate::fakes::ManualWatcher`]): a write
/// touches the path, nothing needs waiting for.
pub struct ToldDisk(pub crate::fakes::ManualWatcher);

impl WatchDisk for ToldDisk {
    fn path(&self, name: &str) -> String {
        format!("/told/{name}")
    }
    fn write(&mut self, name: &str, _bytes: &[u8]) {
        self.0.touch(&self.path(name));
    }
    fn replace(&mut self, name: &str, _bytes: &[u8]) {
        self.0.touch(&self.path(&format!("{name}.tmp")));
        self.0.touch(&self.path(name));
    }
    fn mkdir(&mut self, name: &str) {
        self.0.touch(&self.path(name));
    }
    fn pause(&mut self) {}
    fn patience(&self) -> usize {
        1
    }
    fn quiet(&self) -> usize {
        1
    }
}
