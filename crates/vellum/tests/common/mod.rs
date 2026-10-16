//! TEST DATA. Shared pieces for the daemon document tests: a fake editor engine and an environment over fakes.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use vellum::daemon::{Documents, EditorLink, Env, Recovery};
use vellum_codec_json::JsonCodec;
use vellum_contracts::fakes::{InMemoryDocStore, ManualClock, ManualWatcher, MemFs, SequentialIds};
use vellum_core::doc_api::NoServices;
use vellum_core::guard::{GuardState, apply_guarded};
use vellum_core::ops::{Applied, Inputs, Transaction};
use vellum_core::ports::{
    ChangeSet, Clock, DocStore, FileSystem, ScriptValue, TargetError, Timestamp,
};
use vellum_types::{Actor, Document};

/// What a fake editor did, for the test to look at.
pub struct EditorState {
    pub store: InMemoryDocStore,
    pub saved: Document,
    /// The connection broke: every call answers `Gone`.
    pub gone: bool,
    /// Who made each transaction it applied.
    pub actors: Vec<Actor>,
    /// How many times it followed a writer.
    pub followed: usize,
    pub promoted: bool,
    /// How many times the daemon asked for its document.
    pub snapshots: usize,
    /// Where its saves go.
    pub disk: Option<(Arc<dyn FileSystem>, String)>,
}

/// An editor engine in memory. Clones share the state, so the test keeps one and hands the other to the daemon.
#[derive(Clone)]
pub struct FakeEditor(pub Rc<RefCell<EditorState>>);

impl FakeEditor {
    pub fn new(doc: &Document) -> Self {
        Self(Rc::new(RefCell::new(EditorState {
            store: InMemoryDocStore::new(doc, 7),
            saved: doc.clone(),
            gone: false,
            actors: Vec::new(),
            followed: 0,
            promoted: false,
            snapshots: 0,
            disk: None,
        })))
    }

    /// An editor whose saves write `path` in `fs`.
    pub fn saving_to(doc: &Document, fs: Arc<dyn FileSystem>, path: &str) -> Self {
        let editor = Self::new(doc);
        editor.0.borrow_mut().disk = Some((fs, path.to_owned()));
        editor
    }

    pub fn link(&self) -> Box<dyn EditorLink> {
        Box::new(self.clone())
    }

    pub fn doc(&self) -> Document {
        self.0.borrow().store.snapshot()
    }

    /// An edit the person made in this editor (not through the daemon).
    pub fn edit(&self, tx: Transaction) {
        let mut state = self.0.borrow_mut();
        apply_guarded(
            &mut state.store,
            &mut SequentialIds::new(),
            tx,
            &Inputs::default(),
            &mut GuardState::default(),
            Timestamp(0),
        )
        .expect("the person's edit applies");
    }

    fn alive(&self) -> Result<(), TargetError> {
        if self.0.borrow().gone {
            Err(TargetError::Gone)
        } else {
            Ok(())
        }
    }
}

impl EditorLink for FakeEditor {
    fn snapshot(&mut self) -> Result<Document, TargetError> {
        self.alive()?;
        self.0.borrow_mut().snapshots += 1;
        Ok(self.doc())
    }

    fn transact(&mut self, tx: Transaction) -> Result<Applied, TargetError> {
        self.alive()?;
        let mut state = self.0.borrow_mut();
        let clean = state.store.snapshot() == state.saved;
        let from_file = matches!(tx.origin.actor, Actor::File { .. });
        let actor = tx.origin.actor.clone();
        let applied = apply_guarded(
            &mut state.store,
            &mut SequentialIds::new(),
            tx,
            &Inputs::default(),
            &mut GuardState::default(),
            Timestamp(0),
        )?;
        state.actors.push(actor);
        // guide §10.3: an external change is applied as ops and the editor stays in its save state.
        if from_file && clean {
            state.saved = state.store.snapshot();
        }
        Ok(applied)
    }

    fn render(&mut self, target: &str, _scale: f64) -> Result<ScriptValue, TargetError> {
        self.alive()?;
        Ok(ScriptValue::Text(format!("editor drew {target}")))
    }

    fn save(&mut self) -> Result<Document, TargetError> {
        self.alive()?;
        let mut state = self.0.borrow_mut();
        let doc = state.store.snapshot();
        if let Some((fs, path)) = &state.disk {
            let bytes = JsonCodec::new().encode(&doc).expect("encode");
            vellum_core::files::atomic_write(&**fs, path, &bytes).expect("the editor saves");
        }
        state.saved = doc.clone();
        Ok(doc)
    }

    fn is_dirty(&mut self) -> Result<bool, TargetError> {
        self.alive()?;
        let state = self.0.borrow();
        Ok(state.store.snapshot() != state.saved)
    }

    fn follow(&mut self, _changes: &ChangeSet, document: &Document) -> Result<(), TargetError> {
        self.alive()?;
        let mut state = self.0.borrow_mut();
        state.store = InMemoryDocStore::new(document, 7);
        state.followed += 1;
        Ok(())
    }

    fn promote(&mut self) -> Result<(), TargetError> {
        self.alive()?;
        self.0.borrow_mut().promoted = true;
        Ok(())
    }
}

pub fn hash(bytes: &[u8]) -> String {
    vellum_assets_fs::content_hash(bytes)
}

pub fn encode(doc: &Document) -> Vec<u8> {
    JsonCodec::new().encode(doc).expect("encode")
}

pub fn decode(bytes: &[u8]) -> Document {
    JsonCodec::new().decode(bytes).expect("decode")
}

/// An environment where every path is a path in `fs`, with the fake store and no renderer.
pub fn env(fs: Arc<MemFs>, clock: Arc<ManualClock>) -> Env {
    Env {
        locate: Box::new(move |path| Ok((fs.clone() as Arc<dyn FileSystem>, path.to_owned()))),
        codec: Arc::new(JsonCodec::new()),
        hash,
        clock: clock as Arc<dyn Clock>,
        store: Box::new(|doc| Box::new(InMemoryDocStore::new(doc, 1)) as Box<dyn DocStore>),
        ids: Box::new(|| Box::new(SequentialIds::new())),
        services: Rc::new(NoServices),
    }
}

/// A daemon over fakes: the documents' disk, the recovery folder's disk, the watcher and the clock.
pub struct Daemon {
    pub docs: Documents,
    pub fs: Arc<MemFs>,
    pub recovery: Arc<MemFs>,
    pub watcher: ManualWatcher,
    pub clock: Arc<ManualClock>,
}

impl Daemon {
    /// With `doc` saved at each of `paths`.
    pub fn new(doc: &Document, paths: &[&str]) -> Self {
        let fs = Arc::new(MemFs::new());
        for path in paths {
            fs.seed(path, &encode(doc));
        }
        let recovery = Arc::new(MemFs::new());
        let watcher = ManualWatcher::new();
        let clock = Arc::new(ManualClock::new(1_000_000));
        let docs = Documents::new(
            env(fs.clone(), clock.clone()),
            Box::new(watcher.clone()),
            Recovery::new(recovery.clone(), Arc::new(JsonCodec::new()), hash),
        );
        Self {
            docs,
            fs,
            recovery,
            watcher,
            clock,
        }
    }

    /// Writes `doc` at `path` as another program would, and lets the watcher notice.
    pub fn write_outside(&self, path: &str, doc: &Document) {
        self.fs.write(path, &encode(doc)).expect("write");
        self.watcher.touch(path);
    }

    pub fn on_disk(&self, path: &str) -> Document {
        decode(&self.fs.read(path).expect("read"))
    }
}
