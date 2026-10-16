//! Where agent sessions are kept (ARCHITECTURE §7.6, ADR-006). Conversations live on the person's computer, never
//! in the design file: nothing here is needed to open a document.
//!
//! A session is an append-only log of events, keyed by the repository it belongs to and the document's path inside
//! it, so renaming a branch or moving the checkout's parent folder keeps its sessions. Pictures (step thumbnails,
//! screenshots the agent looked at) are not stored in the log: an event names them by content hash and the store's
//! cache holds the bytes.
use super::clock::Timestamp;
use super::model::ChatRole;
use super::permissions::PermissionRecord;
use crate::plans::PlanEvent;
use crate::turns::{TurnInput, TurnState};
use serde_json::Value;
use std::fmt;
use vellum_types::{AgentId, Origin, SessionId, TurnId};

/// Which document a session belongs to: the repository root and the document's path inside it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocKey {
    repo_root: String,
    path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocKeyError {
    /// The repository root is not an absolute path.
    RootNotAbsolute,
    /// The document path is empty, absolute, or uses `\`, `.` or `..` segments, or empty segments.
    PathNotRelative,
}
impl fmt::Display for DocKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootNotAbsolute => f.write_str("the repository root must be an absolute path"),
            Self::PathNotRelative => f.write_str(
                "the document path must be relative to the repository root, with `/` between plain names",
            ),
        }
    }
}
impl std::error::Error for DocKeyError {}

impl DocKey {
    /// `repo_root` is absolute (`/…`, `C:/…` or `\\server\…`); `path` is relative to it, with `/` separators.
    pub fn new(repo_root: impl Into<String>, path: impl Into<String>) -> Result<Self, DocKeyError> {
        let (repo_root, path) = (repo_root.into(), path.into());
        let bytes = repo_root.as_bytes();
        let drive = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\');
        if !(repo_root.starts_with('/') || repo_root.starts_with("\\\\") || drive) {
            return Err(DocKeyError::RootNotAbsolute);
        }
        let plain = !path.is_empty()
            && !path.contains('\\')
            && path.split('/').all(|part| !matches!(part, "" | "." | ".."));
        if !plain {
            return Err(DocKeyError::PathNotRelative);
        }
        Ok(Self { repo_root, path })
    }
    pub fn repo_root(&self) -> &str {
        &self.repo_root
    }
    /// The document's path inside the repository, `/`-separated.
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Who the session is with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionKind {
    /// A turn-taking conversation with an engine Vellum runs ("builtin", "claude-code", "codex").
    Engine { engine: String },
    /// An agent connected over MCP. Only its tool calls and permission decisions are recorded: its conversation
    /// lives in its own window.
    External { client: String },
}
impl SessionKind {
    /// Whether a session of this kind keeps this event. External sessions keep steps and permission decisions.
    pub fn accepts(&self, body: &EventBody) -> bool {
        match self {
            Self::Engine { .. } => true,
            Self::External { .. } => matches!(body, EventBody::Step(_) | EventBody::Permission(_)),
        }
    }
}

/// What a store needs to start a session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewSession {
    pub id: SessionId,
    pub doc: DocKey,
    pub kind: SessionKind,
    /// The agent whose work it records: a built-in hue or an external client.
    pub agent: AgentId,
    /// Generated from the first request, renameable.
    pub title: String,
    pub created_at: Timestamp,
}

/// A session as the switcher lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionMeta {
    pub id: SessionId,
    pub doc: DocKey,
    pub kind: SessionKind,
    pub agent: AgentId,
    pub title: String,
    pub created_at: Timestamp,
    /// When its newest event happened (its creation, when it has none).
    pub updated_at: Timestamp,
    /// How many events it holds.
    pub events: u64,
}

/// An event's place in its session, from 1, with no gaps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EventSeq(pub u64);

/// A picture in the cache, named by the lowercase hex SHA-256 of its bytes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageRef(String);
impl ImageRef {
    /// Checks that `hash` is 64 lowercase hex digits.
    pub fn new(hash: impl Into<String>) -> Option<Self> {
        let hash = hash.into();
        (hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
            .then_some(Self(hash))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Tokens by kind (guide §9). `input` is input that was neither read from nor written to the provider's cache;
/// `output` includes reasoning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}
impl Tokens {
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }
    #[must_use]
    pub fn plus(self, other: Tokens) -> Tokens {
        Tokens {
            input: self.input.saturating_add(other.input),
            output: self.output.saturating_add(other.output),
            cache_read: self.cache_read.saturating_add(other.cache_read),
            cache_write: self.cache_write.saturating_add(other.cache_write),
        }
    }
}

/// What one model request used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageRecord {
    pub model: String,
    pub tokens: Tokens,
    /// Estimated cost in micro-USD at API rates from `pricing.json`, when the model and every kind used is priced.
    /// Always shown as an estimate.
    pub estimate: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    Done,
    Failed,
    /// The Guard refused the write (lease, pause, limits).
    Rejected,
}

/// One tool call, as the Step Timeline shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRecord {
    pub tool: String,
    /// The arguments as JSON, cut to [`crate::sessions::PREVIEW_CHARS`] with a marker.
    pub arguments: String,
    /// The result (or error) as JSON, cut the same way.
    pub result: String,
    pub thumbnail: Option<ImageRef>,
    pub started_at: Timestamp,
    pub duration_ms: u64,
    pub outcome: StepOutcome,
    /// The step's full details as the Agent panel shows them (target, counts, script, progress).
    pub detail: Value,
}

