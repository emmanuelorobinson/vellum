//! Document storage and history. Today: Loro. Alternatives: Automerge, Yrs, plain in-memory.
//!
//! A store applies [`Commit`]s: edits the op layer has already checked against this store's document, with
//! their inverse for undo. Only `vellum_core::ops` can build a commit, so `DocStore::apply` is reachable from
//! the op layer alone (Invariant 1); the trybuild tests in `tests/write-path/` hold that in place.

use std::fmt;

use vellum_types::{Actor, Document, NodeId, Origin, VariableId};

use crate::ops::Edit;

/// A checked transaction, ready to apply: its origin, its edits, their inverse and what they change. It has no
/// public constructor and cannot be cloned; `vellum_core::ops::apply` makes one and hands it to a store.
#[derive(Debug)]
pub struct Commit {
    origin: Origin,
    edits: Vec<Edit>,
    inverse: Vec<Edit>,
    changes: ChangeSet,
}

impl Commit {
    pub(crate) fn new(
        origin: Origin,
        edits: Vec<Edit>,
        inverse: Vec<Edit>,
        changes: ChangeSet,
    ) -> Self {
        Self {
            origin,
            edits,
            inverse,
            changes,
        }
    }

    /// Who made it.
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// The edits, in order.
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    /// The edits that undo it, in the order to apply them.
    pub fn inverse(&self) -> &[Edit] {
        &self.inverse
    }

    /// What it changes.
    pub fn changes(&self) -> &ChangeSet {
        &self.changes
    }

    /// Origin, edits, inverse and changes, for a store to keep.
    pub fn into_parts(self) -> (Origin, Vec<Edit>, Vec<Edit>, ChangeSet) {
        (self.origin, self.edits, self.inverse, self.changes)
    }
}

/// What a transaction (or an undo, or an import) changed. Ids are sorted; a subtree counts every node in it. A
/// moved node counts as updated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    /// Who made the change.
    pub origin: Origin,
    /// Nodes that now exist and did not.
    pub inserted: Vec<NodeId>,
    /// Nodes that exist and changed.
    pub updated: Vec<NodeId>,
    /// Nodes that no longer exist.
    pub removed: Vec<NodeId>,
    /// Variables added, changed or removed.
    pub variables: Vec<VariableId>,
}

impl ChangeSet {
    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && self.variables.is_empty()
    }
}

/// How far a store has seen. Opaque: only the adapter that made it can read it. The empty cursor means
/// "nothing yet".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncCursor(pub Vec<u8>);

impl SyncCursor {
    /// Nothing seen: asks for everything.
    pub fn empty() -> Self {
        Self(Vec::new())
    }
}

/// Updates to send to another store: opaque bytes plus the id of the adapter that wrote them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPayload {
    /// The adapter that made the bytes. A store refuses another adapter's.
    pub adapter: String,
    /// The bytes.
    pub bytes: Vec<u8>,
}

/// Why updates could not be imported. The store is unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// Another adapter wrote the payload.
    AdapterMismatch {
        /// This store's adapter.
        expected: String,
        /// The payload's.
        found: String,
    },
    /// The bytes are not a valid payload.
    Malformed(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdapterMismatch { expected, found } => write!(
                f,
                "These updates come from a different version of the engine ({found}, this one uses {expected}). Update to sync."
            ),
            Self::Malformed(why) => write!(f, "These updates are damaged: {why}"),
        }
    }
}

impl std::error::Error for SyncError {}

/// Document storage and history.
///
/// Hands out and takes our types only (ARCHITECTURE §2.3). Nothing from the CRDT leaks.
pub trait DocStore {
    /// The adapter's name, written into every [`SyncPayload`]. Stable.
    fn adapter_id(&self) -> &'static str;
    /// The document as it is now, in our types.
    fn snapshot(&self) -> Document;
    /// Applies `commit` whole as one undo step and clears its actor's redo. The commit was checked against this
    /// store's document, so it applies; `vellum_core::ops::apply_edits` does the work for a store that keeps a
    /// `Document`.
    fn apply(&mut self, commit: Commit) -> ChangeSet;
    /// Reverts the actor's latest change that is not undone. None when there is nothing to undo.
    /// Other actors' changes are never touched.
    fn undo(&mut self, actor: &Actor) -> Option<ChangeSet>;
    /// Reapplies the actor's latest undone change. None when there is nothing to redo.
    fn redo(&mut self, actor: &Actor) -> Option<ChangeSet>;
    /// How much this store has seen; pass it to a peer's `export_updates` to get what it lacks.
    fn cursor(&self) -> SyncCursor;
    /// Everything this store has that `since` has not seen.
    fn export_updates(&self, since: &SyncCursor) -> SyncPayload;
    /// Merges updates from another store. Importing the same payload twice changes nothing the second time.
    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError>;
}
