//! Document storage and history. Today: Loro. Alternatives: Automerge, Yrs, plain in-memory.
//!
//! The transaction vocabulary here is the smallest one the port needs to be testable: insert, update and
//! delete. Prompt 11 owns the real op set, the validation and the `Actor` model behind [`ActorId`], and
//! replaces `Op` and `OpErrorKind` then; the shape of the port does not change.

use std::fmt;

use vellum_types::{Document, Node, NodeId, NodePatch, PageId};

macro_rules! opaque_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            /// Wraps `id`.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// The id as text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

opaque_id!(
    /// Who made a change: a person's device, an agent, the file watcher or the system. Undo is per actor.
    ActorId
);
opaque_id!(
    /// The agent session a change belongs to.
    SessionId
);
opaque_id!(
    /// The turn of a session a change belongs to; "Undo this turn" reverts by it.
    TurnId
);

/// Where a transaction came from. A transaction cannot be built without one (Invariant 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// Who.
    pub actor: ActorId,
    /// The agent session, when there is one.
    pub session: Option<SessionId>,
    /// The turn, when there is one.
    pub turn: Option<TurnId>,
}

impl Origin {
    /// An origin with an actor and nothing else.
    pub fn new(actor: ActorId) -> Self {
        Self {
            actor,
            session: None,
            turn: None,
        }
    }

    /// The same origin inside a session.
    pub fn in_session(mut self, session: SessionId) -> Self {
        self.session = Some(session);
        self
    }

    /// The same origin inside a turn.
    pub fn in_turn(mut self, turn: TurnId) -> Self {
        self.turn = Some(turn);
        self
    }
}

/// One change to the document.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Adds `node` (and everything under it) to `page`, inside `parent` or at the page's top level, at
    /// `index` among the siblings (`index == len` appends).
    Insert {
        /// The page.
        page: PageId,
        /// The container, or none for the page's top level.
        parent: Option<NodeId>,
        /// Position among the siblings, back to front.
        index: usize,
        /// The node to add. Its id must not exist yet, nor any id under it.
        node: Box<Node>,
    },
    /// Sets properties of a node. The node must read back as the same node afterwards.
    Update {
        /// The node.
        id: NodeId,
        /// The properties to set.
        patch: NodePatch,
    },
    /// Removes a node and everything under it.
    Delete {
        /// The node.
        id: NodeId,
    },
}

/// Changes applied together, all or nothing, as one undo step.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    /// Who made it.
    pub origin: Origin,
    /// What it does, in order.
    pub ops: Vec<Op>,
}

/// What a transaction (or an undo, or an import) changed. Ids are sorted; a subtree counts every node in it.
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
}

impl ChangeSet {
    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }
}

/// Why a transaction was refused. Nothing was applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpError {
    /// Which op, counting from 0. Zero for a fault in the transaction itself.
    pub op_index: usize,
    /// What is wrong.
    pub kind: OpErrorKind,
}

/// What is wrong with an op.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpErrorKind {
    /// The transaction has no ops.
    Empty,
    /// No such page.
    UnknownPage(PageId),
    /// No such node.
    UnknownNode(NodeId),
    /// The parent cannot hold children.
    NotAContainer(NodeId),
    /// The id is taken.
    DuplicateId(NodeId),
    /// The index is past the end of the siblings.
    IndexOutOfRange {
        /// The index given.
        index: usize,
        /// How many siblings there are.
        len: usize,
    },
    /// The patch does not leave a valid node.
    InvalidPatch {
        /// Why.
        message: String,
    },
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            OpErrorKind::Empty => f.write_str("The change has nothing in it."),
            kind => {
                write!(f, "Change {} was refused: ", self.op_index + 1)?;
                match kind {
                    OpErrorKind::Empty => unreachable!(),
                    OpErrorKind::UnknownPage(id) => write!(f, "there is no page {id}."),
                    OpErrorKind::UnknownNode(id) => write!(f, "there is no layer {id}."),
                    OpErrorKind::NotAContainer(id) => write!(f, "layer {id} cannot hold layers."),
                    OpErrorKind::DuplicateId(id) => write!(f, "the id {id} is already used."),
                    OpErrorKind::IndexOutOfRange { index, len } => {
                        write!(f, "position {index} is past the end ({len} layers).")
                    }
                    OpErrorKind::InvalidPatch { message } => write!(f, "{message}"),
                }
            }
        }
    }
}

impl std::error::Error for OpError {}

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
/// Hands out and takes our types only (Invariant 1; ARCHITECTURE §2.3). Nothing from the CRDT leaks. Today
/// anything can call `apply`; Prompt 11 makes it reachable from the op layer alone.
pub trait DocStore {
    /// The adapter's name, written into every [`SyncPayload`]. Stable.
    fn adapter_id(&self) -> &'static str;
    /// The document as it is now, in our types.
    fn snapshot(&self) -> Document;
    /// Applies `tx` as one step, or refuses it whole and changes nothing. A success clears the actor's redo.
    fn apply(&mut self, tx: Transaction) -> Result<ChangeSet, OpError>;
    /// Reverts the actor's latest change that is not undone. None when there is nothing to undo.
    /// Other actors' changes are never touched.
    fn undo(&mut self, actor: &ActorId) -> Option<ChangeSet>;
    /// Reapplies the actor's latest undone change. None when there is nothing to redo.
    fn redo(&mut self, actor: &ActorId) -> Option<ChangeSet>;
    /// How much this store has seen; pass it to a peer's `export_updates` to get what it lacks.
    fn cursor(&self) -> SyncCursor;
    /// Everything this store has that `since` has not seen.
    fn export_updates(&self, since: &SyncCursor) -> SyncPayload;
    /// Merges updates from another store. Importing the same payload twice changes nothing the second time.
    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError>;
}
