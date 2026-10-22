//! TEST DATA. Shared pieces for the daemon document tests: a fake editor engine and an environment over fakes.
#![allow(dead_code)]

pub mod workspace;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use vellum::daemon::{Documents, EditorLink, Env, Recovery};
use vellum_codec_json::JsonCodec;
use vellum_contracts::fakes::{InMemoryDocStore, ManualClock, ManualWatcher, MemFs, SequentialIds};
use vellum_core::doc_api::NoServices;
use vellum_core::guard::{AgentControl, GuardState, LeaseChange, apply_guarded};
use vellum_core::ops::{Applied, Inputs, Transaction};
use vellum_core::ports::{
    ChangeSet, Clock, DocStore, FileSystem, ScriptValue, TargetError, Timestamp, TurnUndone,
};
use vellum_types::{Actor, AgentId, Document, NodeId, TurnId};

/// What a fake editor did, for the test to look at.
pub struct EditorState {
    pub store: InMemoryDocStore,
    pub saved: Document,
    /// The connection broke: every call answers `Gone`.
    pub gone: bool,
    /// Who made each transaction it applied.
    pub actors: Vec<Actor>,
    /// The full origin of each transaction it applied (session and turn included).
    pub origins: Vec<vellum_types::Origin>,
    /// How many times it followed a writer.
    pub followed: usize,
    pub promoted: bool,
    /// How many times the daemon asked for its document.
    pub snapshots: usize,
    /// Where its saves go.
    pub disk: Option<(Arc<dyn FileSystem>, String)>,
    /// Its Guard: leases claimed through it hold against other agents' writes.
    pub guard: GuardState,
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
            origins: Vec::new(),
            followed: 0,
            promoted: false,
            snapshots: 0,
            disk: None,
            guard: GuardState::default(),
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
        let origin = tx.origin.clone();
        let state = &mut *state;
        let applied = apply_guarded(
            &mut state.store,
            &mut SequentialIds::new(),
            tx,
            &Inputs::default(),
            &mut state.guard,
            Timestamp(0),
        )?;
        state.actors.push(actor);
        state.origins.push(origin);
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

    fn lease(
        &mut self,
        frame: &NodeId,
        agent: &AgentId,
        change: LeaseChange,
    ) -> Result<(), TargetError> {
        self.alive()?;
        let mut state = self.0.borrow_mut();
        let doc = state.store.snapshot();
        state
            .guard
            .lease(&doc, frame, agent, change, Timestamp(0))
            .map_err(TargetError::Lease)
    }

    fn control(&mut self, change: &AgentControl) -> Result<(), TargetError> {
        self.alive()?;
        self.0.borrow_mut().guard.control(change, Timestamp(0));
        Ok(())
    }

