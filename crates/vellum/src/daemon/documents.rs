//! The daemon's open documents: one target per file, editors attaching to it, the file watcher turning outside
//! changes into ops, and recovery copies (ARCHITECTURE §4.5, §7.1; guide §7 Ownership, Prompt 28).
//!
//! The daemon owns none of the document's truth. A file is held either by an editor (live) or by the daemon's
//! own engine (headless), never both, and whoever holds it is its only writer.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use vellum_core::diff::diff;
use vellum_core::doc_api::Services;
use vellum_core::merge::merge;
use vellum_core::ops::{Applied, Transaction};
use vellum_core::ports::{
    Clock, Codec, DocStore, DocumentTarget, FileError, FileSystem, FileWatcher, FsError, IdGen,
    TargetError, Timestamp,
};
use vellum_types::{Actor, Document, FileReason, Origin};

use super::headless::{DocFile, Engine, HeadlessTarget};
use super::live::{EditorId, EditorLink, LiveTarget, Role};
use super::recovery::{RECOVERY_INTERVAL_MS, Recovery, RecoveryCopy};

/// Finds where a document path lives: the file system and the path inside it.
pub type Locate = Box<dyn Fn(&str) -> Result<(Arc<dyn FileSystem>, String), FsError>>;
/// Opens a document store on a document.
pub type MakeStore = Box<dyn Fn(&Document) -> Box<dyn DocStore>>;

/// Everything the daemon plugs in. Tests use fakes; the binary uses the real adapters.
pub struct Env {
    /// Where a document path lives: the file system to use and the path inside it.
    pub locate: Locate,
    /// The codec both formats encode with.
    pub codec: Arc<dyn Codec>,
    /// A content hash: how self-writes are told from outside changes.
    pub hash: fn(&[u8]) -> String,
    /// UTC milliseconds.
    pub clock: Arc<dyn Clock>,
    /// A document store opened on a document.
    pub store: MakeStore,
    /// Ids for a document's new nodes.
    pub ids: Box<dyn Fn() -> Box<dyn IdGen>>,
    /// Scene and drawing for headless renders.
    pub services: Rc<dyn Services>,
}

/// What opening a file in an editor gave it.
#[derive(Debug, Clone, PartialEq)]
pub struct Opened {
    /// Writer or attached.
    pub role: Role,
    /// What the editor shows: the file, or the writer's engine with its unsaved changes.
    pub document: Document,
    /// Unsaved changes from a session that ended unexpectedly (A06), if any.
    pub recovery: Option<RecoveryCopy>,
}

/// What the watcher's report about one file came to.
#[derive(Debug, Clone, PartialEq)]
pub enum External {
    /// The file holds what the daemon or the holding editor last wrote or already applied: nothing to do.
    Unchanged,
    /// The difference was applied as one transaction by `Actor::File { reason: ExternalChange }`. `conflicts`
    /// counts properties both the file and unsaved edits changed (conflict nodes mark node-level ones).
    Applied {
        /// What changed.
        applied: Box<Applied>,
        /// How many conflicts the merge with unsaved edits found.
        conflicts: usize,
    },
    /// The file cannot be read as a document (a text merge left conflict markers, a half-written file from a
    /// tool that does not save atomically). Nothing was applied; the next change is tried again.
    Unreadable(String),
    /// A folder save is still in progress (or was cut short). Nothing was applied.
    Interrupted,
    /// The file is gone (deleted, or a checkout of a branch without it). The document stays open as it was.
    Missing,
    /// The change could not be applied: the merge or the target refused it.
    Refused(String),
}

/// The holder of a file.
enum Holder {
    Headless(Box<HeadlessTarget>),
    Live(LiveTarget),
}

struct Entry {
    holder: Holder,
    file: DocFile,
    /// What the file holds, as far as the daemon knows, and its hash. For a headless file this follows the
    /// target's saves.
    base: Document,
    base_hash: String,
    /// When the last recovery copy was considered.
    copied_at: Option<Timestamp>,
}

/// The daemon's open documents.
pub struct Documents {
    env: Env,
    watcher: Box<dyn FileWatcher>,
    recovery: Recovery,
    open: BTreeMap<String, Entry>,
}

fn file_actor(reason: FileReason) -> Origin {
    Origin::new(Actor::File { reason })
}

