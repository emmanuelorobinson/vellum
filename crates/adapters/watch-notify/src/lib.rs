//! `FileWatcher` over `notify` (guide Prompt 28; ARCHITECTURE §4.5).
//!
//! A document is watched through the folder it sits in, not through the file itself: an atomic save (write a
//! temporary file, rename it over the document) and a Git checkout replace the file, and a watch on the old file
//! would go quiet. The folder watch is not recursive, so watching a design file at the top of a repository does
//! not watch `node_modules`. A folder document (the directory format) is watched recursively as well.
//!
//! Events are only ever a hint. Each burst is debounced per document, read-only access is ignored (the daemon
//! reading the file after an event must not cause another one), and the daemon decides what changed by reading
//! the file and comparing hashes, never from the event kind.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use notify::event::{AccessKind, AccessMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use vellum_core::ports::{FileWatcher, FsError};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// A watched document: the name it was watched under, and the folder watch it holds.
struct Doc {
    name: String,
    parent: PathBuf,
    recursive: bool,
}

/// Native file events, debounced per document.
pub struct NotifyWatcher {
    watcher: RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
    debounce: Duration,
    /// By canonical path (the folder resolved, so `/var` and `/private/var` agree on macOS).
    docs: BTreeMap<PathBuf, Doc>,
    /// How many documents share each folder watch.
    folders: BTreeMap<PathBuf, usize>,
    /// Documents with events not yet reported, by when the last one came.
    pending: BTreeMap<PathBuf, Instant>,
}

fn unavailable(error: impl std::fmt::Display) -> FsError {
    FsError::Unavailable(error.to_string())
}

impl NotifyWatcher {
    /// A watcher that reports a document once it has been quiet for `debounce`.
    pub fn new(debounce: Duration) -> Result<Self, FsError> {
        let (send, events) = channel();
        let watcher = notify::recommended_watcher(move |event| {
            let _ = send.send(event);
        })
        .map_err(unavailable)?;
        Ok(Self {
            watcher,
            events,
            debounce,
            docs: BTreeMap::new(),
            folders: BTreeMap::new(),
            pending: BTreeMap::new(),
        })
    }

    fn key(path: &str) -> Result<(PathBuf, PathBuf), FsError> {
        let path = Path::new(path);
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(FsError::InvalidPath(path.display().to_string()));
        };
        let parent = parent
            .canonicalize()
            .map_err(|_| FsError::NotFound(parent.display().to_string()))?;
        Ok((parent.join(name), parent))
    }

    fn watch_recursive(&mut self, key: &Path) {
        if let Some(doc) = self.docs.get_mut(key)
            && !doc.recursive
            && key.is_dir()
            && self.watcher.watch(key, RecursiveMode::Recursive).is_ok()
        {
            doc.recursive = true;
        }
    }

    fn note(&mut self, event: Event, now: Instant) {
        if matches!(event.kind, EventKind::Access(kind) if kind != AccessKind::Close(AccessMode::Write))
        {
            return;
        }
        for path in event.paths {
            // Events name canonical paths on some platforms and the watched spelling on others.
            let path = path
                .parent()
                .and_then(|p| p.canonicalize().ok())
                .zip(path.file_name())
                .map(|(p, n)| p.join(n))
                .unwrap_or(path);
            let hit = self
                .docs
                .keys()
                .find(|key| path == **key || path.starts_with(key))
                .cloned();
            if let Some(key) = hit {
                // A folder document that has just appeared (a checkout, a rename) needs its own watch.
                self.watch_recursive(&key);
                self.pending.insert(key, now);
            }
        }
    }
}

impl FileWatcher for NotifyWatcher {
    fn id(&self) -> &'static str {
        "notify"
    }

    fn watch(&mut self, path: &str) -> Result<(), FsError> {
        let (key, parent) = Self::key(path)?;
        if self.docs.contains_key(&key) {
            return Ok(());
        }
        let count = self.folders.entry(parent.clone()).or_default();
        if *count == 0 {
            self.watcher
                .watch(&parent, RecursiveMode::NonRecursive)
                .map_err(|e| {
                    self.folders.remove(&parent);
                    unavailable(e)
                })?;
        }
        *self.folders.get_mut(&parent).expect("just counted") += 1;
        self.docs.insert(
            key.clone(),
            Doc {
                name: path.to_owned(),
                parent,
                recursive: false,
            },
        );
        self.watch_recursive(&key);
        Ok(())
    }

    fn unwatch(&mut self, path: &str) {
        let Ok((key, _)) = Self::key(path) else {
            return;
        };
        let Some(doc) = self.docs.remove(&key) else {
            return;
        };
        self.pending.remove(&key);
        if doc.recursive {
            let _ = self.watcher.unwatch(&key);
        }
        if let Some(count) = self.folders.get_mut(&doc.parent) {
            *count -= 1;
            if *count == 0 {
                self.folders.remove(&doc.parent);
                let _ = self.watcher.unwatch(&doc.parent);
            }
        }
    }

    fn changed(&mut self) -> Vec<String> {
        let now = Instant::now();
        while let Ok(event) = self.events.try_recv() {
            // An error from the OS (a queue overflow) means events were lost: treat every document as changed,
            // and let the hash check sort out which really did.
            match event {
                Ok(event) => self.note(event, now),
                Err(_) => {
                    for key in self.docs.keys() {
                        self.pending.insert(key.clone(), now);
                    }
                }
            }
        }
        let ready: BTreeSet<PathBuf> = self
            .pending
            .iter()
            .filter(|(_, at)| now.duration_since(**at) >= self.debounce)
            .map(|(key, _)| key.clone())
            .collect();
        let mut names: Vec<String> = ready
            .iter()
            .filter_map(|key| {
                self.pending.remove(key);
                self.docs.get(key).map(|doc| doc.name.clone())
            })
            .collect();
        names.sort();
        names
    }
}
