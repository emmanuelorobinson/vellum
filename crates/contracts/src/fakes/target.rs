use vellum_core::guard::{AgentControl, GuardState, LeaseChange, WriteError, apply_guarded};
use vellum_core::ops::{self, Applied, Inputs, Transaction};
use vellum_core::ports::{
    DocStore, DocumentTarget, ScriptValue, TargetError, TargetMode, Timestamp, TurnUndone,
};
use vellum_types::{Actor, AgentId, Document, NodeId, TurnId};

use super::{InMemoryDocStore, SequentialIds};

/// A headless target with no file: an in-memory store, the Guard, and a "saved" copy standing in for the disk.
/// It draws nothing.
#[derive(Debug)]
pub struct InMemoryTarget {
    store: InMemoryDocStore,
    ids: SequentialIds,
    guard: GuardState,
    saved: Document,
}

impl InMemoryTarget {
    /// A target holding `doc`, as saved.
    pub fn new(doc: &Document) -> Self {
        Self {
            store: InMemoryDocStore::new(doc, 1),
            ids: SequentialIds::new(),
            guard: GuardState::default(),
            saved: doc.clone(),
        }
    }

    /// What the last save wrote.
    pub fn saved(&self) -> &Document {
        &self.saved
    }

    /// The Guard state agents' writes are checked against.
    pub fn guard_mut(&mut self) -> &mut GuardState {
        &mut self.guard
    }
}

impl DocumentTarget for InMemoryTarget {
    fn mode(&self) -> TargetMode {
        TargetMode::Headless
    }

    fn snapshot(&mut self) -> Result<Document, TargetError> {
        Ok(self.store.snapshot())
    }

    fn transact(&mut self, tx: Transaction) -> Result<Applied, TargetError> {
        Ok(apply_guarded(
            &mut self.store,
            &mut self.ids,
            tx,
            &Inputs::default(),
            &mut self.guard,
            Timestamp(0),
        )?)
    }

    fn render(&mut self, _target: &str, _scale: f64) -> Result<ScriptValue, TargetError> {
        Err(TargetError::Render(
            "the in-memory target draws nothing".into(),
        ))
    }

    fn lease(
        &mut self,
        frame: &NodeId,
        agent: &AgentId,
        change: LeaseChange,
    ) -> Result<(), TargetError> {
        let doc = self.store.snapshot();
        self.guard
            .lease(&doc, frame, agent, change, Timestamp(0))
            .map_err(TargetError::Lease)
    }

    fn control(&mut self, change: &AgentControl) -> Result<(), TargetError> {
        self.guard.control(change, Timestamp(0));
        Ok(())
    }

    fn undo_turn(&mut self, turn: &TurnId, by: &Actor) -> Result<TurnUndone, TargetError> {
        ops::undo_turn(&mut self.store, turn, by)
            .map_err(|e| TargetError::Write(WriteError::Invalid(e)))
    }

    fn save(&mut self) -> Result<(), TargetError> {
        self.saved = self.store.snapshot();
        Ok(())
    }

    fn is_dirty(&mut self) -> Result<bool, TargetError> {
        Ok(self.store.snapshot() != self.saved)
    }
}