    fn undo_turn(&mut self, turn: &TurnId, by: &Actor) -> Result<TurnUndone, TargetError> {
        self.alive()?;
        vellum_core::ops::undo_turn(&mut self.0.borrow_mut().store, turn, by)
            .map_err(|e| TargetError::Write(vellum_core::guard::WriteError::Invalid(e)))
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
    let ids = Rc::new(std::cell::Cell::new(0));
    let entropy = std::cell::Cell::new(0u128);
    Env {
        // An absolute path is a path in the fake disk, as the production locator roots it at the repo.
        locate: Box::new(move |path| {
            Ok((
                fs.clone() as Arc<dyn FileSystem>,
                path.trim_start_matches('/').to_owned(),
            ))
        }),
        codec: Arc::new(JsonCodec::new()),
        hash,
        clock: clock as Arc<dyn Clock>,
        store: Box::new(|doc| Box::new(InMemoryDocStore::new(doc, 1)) as Box<dyn DocStore>),
        ids: Box::new(move || Box::new(SharedIds(ids.clone()))),
        services: Rc::new(NoServices),
        export_services: None,
        document_services: None,
        scripts: None,
        entropy: Box::new(move || {
            entropy.set(entropy.get() + 1);
            entropy.get()
        }),
    }
}

/// Sequential ids from one counter, however many generators the daemon makes, so a tool's new ids and an
/// engine's copies never collide (each real generator is random).
struct SharedIds(Rc<std::cell::Cell<u64>>);

impl vellum_core::ports::IdGen for SharedIds {
    fn node_id(&mut self) -> vellum_types::NodeId {
        self.0.set(self.0.get() + 1);
        vellum_types::NodeId::new(format!("id{:08}", self.0.get())).expect("a valid id")
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

// ---- Prompt 29: tools ----------------------------------------------------------------------------------------

use serde_json::json;
use vellum_core::doc_api::Services;

/// TEST DATA. A small design: a Home page with a Hero frame (text in two sizes, a card bound to a colour variable,
/// a Button instance, notes for agents and a context node), another frame, a Button component, a dark theme.
pub fn design() -> Document {
    serde_json::from_value(json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA: tools", "createdWith": "Vellum 0.0.0"},
        "themes": {"Mode": ["light", "dark"]},
        "variables": {
            "varSurface1": {"type": "color", "name": "color/surface", "values": [
                {"value": "#FFFFFF"}, {"value": "#111111", "when": {"Mode": "dark"}}]},
            "varSpace004": {"type": "number", "name": "space/4", "values": [{"value": 16}]},
            "varAlias001": {"type": "color", "name": "color/card", "values": [{"$alias": "varSurface1"}]}
        },
        "fonts": {"fontAsset01": {"family": "Geist", "weight": 600}},
        "components": {
            "compButton1": {
                "name": "Button",
                "props": {"label": {"type": "text", "default": "Press"}},
                "root": {"type": "frame", "id": "buttonRoot", "name": "Button", "width": 120, "height": 40,
                    "children": [
                        {"type": "text", "id": "buttonText", "name": "Label", "width": "hug", "height": "hug",
                         "content": [{"text": "Press", "font": "Inter", "size": 14}]}]}
            }
        },
        "pages": [
            {"id": "pageHome01", "name": "Home", "children": [
                {"type": "frame", "id": "frameHero1", "name": "Hero", "position": {"type": "absolute", "x": 0, "y": 0}, "width": 390, "height": 300,
                 "agentNotes": "Keep the hero short.",
                 "children": [
                    {"type": "text", "id": "textTitle1", "name": "Title", "position": {"type": "absolute", "x": 20, "y": 20}, "width": 200, "height": 40,
                     "content": [{"text": "Welcome back", "font": "Inter", "size": 32}]},
                    {"type": "text", "id": "textBody01", "name": "Body", "position": {"type": "absolute", "x": 20, "y": 80}, "width": 200, "height": 20,
                     "content": [{"text": "Pick up where you left off", "font": "Inter", "size": 16}]},
                    {"type": "rect", "id": "rectCard01", "name": "Card", "position": {"type": "absolute", "x": 20, "y": 120}, "width": 160, "height": 80,
                     "fills": [{"type": "solid", "color": {"$var": "varSurface1"}}]},
                    {"type": "instance", "id": "instBtn001", "name": "Primary button", "position": {"type": "absolute", "x": 20, "y": 220},
                     "width": 120, "height": 40, "componentId": "compButton1"},
                    {"type": "context", "id": "ctxHero001", "name": "Voice", "position": {"type": "absolute", "x": 300, "y": 20}, "width": 80, "height": 80,
                     "content": "Write like a friend, not a brochure."}
                 ]},
                {"type": "frame", "id": "frameFoot1", "name": "Footer", "position": {"type": "absolute", "x": 0, "y": 400}, "width": 390, "height": 100,
                 "children": [
                    {"type": "text", "id": "textFoot01", "name": "Legal", "position": {"type": "absolute", "x": 20, "y": 20}, "width": 200, "height": 20,
                     "content": [{"text": "All rights reserved", "font": "Inter", "size": 12}]},
                    {"type": "note", "id": "noteFoot01", "name": "Todo", "position": {"type": "absolute", "x": 250, "y": 20}, "width": 100, "height": 60,
                     "content": "Check the legal copy."}
                 ]}
            ]},
            {"id": "pageOther1", "name": "Other", "children": []}
        ]
    }))
    .expect("the test design is a valid document")
}

/// The real scene: Taffy layout, Parley text, rstar index and Skia drawing, exactly as the daemon wires them.
pub fn scene() -> Rc<dyn Services> {
    vellum::daemon::production::scene_services()
}

impl Daemon {
    /// A daemon whose `execute` runs on the real QuickJS sandbox.
    pub fn with_scripts(doc: &Document, paths: &[&str]) -> Self {
        let mut daemon = Self::new(doc, paths);
        let mut env = env(daemon.fs.clone(), daemon.clock.clone());
        env.scripts = Some(Rc::new(vellum_script_quickjs::QuickJsRuntime::new(|| 0, 1)));
        daemon.docs = Documents::new(
            env,
            Box::new(daemon.watcher.clone()),
            Recovery::new(daemon.recovery.clone(), Arc::new(JsonCodec::new()), hash),
        );
        daemon
    }

    /// A daemon whose documents draw and lay out with the real scene.
    pub fn with_scene(doc: &Document, paths: &[&str]) -> Self {
        let mut daemon = Self::new(doc, paths);
        let fs = daemon.fs.clone();
        let clock = daemon.clock.clone();
        let mut env = env(fs, clock);
        env.services = scene();
        env.export_services = Some(vellum::daemon::production::export_services);
        env.document_services = Some(vellum::daemon::production::snapshot_services);
        daemon.docs = Documents::new(
            env,
            Box::new(daemon.watcher.clone()),
            Recovery::new(daemon.recovery.clone(), Arc::new(JsonCodec::new()), hash),
        );
        daemon
    }
}