impl Documents {
    /// Nothing open yet.
    pub fn new(env: Env, watcher: Box<dyn FileWatcher>, recovery: Recovery) -> Self {
        Self {
            env,
            watcher,
            recovery,
            open: BTreeMap::new(),
        }
    }

    fn hash(&self, doc: &Document) -> Result<String, FileError> {
        Ok((self.env.hash)(&self.env.codec.encode(doc)?))
    }

    fn file(&self, path: &str) -> Result<DocFile, TargetError> {
        let (fs, inner) = (self.env.locate)(path).map_err(FileError::Fs)?;
        Ok(DocFile {
            fs,
            path: inner,
            codec: self.env.codec.clone(),
        })
    }

    fn engine(&self, doc: &Document) -> Engine {
        Engine {
            store: (self.env.store)(doc),
            ids: (self.env.ids)(),
            services: self.env.services.clone(),
            clock: self.env.clock.clone(),
        }
    }

    /// Opens `path` headless: loads the file and starts watching it.
    fn load(&mut self, path: &str) -> Result<(), TargetError> {
        let file = self.file(path)?;
        let (target, loaded) = HeadlessTarget::open(file.clone(), |doc| self.engine(doc))?;
        let base_hash = self.hash(&loaded.document)?;
        self.watcher.watch(path).map_err(FileError::Fs)?;
        self.open.insert(
            path.to_owned(),
            Entry {
                holder: Holder::Headless(Box::new(target)),
                file,
                base: loaded.document,
                base_hash,
                copied_at: None,
            },
        );
        Ok(())
    }

    /// A writer that stopped answering loses the file: the daemon falls back to the file on disk and keeps the
    /// recovery copy, if one was made, for the next open.
    fn drop_lost(&mut self, path: &str) {
        let lost = matches!(
            self.open.get(path).map(|e| &e.holder),
            Some(Holder::Live(live)) if live.is_lost()
        );
        if lost {
            let Some(entry) = self.open.remove(path) else {
                return;
            };
            let Holder::Live(live) = entry.holder else {
                return;
            };
            let writer = live.writer().clone();
            match live.detach(&writer) {
                Some(promoted) => {
                    self.open.insert(
                        path.to_owned(),
                        Entry {
                            holder: Holder::Live(promoted),
                            ..entry
                        },
                    );
                }
                None => self.watcher.unwatch(path),
            }
        }
    }

    /// The target for `path`, opening it headless when nobody holds it. Tools only ever see this.
    pub fn target(&mut self, path: &str) -> Result<&mut dyn DocumentTarget, TargetError> {
        self.drop_lost(path);
        if !self.open.contains_key(path) {
            self.load(path)?;
        }
        let entry = self.open.get_mut(path).expect("just opened");
        Ok(match &mut entry.holder {
            Holder::Headless(target) => &mut **target,
            Holder::Live(target) => target,
        })
    }

    /// The files the daemon has open, sorted.
    pub fn paths(&self) -> Vec<String> {
        self.open.keys().cloned().collect()
    }

    /// The writer of `path`, when an editor holds it.
    pub fn writer(&self, path: &str) -> Option<&EditorId> {
        match &self.open.get(path)?.holder {
            Holder::Live(live) => Some(live.writer()),
            Holder::Headless(_) => None,
        }
    }

    /// An editor opens `path`. The first editor becomes the writer (a headless target hands over: it has
    /// already saved everything); later ones attach to the writer's engine.
    pub fn open_editor(
        &mut self,
        path: &str,
        editor: EditorId,
        link: Box<dyn EditorLink>,
    ) -> Result<Opened, TargetError> {
        // A writer that quit unexpectedly may not have failed a call yet: ask it before attaching to it.
        while let Some(Holder::Live(live)) = self.open.get_mut(path).map(|e| &mut e.holder) {
            let _ = live.snapshot();
            if !live.is_lost() {
                break;
            }
            self.drop_lost(path);
        }
        if !self.open.contains_key(path) {
            self.load(path)?;
        }
        let recovery = self.recovery.find(path);
        let entry = self.open.get_mut(path).expect("just opened");
        let (role, document) = match &mut entry.holder {
            Holder::Live(live) => {
                // Opening again (a reload of the same editor) keeps the role it has.
                if live.has(&editor) {
                    let role = if *live.writer() == editor {
                        Role::Writer
                    } else {
                        Role::Attached
                    };
                    return Ok(Opened {
                        role,
                        document: live.snapshot()?,
                        recovery,
                    });
                }
                let document = live.snapshot()?;
                live.attach(editor, link);
                (Role::Attached, document)
            }
            Holder::Headless(headless) => {
                // Anything a failed save left behind is written now, or the editor would open without it.
                if headless.is_dirty()? {
                    headless.save()?;
                }
                let document = headless.snapshot()?;
                entry.base = document.clone();
                entry.base_hash =
                    (self.env.hash)(&self.env.codec.encode(&document).map_err(FileError::Codec)?);
                entry.holder = Holder::Live(LiveTarget::new(editor, link));
                (Role::Writer, document)
            }
        };
        Ok(Opened {
            role,
            document,
            recovery,
        })
    }

