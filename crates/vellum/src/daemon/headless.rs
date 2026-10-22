//! The headless target: no editor holds the file, so the daemon's native engine loads it, applies, renders and
//! saves (ARCHITECTURE §7.1). Every applied transaction is saved at once: with no editor there is nobody to press
//! Save, and the file is the truth (Invariant 5).

use std::rc::Rc;
use std::sync::Arc;

use vellum_codec_json_dir::DirCodec;
use vellum_core::doc_api::{Method, Services};
use vellum_core::files::SingleFile;
use vellum_core::guard::{AgentControl, GuardState, LeaseChange, WriteError, apply_guarded};
use vellum_core::ops::{self, Applied, Inputs, Transaction};
use vellum_core::ports::{
    ChangeSet, Clock, Codec, DocStore, DocumentFiles, DocumentTarget, FileError, FileSystem, IdGen,
    Loaded, ScriptValue, SyncCursor, SyncPayload, TargetError, TargetMode, TurnUndone,
};
use vellum_types::{Actor, AgentId, Document, NodeId, TurnId};

use super::live::{EditorId, Role};

/// A document at a path of a file system: one file, or a folder in the directory format.
#[derive(Clone)]
pub struct DocFile {
    /// The file system the document lives in.
    pub fs: Arc<dyn FileSystem>,
    /// The document's path inside it.
    pub path: String,
    /// The codec both formats encode with.
    pub codec: Arc<dyn Codec>,
}

impl DocFile {
    /// Whether the document is a folder (the directory format).
    pub fn is_folder(&self) -> bool {
        self.fs.list(&self.path).is_ok()
    }

    fn format(&self) -> Box<dyn DocumentFiles + '_> {
        if self.is_folder() {
            Box::new(DirCodec::new(&*self.codec))
        } else {
            Box::new(SingleFile::new(&*self.codec))
        }
    }

    /// Reads the document.
    pub fn read(&self) -> Result<Loaded, FileError> {
        self.format().read(&*self.fs, &self.path)
    }

    /// Writes the document in the format it is already in.
    pub fn write(&self, doc: &Document) -> Result<(), FileError> {
        self.format().write(&*self.fs, &self.path, doc).map(|_| ())
    }
}

/// What the native engine needs besides the file.
pub struct Engine {
    /// The document store, opened on the loaded document.
    pub store: Box<dyn DocStore>,
    /// Ids for copies and boolean containers.
    pub ids: Box<dyn IdGen>,
    /// Scene, layout and drawing for `render` (the same services `doc.screenshot` uses).
    pub services: Rc<dyn Services>,
    /// The Guard's clock (lease expiry).
    pub clock: Arc<dyn Clock>,
}

/// The daemon's own engine holding one file. Editors that open the file are replicas of this engine (ADR-033):
/// they start from its genesis document and its updates, push their person's edits and pull everyone else's.
/// While one is attached the target does not save on its own: the editor's Save writes the file.
pub struct HeadlessTarget {
    file: DocFile,
    engine: Engine,
    guard: GuardState,
    /// What the file holds, as far as this target knows: what it loaded or last wrote.
    saved: Document,
    /// The last save failed; the next transaction or `save` tries again.
    failed: bool,
    /// The document the store was opened on. Replicas start from it, so their updates line up with ours.
    genesis: Document,
    /// The editors replicating this engine, in the order they opened, with the person each one edits as.
    editors: Vec<(EditorId, Actor)>,
    /// Counts the engine's changes, whoever made them: hosts compare it with the revision they last saved.
    revision: u64,
}

/// What an editor needs to start its replica.
#[derive(Debug, Clone)]
pub struct EditorStart {
    /// The first editor is the writer; the others attach. Both edit the same engine.
    pub role: Role,
    /// The document the daemon's store was opened on.
    pub genesis: Document,
    /// Everything since the genesis document: the engine as it is now, unsaved changes included.
    pub updates: SyncPayload,
}

impl HeadlessTarget {
    /// Opens the document at `file`. `make` builds the engine around the loaded document.
    pub fn open(
        file: DocFile,
        make: impl FnOnce(&Document) -> Engine,
    ) -> Result<(Self, Loaded), TargetError> {
        let loaded = file.read()?;
        Ok((
            Self::new(file, make(&loaded.document), loaded.document.clone()),
            loaded,
        ))
    }

    /// A target over an engine that already holds `saved`, which is what the file has.
    pub fn new(file: DocFile, engine: Engine, saved: Document) -> Self {
        Self {
            file,
            engine,
            guard: GuardState::default(),
            genesis: saved.clone(),
            saved,
            failed: false,
            editors: Vec::new(),
            revision: 0,
        }
    }

    /// An editor opens the file. It edits as `actor`; opening again keeps its place.
    pub fn attach(&mut self, editor: EditorId, actor: Actor) -> EditorStart {
        let role = match self.editors.iter().position(|(id, _)| *id == editor) {
            Some(0) => Role::Writer,
            Some(_) => Role::Attached,
            None if self.editors.is_empty() => {
                self.editors.push((editor, actor));
                Role::Writer
            }
            None => {
                self.editors.push((editor, actor));
                Role::Attached
            }
        };
        EditorStart {
            role,
            genesis: self.genesis.clone(),
            updates: self.engine.store.export_updates(&SyncCursor::empty()),
        }
    }

    /// An editor closed the file. Returns whether any editor is left.
    pub fn detach(&mut self, editor: &EditorId) -> bool {
        self.editors.retain(|(id, _)| id != editor);
        !self.editors.is_empty()
    }