/// A message part as stored: pictures are cache references, not base64.
#[derive(Clone, Debug, PartialEq)]
pub enum StoredPart {
    Text(String),
    Image {
        mime: String,
        image: ImageRef,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Value,
        metadata: Value,
    },
    ToolResult {
        id: String,
        name: String,
        result: Value,
        failed: bool,
    },
    Continuation(Value),
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredMessage {
    pub role: ChatRole,
    pub parts: Vec<StoredPart>,
}

/// A turn moved from one state to another (guide §10.1 rule 3); the event's origin says who moved it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnTransition {
    pub turn: TurnId,
    pub from: TurnState,
    pub input: TurnInput,
    pub to: TurnState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventBody {
    /// What the person asked, with the context they attached (selection, mentioned files) and pictures.
    Prompt {
        text: String,
        context: Vec<String>,
        images: Vec<ImageRef>,
    },
    /// The messages a turn added, for the next turn's history.
    Messages(Vec<StoredMessage>),
    /// The turn's plan: proposed, built (with its placeholders), ticked, finished or abandoned.
    Plan(PlanEvent),
    Step(StepRecord),
    Usage(UsageRecord),
    Turn(TurnTransition),
    /// A decision about an action outside the canvas (guide Invariant 11), allowed or not.
    Permission(PermissionRecord),
}
impl EventBody {
    /// The pictures this event names.
    pub fn images(&self) -> Vec<&ImageRef> {
        match self {
            Self::Prompt { images, .. } => images.iter().collect(),
            Self::Messages(messages) => messages
                .iter()
                .flat_map(|m| &m.parts)
                .filter_map(|part| match part {
                    StoredPart::Image { image, .. } => Some(image),
                    _ => None,
                })
                .collect(),
            Self::Step(step) => step.thumbnail.iter().collect(),
            Self::Plan(_) | Self::Usage(_) | Self::Turn(_) | Self::Permission(_) => vec![],
        }
    }
}

/// An event before the store gives it a place.
#[derive(Clone, Debug, PartialEq)]
pub struct NewEvent {
    pub at: Timestamp,
    /// Who: the person, the agent, or the system. Its session must be the session it is appended to, and an
    /// event inside a turn carries that turn.
    pub origin: Origin,
    pub body: EventBody,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionEvent {
    pub seq: EventSeq,
    pub at: Timestamp,
    pub origin: Origin,
    pub body: EventBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    UnknownSession,
    /// A session with this id already exists.
    Exists,
    /// The event's origin names another session.
    WrongSession,
    /// External sessions record tool calls and permission decisions only.
    NotRecorded,
    /// A picture the event names is not in the cache.
    MissingImage(ImageRef),
    /// The database or cache failed; the message never holds event contents.
    Io(String),
    /// What is on disk is not something this version wrote.
    Corrupt(String),
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSession => f.write_str("no such session"),
            Self::Exists => f.write_str("the session already exists"),
            Self::WrongSession => f.write_str("the event belongs to another session"),
            Self::NotRecorded => {
                f.write_str("external sessions record tool calls and permission decisions only")
            }
            Self::MissingImage(image) => write!(f, "picture {image} is not in the cache"),
            Self::Io(message) => write!(f, "session store: {message}"),
            Self::Corrupt(message) => write!(f, "session store is damaged: {message}"),
        }
    }
}
impl std::error::Error for StoreError {}

/// The session log. Appends are the only write besides renaming and deleting a session.
pub trait SessionStore {
    fn create(&self, session: NewSession) -> Result<(), StoreError>;
    /// The document's sessions, newest activity first.
    fn sessions(&self, doc: &DocKey) -> Result<Vec<SessionMeta>, StoreError>;
    fn session(&self, id: &SessionId) -> Result<Option<SessionMeta>, StoreError>;
    fn rename(&self, id: &SessionId, title: &str) -> Result<(), StoreError>;
    /// Appends one event and returns its place. Checks the origin's session, the session kind, and that every
    /// picture it names is in the cache.
    fn append(&self, session: &SessionId, event: NewEvent) -> Result<EventSeq, StoreError>;
    /// Events after `since` (all of them for `None`), oldest first.
    fn events(
        &self,
        session: &SessionId,
        since: Option<EventSeq>,
    ) -> Result<Vec<SessionEvent>, StoreError>;
    /// Puts a picture in the cache (idempotent) and returns its name.
    fn put_image(&self, bytes: &[u8]) -> Result<ImageRef, StoreError>;
    fn image(&self, image: &ImageRef) -> Result<Option<Vec<u8>>, StoreError>;
    /// Removes the session, its events, and the cached pictures no other session names. Unknown ids are ignored.
    fn delete(&self, id: &SessionId) -> Result<(), StoreError>;
}