    /// An editor closed `path` on purpose. Its unsaved changes, if it had any, were saved or discarded by the
    /// person, so its recovery copy goes. When the writer closes, the next attached editor takes over; when
    /// nobody is left the daemon lets go of the file (the next tool call opens it headless again).
    pub fn close_editor(&mut self, path: &str, editor: &EditorId) {
        let Some(entry) = self.open.remove(path) else {
            return;
        };
        let Holder::Live(live) = entry.holder else {
            self.open.insert(path.to_owned(), entry);
            return;
        };
        let was_writer = live.writer() == editor;
        match live.detach(editor) {
            Some(live) => {
                self.open.insert(
                    path.to_owned(),
                    Entry {
                        holder: Holder::Live(live),
                        ..entry
                    },
                );
            }
            None => {
                self.watcher.unwatch(path);
                if was_writer {
                    let _ = self.recovery.discard(path);
                }
            }
        }
    }

    /// The writer saved `path`, writing `saved`. The watcher's report of that write will be ignored, and the
    /// recovery copy goes.
    pub fn editor_saved(&mut self, path: &str, saved: &Document) -> Result<(), FileError> {
        let hash = self.hash(saved)?;
        if let Some(entry) = self.open.get_mut(path) {
            entry.base = saved.clone();
            entry.base_hash = hash;
            entry.copied_at = None;
        }
        self.recovery.discard(path).map_err(FileError::Fs)
    }

    /// Saves `path` through whoever holds it.
    pub fn save(&mut self, path: &str) -> Result<(), TargetError> {
        self.drop_lost(path);
        let entry = self.open.get_mut(path).ok_or(TargetError::Gone)?;
        let saved = match &mut entry.holder {
            Holder::Headless(target) => {
                target.save()?;
                target.saved().clone()
            }
            Holder::Live(target) => target.save_returning()?,
        };
        self.editor_saved(path, &saved)?;
        Ok(())
    }

    /// Brings `base` up to date for a headless file, whose target saves on its own.
    fn sync_headless(&mut self, path: &str) -> Result<(), FileError> {
        let Some(entry) = self.open.get(path) else {
            return Ok(());
        };
        let Holder::Headless(target) = &entry.holder else {
            return Ok(());
        };
        if *target.saved() == entry.base {
            return Ok(());
        }
        let saved = target.saved().clone();
        let hash = self.hash(&saved)?;
        let entry = self.open.get_mut(path).expect("present");
        entry.base = saved;
        entry.base_hash = hash;
        // A headless save is a save: there is nothing left to recover.
        self.recovery.discard(path).map_err(FileError::Fs)
    }

    /// Handles what the watcher reported: every changed file is read again and, when it differs from what the
    /// daemon knows, the difference is applied as ops by `Actor::File`. Open editors update in place. Unsaved
    /// edits are kept: the change is a three-way merge of the last known file, the engine's document and the
    /// new file.
    pub fn poll(&mut self) -> Vec<(String, External)> {
        let changed = self.watcher.changed();
        let mut out = Vec::new();
        for path in changed {
            if !self.open.contains_key(&path) {
                continue;
            }
            self.drop_lost(&path);
            let outcome = self.external(&path);
            out.push((path, outcome));
        }
        out
    }

