//! Sync persistence uses optimistic room revisions. Snapshots are immutable canonical `.vellum` bytes.
use super::{SecretString, SyncPayload};
use vellum_types::sync::{ClientFrame, Grant, RoomId, RoomLog, ServerFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Forbidden,
    NotFound,
    Conflict,
    ResetRequired,
    InvalidUpdate,
    UpgradeRequired,
    Limit,
    Unavailable,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sync: {}", self.code())
    }
}
impl std::error::Error for Error {}
impl Error {
    pub fn code(self) -> &'static str {
        match self {
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Conflict => "retry",
            Self::ResetRequired => "reset_required",
            Self::InvalidUpdate => "invalid_update",
            Self::UpgradeRequired => "upgrade_required",
            Self::Limit => "limit",
            Self::Unavailable => "unavailable",
        }
    }
}
pub trait UpdateLog: Send + Sync {
    fn load(&self, room: &RoomId) -> Result<Option<RoomLog>, Error>;
    fn create(&self, room: &RoomId, snapshot: &str) -> Result<(), Error>;
    /// Exactly one writer wins a revision. Persistence precedes the acknowledgement/broadcast.
    fn append(&self, room: &RoomId, expected: u64, epoch: u64, bytes: &[u8]) -> Result<u64, Error>;
    /// Publish the snapshot, advance the epoch and revision and prune the old epoch, in one transaction.
    fn compact(
        &self,
        room: &RoomId,
        expected: u64,
        epoch: u64,
        snapshot: &str,
    ) -> Result<u64, Error>;
}
pub trait SnapshotStore: Send + Sync {
    /// Durably store immutable bytes, return an opaque safe key. Equal bytes may reuse a key.
    fn put(&self, bytes: &[u8]) -> Result<String, Error>;
    fn get(&self, key: &str) -> Result<Vec<u8>, Error>;
}
pub trait AccessControl: Send + Sync {
    fn authorize_collaboration(
        &self,
        credential: &SecretString,
        room: &RoomId,
        now_ms: i64,
    ) -> Result<Grant, Error> {
        self.authorize(credential, room, now_ms)
    }
    fn manage(
        &self,
        _credential: &SecretString,
        _room: &RoomId,
        _command: vellum_types::sync::SharingCommand,
        _now_ms: i64,
    ) -> Result<vellum_types::sync::SharingState, Error> {
        Err(Error::Forbidden)
    }
    /// Rechecked on every read/write. Tokens are never stored with document content.
    fn authorize(
        &self,
        credential: &SecretString,
        room: &RoomId,
        now_ms: i64,
    ) -> Result<Grant, Error>;
}
pub use vellum_types::sync::Participant as PresenceEntry;
pub trait Presence: Send + Sync {
    fn touch(&self, room: &RoomId, entry: &PresenceEntry, now_ms: i64) -> Result<(), Error>;
    fn list(&self, room: &RoomId, now_ms: i64) -> Result<Vec<PresenceEntry>, Error>;
    fn remove(&self, room: &RoomId, connection: &str) -> Result<(), Error>;
}
pub trait SyncEndpoint: Send + Sync {
    fn authorize_collaboration(
        &self,
        credential: &SecretString,
        room: &RoomId,
        now_ms: i64,
    ) -> Result<Grant, Error> {
        self.authorize(credential, room, now_ms)
    }
    fn handle(
        &self,
        credential: &SecretString,
        room: &RoomId,
        frame: ClientFrame,
        now_ms: i64,
    ) -> ServerFrame;
    fn authorize(
        &self,
        credential: &SecretString,
        room: &RoomId,
        now_ms: i64,
    ) -> Result<Grant, Error>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pulled {
    pub epoch: u64,
    pub revision: u64,
    pub snapshot: Option<Vec<u8>>,
    pub updates: Vec<SyncPayload>,
}
