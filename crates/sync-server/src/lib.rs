//! Sync composition root. Durable updates are the broadcast source; `.vellum` is the recovery format.
use ports::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
pub use vellum_core::ports::sync as ports;
use vellum_core::ports::{DocStore, SecretString, SyncPayload};
use vellum_doc_loro::LoroDocStore;
pub use vellum_sync_memory as memory;
use vellum_types::{Document, sync::*};
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const MAX_ROOMS: usize = 64;
const MAX_TAIL: usize = 256;
const MAX_TAIL_BYTES: usize = 8 * 1024 * 1024;
struct Room {
    revision: u64,
    store: Option<LoroDocStore>,
}
pub struct SyncService {
    log: Arc<dyn UpdateLog>,
    snapshots: Arc<dyn SnapshotStore>,
    access: Arc<dyn AccessControl>,
    rooms: Mutex<BTreeMap<RoomId, Arc<Mutex<Room>>>>,
}
impl SyncService {
    pub fn new(
        log: Arc<dyn UpdateLog>,
        snapshots: Arc<dyn SnapshotStore>,
        access: Arc<dyn AccessControl>,
    ) -> Self {
        Self {
            log,
            snapshots,
            access,
            rooms: Mutex::new(BTreeMap::new()),
        }
    }
    /// Administrative provisioning only. The public sync endpoint cannot create or replace a room.
    pub fn seed(&self, room: &RoomId, document: &Document) -> Result<(), Error> {
        if !vellum_core::ops::check_document(document).is_empty() {
            return Err(Error::InvalidUpdate);
        }
        let bytes = canonical_bytes(document)?;
        let key = self.snapshots.put(&bytes)?;
        self.log.create(room, &key)
    }
    fn load(&self, room: &RoomId) -> Result<RoomLog, Error> {
        let log = self.log.load(room)?.ok_or(Error::NotFound)?;
        if log.epoch == 0
            || log.epoch > MAX_SYNC_COUNTER
            || log.revision > MAX_SYNC_COUNTER
            || log.revision < 1 + log.updates.len() as u64
        {
            return Err(Error::InvalidUpdate);
        }
        if log.updates.len() > MAX_TAIL
            || log.updates.iter().map(|u| u.bytes.len()).sum::<usize>() > MAX_TAIL_BYTES
        {
            return Err(Error::Limit);
        }
        Ok(log)
    }
    fn open(&self, log: &RoomLog) -> Result<LoroDocStore, Error> {
        let bytes = self.snapshots.get(&log.snapshot)?;
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(Error::Limit);
        }
        let doc = vellum_codec_json::JsonCodec::new()
            .decode(&bytes)
            .map_err(|_| Error::InvalidUpdate)?;
        if !vellum_core::ops::check_document(&doc).is_empty() {
            return Err(Error::InvalidUpdate);
        }
        let mut store = LoroDocStore::new(&doc, 0);
        let mut previous = log.revision - log.updates.len() as u64;
        for update in &log.updates {
            if update.revision != previous + 1 || update.bytes.len() > MAX_UPDATE_BYTES {
                return Err(Error::InvalidUpdate);
            }
            store
                .import_updates(&SyncPayload {
                    adapter: SYNC_ADAPTER.into(),
                    bytes: update.bytes.clone(),
                })
                .map_err(|_| Error::InvalidUpdate)?;
            previous = update.revision;
        }
        Ok(store)
    }
    fn room(&self, id: &RoomId) -> Result<Arc<Mutex<Room>>, Error> {
        let mut rooms = self.rooms.lock().map_err(|_| Error::Unavailable)?;
        if let Some(room) = rooms.get(id) {
            return Ok(room.clone());
        }
        if rooms.len() >= MAX_ROOMS
            && let Some(key) = rooms.keys().next().cloned()
        {
            rooms.remove(&key);
        }
        let room = Arc::new(Mutex::new(Room {
            revision: 0,
            store: None,
        }));
        rooms.insert(id.clone(), room.clone());
        Ok(room)
    }
    fn refresh<'a>(
        &self,
        room: &'a mut Room,
        log: &RoomLog,
    ) -> Result<&'a mut LoroDocStore, Error> {
        if room.revision != log.revision {
            room.store = Some(self.open(log)?);
            room.revision = log.revision;
        }
        room.store.as_mut().ok_or(Error::Unavailable)
    }
    pub fn pull(
        &self,
        credential: &SecretString,
        room: &RoomId,
        epoch: u64,
        after: u64,
        now_ms: i64,
    ) -> Result<Pulled, Error> {
        self.access.authorize(credential, room, now_ms)?;
        let log = self.load(room)?;
        let reset = epoch != log.epoch;
        if !reset && after > log.revision {
            return Err(Error::ResetRequired);
        }
        let snapshot = if reset {
            Some(self.snapshots.get(&log.snapshot)?)
        } else {
            None
        };
        Ok(Pulled {
            epoch: log.epoch,
            revision: log.revision,
            snapshot,
            updates: log
                .updates
                .into_iter()
                .filter(|u| reset || u.revision > after)
                .map(|u| SyncPayload {
                    adapter: SYNC_ADAPTER.into(),
                    bytes: u.bytes,
                })
                .collect(),
        })
    }
    pub fn push(
        &self,
        credential: &SecretString,
        id: &RoomId,
        epoch: u64,
        payload: &SyncPayload,
        now_ms: i64,
    ) -> Result<u64, Error> {
        if payload.adapter != SYNC_ADAPTER {
            return Err(Error::UpgradeRequired);
        }
        if payload.bytes.len() > MAX_UPDATE_BYTES || payload.bytes.is_empty() {
            return Err(Error::Limit);
        }
        for _ in 0..3 {
            let grant = self.access.authorize(credential, id, now_ms)?;
            if grant.role != Role::Editor {
                return Err(Error::Forbidden);
            }
            let log = self.load(id)?;
            if log.epoch != epoch {
                return Err(Error::ResetRequired);
            }
            let cached = self.room(id)?;
            let mut room = cached.lock().map_err(|_| Error::Unavailable)?;
            let mut candidate = self
                .refresh(&mut room, &log)?
                .fork()
                .map_err(|_| Error::Unavailable)?;
            let before = candidate.cursor();
            candidate
                .import_authenticated(payload, &grant.actor)
                .map_err(|_| Error::InvalidUpdate)?;
            if candidate.snapshot().comments != self.refresh(&mut room, &log)?.snapshot().comments {
                return Err(Error::Forbidden);
            }
            if candidate.cursor() == before {
                return Ok(log.revision);
            }
            canonical_bytes(&candidate.snapshot())?;
            let accepted = candidate.export_updates(&before);
            if accepted.bytes.len() > MAX_UPDATE_BYTES {
                return Err(Error::Limit);
            }
            if log.updates.len() >= MAX_TAIL
                || log.updates.iter().map(|u| u.bytes.len()).sum::<usize>() + accepted.bytes.len()
                    > MAX_TAIL_BYTES
            {
                return Err(Error::Limit);
            }
            match self.log.append(id, log.revision, epoch, &accepted.bytes) {
                Ok(revision) => {
                    room.store = Some(candidate);
                    room.revision = revision;
                    return Ok(revision);
                }
                Err(Error::Conflict) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(Error::Conflict)
    }
    /// Apply comment policy before committing through the one core Op path and durable CAS.
    pub fn comment(
        &self,
        credential: &SecretString,
        id: &RoomId,
        epoch: u64,
        request: vellum_types::comments::CommentRequest,
        now_ms: i64,
    ) -> Result<u64, Error> {
        use vellum_types::comments::CommentRequest;
        for _ in 0..3 {
            let grant = self
                .access
                .authorize_collaboration(credential, id, now_ms)?;
            if grant.role == Role::Viewer || !grant.actor.is_person() {
                return Err(Error::Forbidden);
            }
            let log = self.load(id)?;
            if epoch != log.epoch {
                return Err(Error::ResetRequired);
            }
            let cached = self.room(id)?;
            let mut room = cached.lock().map_err(|_| Error::Unavailable)?;
            let mut candidate = self
                .refresh(&mut room, &log)?
                .fork()
                .map_err(|_| Error::Unavailable)?;
            let doc = candidate.snapshot();
            let duplicate = match &request {
                CommentRequest::Create {
                    thread,
                    message,
                    anchor,
                    page,
                    point,
                    text,
                } => doc.comments.get(thread).is_some_and(|t| {
                    t.author == grant.actor
                        && t.anchor == *anchor
                        && t.page == *page
                        && t.point == *point
                        && t.messages
                            .get(message)
                            .is_some_and(|m| m.author == grant.actor && m.text == *text)
                }),
                CommentRequest::Reply {
                    thread,
                    message,
                    text,
                } => doc
                    .comments
                    .get(thread)
                    .and_then(|t| t.messages.get(message))
                    .is_some_and(|m| m.author == grant.actor && m.text == *text),
                CommentRequest::Resolve { thread, resolved } => {
                    doc.comments.get(thread).is_some_and(|t| {
                        t.resolved == *resolved
                            && (t.author == grant.actor || grant.role == Role::Editor)
                    })
                }
            };
            if duplicate {
                return Ok(log.revision);
            }
            let tx = vellum_core::comments::transaction(
                &doc,
                grant.actor,
                grant.role,
                request.clone(),
                now_ms,
            )
            .map_err(|e| {
                if e == vellum_core::comments::CommentError::Forbidden {
                    Error::Forbidden
                } else {
                    Error::InvalidUpdate
                }
            })?;
            let before = candidate.cursor();
            vellum_core::ops::apply(&mut candidate, &mut NoCommentIds, tx)
                .map_err(|_| Error::InvalidUpdate)?;
            canonical_bytes(&candidate.snapshot())?;
            let accepted = candidate.export_updates(&before);
            if accepted.bytes.len() > MAX_UPDATE_BYTES
                || log.updates.len() >= MAX_TAIL
                || log.updates.iter().map(|u| u.bytes.len()).sum::<usize>() + accepted.bytes.len()
                    > MAX_TAIL_BYTES
            {
                return Err(Error::Limit);
            }
            match self.log.append(id, log.revision, epoch, &accepted.bytes) {
                Ok(revision) => {
                    room.store = Some(candidate);
                    room.revision = revision;
                    return Ok(revision);
                }
                Err(Error::Conflict) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Error::Conflict)
    }
    /// Store canonical bytes before atomically switching the checkpoint and epoch.
    pub fn compact(
        &self,
        credential: &SecretString,
        id: &RoomId,
        epoch: u64,
        now_ms: i64,
    ) -> Result<u64, Error> {
        let grant = self.access.authorize(credential, id, now_ms)?;
        if grant.role != Role::Editor {
            return Err(Error::Forbidden);
        }
        let log = self.load(id)?;
        if log.epoch != epoch {
            return Err(Error::ResetRequired);
        }
        let cached = self.room(id)?;
        let mut room = cached.lock().map_err(|_| Error::Unavailable)?;
        let store = self.refresh(&mut room, &log)?;
        let bytes = canonical_bytes(&store.snapshot())?;
        let key = self.snapshots.put(&bytes)?;
        let next = self.log.compact(id, log.revision, epoch, &key)?;
        room.revision = 0;
        room.store = None;
        Ok(next)
    }
}
impl SyncEndpoint for SyncService {
    fn authorize_collaboration(
        &self,
        credential: &SecretString,
        room: &RoomId,
        now_ms: i64,
    ) -> Result<Grant, Error> {
        self.access
            .authorize_collaboration(credential, room, now_ms)
    }
    fn authorize(
        &self,
        credential: &SecretString,
        room: &RoomId,
        now_ms: i64,
    ) -> Result<Grant, Error> {
        self.access.authorize(credential, room, now_ms)
    }
    fn handle(
        &self,
        credential: &SecretString,
        room: &RoomId,
        frame: ClientFrame,
        now_ms: i64,
    ) -> ServerFrame {
        let result = if frame.v != SYNC_VERSION || frame.adapter != SYNC_ADAPTER {
            Err(Error::UpgradeRequired)
        } else {
            match frame.request {
                SyncRequest::Sharing { command } => self
                    .access
                    .manage(credential, room, command, now_ms)
                    .map(|state| (frame.epoch, 0, SyncReply::Sharing { state })),
                SyncRequest::Comment { op } => self
                    .comment(credential, room, frame.epoch, op, now_ms)
                    .map(|r| (frame.epoch, r, SyncReply::Ack)),
                SyncRequest::Presence { .. } => Err(Error::InvalidUpdate),
                SyncRequest::Pull { after } => self
                    .pull(credential, room, frame.epoch, after, now_ms)
                    .map(|p| {
                        (
                            p.epoch,
                            p.revision,
                            SyncReply::State {
                                snapshot: p.snapshot,
                                updates: p.updates.into_iter().map(|u| u.bytes).collect(),
                            },
                        )
                    }),
                SyncRequest::Push { bytes } => self
                    .push(
                        credential,
                        room,
                        frame.epoch,
                        &SyncPayload {
                            adapter: frame.adapter,
                            bytes,
                        },
                        now_ms,
                    )
                    .map(|r| (frame.epoch, r, SyncReply::Ack)),
            }
        };
        let (epoch, revision, body) = result.unwrap_or_else(|e| {
            (
                frame.epoch,
                0,
                SyncReply::Error {
                    code: e.code().into(),
                },
            )
        });
        ServerFrame {
            v: SYNC_VERSION,
            adapter: SYNC_ADAPTER.into(),
            epoch,
            revision,
            body,
        }
    }
}

struct NoCommentIds;
impl vellum_core::ports::IdGen for NoCommentIds {
    fn node_id(&mut self) -> vellum_types::NodeId {
        unreachable!("comment transactions supply their ids")
    }
}
// Every admitted state must be recoverable from the canonical format, not only native updates.
fn canonical_bytes(document: &Document) -> Result<Vec<u8>, Error> {
    let codec = vellum_codec_json::JsonCodec::new();
    let bytes = codec.encode(document).map_err(|_| Error::InvalidUpdate)?;
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(Error::Limit);
    }
    let decoded = codec.decode(&bytes).map_err(|_| Error::InvalidUpdate)?;
    if !vellum_core::ops::check_document(&decoded).is_empty() {
        return Err(Error::InvalidUpdate);
    }
    Ok(bytes)
}