    fn external(&mut self, path: &str) -> External {
        if let Err(e) = self.sync_headless(path) {
            return External::Refused(e.to_string());
        }
        let Some(entry) = self.open.get(path) else {
            return External::Unchanged;
        };
        let loaded = match entry.file.read() {
            Ok(loaded) => loaded,
            Err(FileError::Fs(FsError::NotFound(_))) => return External::Missing,
            Err(e) => return External::Unreadable(e.to_string()),
        };
        if loaded.interrupted {
            return External::Interrupted;
        }
        let theirs = loaded.document;
        let theirs_hash = match self.hash(&theirs) {
            Ok(hash) => hash,
            Err(e) => return External::Unreadable(e.to_string()),
        };
        // Self-writes (the daemon's headless saves, the writer's saves) and touches that change nothing.
        if theirs_hash == entry.base_hash {
            return External::Unchanged;
        }
        let base = entry.base.clone();
        let entry = self.open.get_mut(path).expect("present");
        let ours = match &mut entry.holder {
            Holder::Headless(t) => t.snapshot(),
            Holder::Live(t) => t.snapshot(),
        };
        let ours = match ours {
            Ok(doc) => doc,
            Err(e) => return External::Refused(e.to_string()),
        };
        let merged = match merge(&base, &ours, &theirs) {
            Ok(merged) => merged,
            Err(e) => return External::Refused(format!("{e:?}")),
        };
        let ops = match diff(&ours, &merged.document) {
            Ok(ops) => ops,
            Err(e) => return External::Refused(e.to_string()),
        };
        let conflicts = merged.conflicts.len();
        let outcome = if ops.is_empty() {
            if let Holder::Headless(t) = &mut entry.holder {
                t.file_is(theirs.clone());
            }
            External::Unchanged
        } else {
            let tx = Transaction::new(file_actor(FileReason::ExternalChange), ops);
            let result = match &mut entry.holder {
                Holder::Headless(t) => t.apply_from_file(tx, theirs.clone()),
                Holder::Live(t) => t.transact(tx),
            };
            match result {
                Ok(applied) => External::Applied {
                    applied: Box::new(applied),
                    conflicts,
                },
                Err(e) => return External::Refused(e.to_string()),
            }
        };
        entry.base = theirs;
        entry.base_hash = theirs_hash;
        outcome
    }

    /// Writes a recovery copy for every file whose writer has unsaved changes and whose last copy is at least
    /// [`RECOVERY_INTERVAL_MS`] old. Returns the paths copied. Call it every second or so.
    pub fn tick(&mut self) -> Vec<String> {
        let now = self.env.clock.now();
        let mut copied = Vec::new();
        let paths: Vec<String> = self.open.keys().cloned().collect();
        for path in paths {
            let entry = self.open.get_mut(&path).expect("present");
            let Holder::Live(live) = &mut entry.holder else {
                continue;
            };
            if entry
                .copied_at
                .is_some_and(|at| now.0 - at.0 < RECOVERY_INTERVAL_MS)
            {
                continue;
            }
            entry.copied_at = Some(now);
            let document = match live.is_dirty() {
                Ok(true) => live.snapshot(),
                _ => continue,
            };
            if let Ok(document) = document
                && self.recovery.write(&path, &document, now).is_ok()
            {
                copied.push(path);
            }
        }
        copied
    }

    /// Every recovery copy on disk, newest first (and those that could not be read).
    pub fn recoveries(&self) -> (Vec<RecoveryCopy>, Vec<super::recovery::DamagedCopy>) {
        self.recovery.list()
    }

    /// Applies `path`'s recovery copy to its target as one transaction by `Actor::File { reason: Recovery }`.
    /// The copy stays until the file is saved.
    pub fn restore(&mut self, path: &str) -> Result<Applied, TargetError> {
        let copy = self
            .recovery
            .find(path)
            .ok_or(TargetError::File(FileError::Fs(FsError::NotFound(
                path.to_owned(),
            ))))?;
        let target = self.target(path)?;
        let current = target.snapshot()?;
        let ops = diff(&current, &copy.document)
            .map_err(|e| TargetError::Write(vellum_core::guard::WriteError::Invalid(e)))?;
        let applied = target.transact(Transaction::new(file_actor(FileReason::Recovery), ops))?;
        self.sync_headless(path)?;
        Ok(applied)
    }

    /// Removes `path`'s recovery copy (Discard backup).
    pub fn discard_recovery(&mut self, path: &str) -> Result<(), FsError> {
        self.recovery.discard(path)
    }
}
