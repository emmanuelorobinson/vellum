//! `DocStore` over Loro (ARCHITECTURE §4.2, ADR-004).
//!
//! Loro holds the document as a movable tree and merges edits from other stores; this crate maps our types
//! onto it and back, and nothing of Loro is visible outside it. The store also keeps the document in our types,
//! so reading it costs nothing, and the session's history from `vellum_core::history`, which per-actor undo
//! and "Undo this turn" run on. A change from another store is read back from Loro whole.

mod backend;
#[cfg(test)]
mod tests;

use vellum_core::history::{History, Step, TurnUndo};
use vellum_core::ops::apply_edits;
use vellum_core::ports::{
    ChangeSet, Commit, DocStore, SyncCursor, SyncError, SyncPayload, UndoTurn,
};
use vellum_types::{Actor, Document, Origin, Subsystem, TurnId};

use backend::Backend;

/// The Cargo package name, used by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

const ADAPTER_ID: &str = vellum_types::sync::SYNC_ADAPTER;

/// A document held in Loro.
#[derive(Debug)]
pub struct LoroDocStore {
    backend: Backend,
    doc: Document,
    history: History,
}

impl LoroDocStore {
    /// A store holding `doc`, as replica `peer` in sync. Every store opened from the same document can sync
    /// with every other; `peer` must differ between them and be below 2^63.
    pub fn new(doc: &Document, peer: u64) -> Self {
        Self {
            backend: Backend::genesis(doc, peer),
            doc: doc.clone(),
            history: History::new(),
        }
    }

    /// The document read back from Loro rather than from the copy the store keeps. Always equal to
    /// [`DocStore::snapshot`]; for checks and diagnostics.
    pub fn read_back(&self) -> Document {
        self.backend.read()
    }

    /// Disposable replica retaining CRDT identity, without minting a new writer id.
    pub fn fork(&self) -> Result<Self, SyncError> {
        Ok(Self {
            backend: self.backend.fork()?,
            doc: self.doc.clone(),
            history: self.history.clone(),
        })
    }
    /// Network ingress: every new change must carry this authenticated actor's Origin.
    pub fn import_authenticated(
        &mut self,
        payload: &SyncPayload,
        actor: &Actor,
    ) -> Result<ChangeSet, SyncError> {
        self.import_checked(payload, Some(actor))
    }
    fn import_checked(
        &mut self,
        payload: &SyncPayload,
        actor: Option<&Actor>,
    ) -> Result<ChangeSet, SyncError> {
        if payload.adapter != ADAPTER_ID {
            return Err(SyncError::AdapterMismatch {
                expected: ADAPTER_ID.into(),
                found: payload.adapter.clone(),
            });
        }
        let (origin, after) = self.backend.import_checked(&payload.bytes, actor)?;
        let origin = origin.unwrap_or_else(|| {
            Origin::new(Actor::System {
                subsystem: Subsystem::Sync,
            })
        });
        let changes = self.history.record_remote(&self.doc, &after, origin);
        self.doc = after;
        Ok(changes)
    }

    fn written(&mut self, step: Option<Step>) -> Option<ChangeSet> {
        let step = step?;
        self.backend
            .write(&step.edits, &step.inverse, &step.changes.origin);
        Some(step.changes)
    }
}

impl DocStore for LoroDocStore {
    fn adapter_id(&self) -> &'static str {
        ADAPTER_ID
    }

    fn snapshot(&self) -> Document {
        self.doc.clone()
    }

    fn apply(&mut self, commit: Commit) -> ChangeSet {
        let (origin, edits, _, changes) = commit.into_parts();
        let (inverse, _) = apply_edits(&mut self.doc, &edits, origin.clone())
            .expect("a commit was checked against this document");
        self.backend.write(&edits, &inverse, &origin);
        self.history.record(&origin, &edits, inverse);
        changes
    }

    fn undo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        let step = self.history.undo(&mut self.doc, actor);
        self.written(step)
    }

    fn redo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        let step = self.history.redo(&mut self.doc, actor);
        self.written(step)
    }

    fn undo_turn(&mut self, turn: &TurnId, by: &Actor) -> UndoTurn {
        match self.history.undo_turn(&mut self.doc, turn, by) {
            TurnUndo::Nothing => UndoTurn::Nothing,
            TurnUndo::Revert(revert) => UndoTurn::Revert(revert),
            TurnUndo::Undone(step) => UndoTurn::Undone(self.written(Some(step)).expect("a step")),
        }
    }

    fn cursor(&self) -> SyncCursor {
        SyncCursor(self.backend.cursor())
    }

    fn export_updates(&self, since: &SyncCursor) -> SyncPayload {
        SyncPayload {
            adapter: ADAPTER_ID.into(),
            bytes: self.backend.export(&since.0),
        }
    }

    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError> {
        self.import_checked(payload, None)
    }
}
