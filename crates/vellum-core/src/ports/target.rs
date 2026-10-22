//! Where a tool's transaction goes (ARCHITECTURE §3.4, §7.1; guide Prompt 28).
//!
//! Per file, the daemon holds one target. When an editor has the file open it is a live target: transactions
//! are forwarded to the editor's engine, which owns the document, and the canvas updates in place. With no
//! editor it is a headless target: the daemon's native engine loads the file, applies, renders and saves.
//! Tools never know which one they are talking to.

use std::fmt;

use vellum_types::{Actor, AgentId, Document, NodeId, TurnId};

use super::{ChangeSet, FileError, Kept};
use crate::guard::{AgentControl, LeaseChange, LeaseRefused, WriteError};
use crate::ops::{Applied, Transaction};
use crate::ports::ScriptValue;

/// Which engine owns the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetMode {
    /// An editor has the file open; its engine applies and draws.
    Live,
    /// No editor: the daemon's native engine owns the file.
    Headless,
}

/// Why a target could not do what was asked. A refused transaction changed nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum TargetError {
    /// The transaction is invalid, or the Guard rejected it.
    Write(WriteError),
    /// The file is open read-only here (a newer format, or the daemon could not take the writer role).
    ReadOnly,
    /// The editor holding the file went away before it answered. Nothing is known to have been applied.
    Gone,
    /// The file could not be read or written.
    File(FileError),
    /// Nothing can draw here, or the drawing failed.
    Render(String),
    /// A claim or release of a frame was refused.
    Lease(LeaseRefused),
    /// An editor's updates could not join the engine (damaged, another document's, or not its person's).
    Sync(super::SyncError),
}

impl fmt::Display for TargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(e) => e.fmt(f),
            Self::ReadOnly => f.write_str("The file is open read-only."),
            Self::Gone => f.write_str("The editor holding the file closed before it answered."),
            Self::File(e) => e.fmt(f),
            Self::Render(why) => write!(f, "Could not render: {why}"),
            Self::Lease(why) => why.fmt(f),
            Self::Sync(why) => why.fmt(f),
        }
    }
}

impl std::error::Error for TargetError {}

impl From<WriteError> for TargetError {
    fn from(e: WriteError) -> Self {
        Self::Write(e)
    }
}

impl From<FileError> for TargetError {
    fn from(e: FileError) -> Self {
        Self::File(e)
    }
}

/// What "Undo this turn" did to a target's document (guide Invariant 3).
#[derive(Debug, Clone, PartialEq)]
pub enum TurnUndone {
    /// The turn has no change left to undo.
    Nothing,
    /// Nothing later depended on the turn: its transactions are undone and leave the history.
    Undone(ChangeSet),
    /// Later work by others depends on the turn: what could be reverted was, as one new transaction by whoever
    /// asked; `kept` is what stayed because someone built on it.
    Reverted { applied: Applied, kept: Vec<Kept> },
}

/// One file's document, wherever it lives. One target per file; it is the file's only writer.
pub trait DocumentTarget {
    /// Who owns the document right now.
    fn mode(&self) -> TargetMode;
    /// The document as the owning engine has it, unsaved changes included.
    fn snapshot(&mut self) -> Result<Document, TargetError>;
    /// Applies `tx` as one transaction and one undo step, or refuses it whole.
    fn transact(&mut self, tx: Transaction) -> Result<Applied, TargetError>;
    /// Draws a node or a page to a PNG: the same `{ mime, base64 }` value `doc.screenshot` returns.
    fn render(&mut self, target: &str, scale: f64) -> Result<ScriptValue, TargetError>;
    /// Claims or lets go of `frame` for `agent` where the Guard lives: the editor's engine when live, the
    /// daemon's when headless. Leases are session state; the document does not change.
    fn lease(
        &mut self,
        frame: &NodeId,
        agent: &AgentId,
        change: LeaseChange,
    ) -> Result<(), TargetError>;
    /// Pause all, Stop and their resumes, applied to the Guard where it lives. Session state; the document does
    /// not change.
    fn control(&mut self, change: &AgentControl) -> Result<(), TargetError>;
    /// "Undo this turn": takes back every transaction whose origin carries `turn`, asked for by `by`, and never
    /// another actor's later change (a `Revert` transaction when one depends on it).
    fn undo_turn(&mut self, turn: &TurnId, by: &Actor) -> Result<TurnUndone, TargetError>;
    /// Writes the document to its file. The file is the truth (Invariant 5).
    fn save(&mut self) -> Result<(), TargetError>;
    /// Whether there are changes the file does not have yet.
    fn is_dirty(&mut self) -> Result<bool, TargetError>;
}