    /// How many changes the engine has taken (transactions, editors' updates, undone turns).
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The editors replicating this engine, writer first.
    pub fn editors(&self) -> impl Iterator<Item = &EditorId> {
        self.editors.iter().map(|(id, _)| id)
    }

    /// An editor's person edited: their changes, and only theirs, join the engine.
    pub fn push(
        &mut self,
        editor: &EditorId,
        updates: &SyncPayload,
    ) -> Result<ChangeSet, TargetError> {
        let (_, actor) = self
            .editors
            .iter()
            .find(|(id, _)| id == editor)
            .ok_or(TargetError::Gone)?;
        let changes = self
            .engine
            .store
            .import_updates_as(updates, actor)
            .map_err(TargetError::Sync)?;
        if !changes.is_empty() {
            self.revision += 1;
        }
        Ok(changes)
    }

    /// The sync adapter of the engine's store.
    pub fn adapter_id(&self) -> &'static str {
        self.engine.store.adapter_id()
    }

    /// What an editor whose replica has seen `since` is missing.
    pub fn pull(&self, since: &SyncCursor) -> SyncPayload {
        self.engine.store.export_updates(since)
    }

    /// Saves after a change unless an editor holds the file; then its Save writes.
    fn autosave(&mut self) {
        if self.editors.is_empty() {
            let _ = self.save();
        }
    }

    /// What the file holds, as far as this target knows.
    pub fn saved(&self) -> &Document {
        &self.saved
    }

    /// The file.
    pub fn file(&self) -> &DocFile {
        &self.file
    }

    /// The Guard state agents' writes are checked against (leases, pauses, limits).
    pub fn guard_mut(&mut self) -> &mut GuardState {
        &mut self.guard
    }

    fn apply(&mut self, tx: Transaction) -> Result<Applied, TargetError> {
        let now = self.engine.clock.now();
        self.revision += 1;
        Ok(apply_guarded(
            &mut *self.engine.store,
            &mut *self.engine.ids,
            tx,
            &Inputs::default(),
            &mut self.guard,
            now,
        )?)
    }

    /// Applies a change that came from the file itself, which already has it: nothing is written back. `file` is
    /// what is on disk now.
    pub fn apply_from_file(
        &mut self,
        tx: Transaction,
        file: Document,
    ) -> Result<Applied, TargetError> {
        let applied = self.apply(tx)?;
        self.saved = file;
        if self.editors.is_empty() && self.engine.store.snapshot() != self.saved {
            // The merge kept something the file does not have (a conflict node): write it.
            self.save()?;
        }
        Ok(applied)
    }

    /// The disk view changed under us without a transaction (nothing to apply): remember it.
    pub fn file_is(&mut self, file: Document) {
        self.saved = file;
    }
}

impl DocumentTarget for HeadlessTarget {
    fn mode(&self) -> TargetMode {
        if self.editors.is_empty() {
            TargetMode::Headless
        } else {
            TargetMode::Live
        }
    }

    fn snapshot(&mut self) -> Result<Document, TargetError> {
        Ok(self.engine.store.snapshot())
    }

    fn transact(&mut self, tx: Transaction) -> Result<Applied, TargetError> {
        let applied = self.apply(tx)?;
        // Applied but not saved is a save failure, not a refused transaction: the change stays in the engine
        // and the next save retries (guide §10.3, save_failed).
        self.autosave();
        Ok(applied)
    }

    fn render(&mut self, target: &str, scale: f64) -> Result<ScriptValue, TargetError> {
        let doc = self.engine.store.snapshot();
        let options = ScriptValue::Map(
            [("scale".to_owned(), ScriptValue::Number(scale))]
                .into_iter()
                .collect(),
        );
        self.engine
            .services
            .read(
                Method::Screenshot,
                &doc,
                &[ScriptValue::Text(target.to_owned()), options],
            )
            .map_err(|e| TargetError::Render(e.to_string()))
    }

    fn lease(
        &mut self,
        frame: &NodeId,
        agent: &AgentId,
        change: LeaseChange,
    ) -> Result<(), TargetError> {
        let doc = self.engine.store.snapshot();
        let now = self.engine.clock.now();
        self.guard
            .lease(&doc, frame, agent, change, now)
            .map_err(TargetError::Lease)
    }

    fn control(&mut self, change: &AgentControl) -> Result<(), TargetError> {
        let now = self.engine.clock.now();
        self.guard.control(change, now);
        Ok(())
    }

    fn undo_turn(&mut self, turn: &TurnId, by: &Actor) -> Result<TurnUndone, TargetError> {
        let undone = ops::undo_turn(&mut *self.engine.store, turn, by)
            .map_err(|e| TargetError::Write(WriteError::Invalid(e)))?;
        if undone != TurnUndone::Nothing {
            self.revision += 1;
            // As after a transaction: a failed save is retried, the undo stands.
            self.autosave();
        }
        Ok(undone)
    }

    fn save(&mut self) -> Result<(), TargetError> {
        let doc = self.engine.store.snapshot();
        match self.file.write(&doc) {
            Ok(()) => {
                self.saved = doc;
                self.failed = false;
                Ok(())
            }
            Err(e) => {
                self.failed = true;
                Err(e.into())
            }
        }
    }

    fn is_dirty(&mut self) -> Result<bool, TargetError> {
        Ok(self.failed || self.engine.store.snapshot() != self.saved)
    }
}
