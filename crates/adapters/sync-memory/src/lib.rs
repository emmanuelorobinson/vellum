//! In-memory adapters. No filesystem, clock or random ids; credentials live in memory only.
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;

#[derive(Default)]
pub struct MemoryLog {
    rooms: Mutex<BTreeMap<RoomId, RoomLog>>,
    fail: AtomicBool,
}
impl MemoryLog {
    /// Fault injection for persistence contracts.
    pub fn fail_next_write(&self) {
        self.fail.store(true, Ordering::SeqCst);
    }
    fn check(&self) -> Result<(), Error> {
        if self.fail.swap(false, Ordering::SeqCst) {
            Err(Error::Unavailable)
        } else {
            Ok(())
        }
    }
}
impl UpdateLog for MemoryLog {
    fn load(&self, room: &RoomId) -> Result<Option<RoomLog>, Error> {
        Ok(self
            .rooms
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(room)
            .cloned())
    }
    fn create(&self, room: &RoomId, snapshot: &str) -> Result<(), Error> {
        self.check()?;
        let mut rooms = self.rooms.lock().map_err(|_| Error::Unavailable)?;
        if rooms.contains_key(room) {
            return Err(Error::Conflict);
        }
        rooms.insert(
            room.clone(),
            RoomLog {
                epoch: 1,
                revision: 1,
                snapshot: snapshot.into(),
                updates: vec![],
            },
        );
        Ok(())
    }
    fn append(&self, room: &RoomId, expected: u64, epoch: u64, bytes: &[u8]) -> Result<u64, Error> {
        self.check()?;
        let mut rooms = self.rooms.lock().map_err(|_| Error::Unavailable)?;
        let log = rooms.get_mut(room).ok_or(Error::NotFound)?;
        if log.revision != expected || log.epoch != epoch {
            return Err(Error::Conflict);
        }
        log.revision = log
            .revision
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        log.updates.push(LoggedUpdate {
            revision: log.revision,
            bytes: bytes.to_vec(),
        });
        Ok(log.revision)
    }
    fn compact(
        &self,
        room: &RoomId,
        expected: u64,
        epoch: u64,
        snapshot: &str,
    ) -> Result<u64, Error> {
        self.check()?;
        let mut rooms = self.rooms.lock().map_err(|_| Error::Unavailable)?;
        let log = rooms.get_mut(room).ok_or(Error::NotFound)?;
        if log.revision != expected || log.epoch != epoch {
            return Err(Error::Conflict);
        }
        let next_epoch = log
            .epoch
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        let next_rev = log
            .revision
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        *log = RoomLog {
            epoch: next_epoch,
            revision: next_rev,
            snapshot: snapshot.into(),
            updates: vec![],
        };
        Ok(next_epoch)
    }
}
#[derive(Default)]
pub struct MemorySnapshots(Mutex<BTreeMap<String, Vec<u8>>>);
impl SnapshotStore for MemorySnapshots {
    fn put(&self, bytes: &[u8]) -> Result<String, Error> {
        let mut blobs = self.0.lock().map_err(|_| Error::Unavailable)?;
        if let Some((key, _)) = blobs.iter().find(|(_, b)| b.as_slice() == bytes) {
            return Ok(key.clone());
        }
        let key = format!("snapshot-{}", blobs.len());
        blobs.insert(key.clone(), bytes.to_vec());
        Ok(key)
    }
    fn get(&self, key: &str) -> Result<Vec<u8>, Error> {
        self.0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(key)
            .cloned()
            .ok_or(Error::NotFound)
    }
}
#[derive(Default)]
pub struct MemoryAccess(Mutex<BTreeMap<(String, RoomId), (Grant, i64)>>);
impl MemoryAccess {
    pub fn set(&self, credential: &str, room: &RoomId, grant: Grant) {
        self.set_expiring(credential, room, grant, i64::MAX);
    }
    pub fn set_expiring(&self, credential: &str, room: &RoomId, grant: Grant, expires_ms: i64) {
        self.0
            .lock()
            .expect("memory access")
            .insert((credential.into(), room.clone()), (grant, expires_ms));
    }
}
impl AccessControl for MemoryAccess {
    fn authorize(
        &self,
        credential: &SecretString,
        room: &RoomId,
        now_ms: i64,
    ) -> Result<Grant, Error> {
        self.0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(&(credential.expose().into(), room.clone()))
            .filter(|(_, expires)| *expires > now_ms)
            .map(|(grant, _)| grant.clone())
            .ok_or(Error::Forbidden)
    }
}
#[derive(Default)]
pub struct MemoryPresence(Mutex<BTreeMap<(RoomId, String), PresenceEntry>>);
impl Presence for MemoryPresence {
    fn touch(&self, room: &RoomId, entry: &PresenceEntry, now_ms: i64) -> Result<(), Error> {
        if entry.expires_ms <= now_ms || entry.expires_ms > now_ms.saturating_add(30_000) {
            return Err(Error::InvalidUpdate);
        }
        let mut entries = self.0.lock().map_err(|_| Error::Unavailable)?;
        entries.retain(|_, p| p.expires_ms > now_ms);
        entries.insert((room.clone(), entry.connection.clone()), entry.clone());
        Ok(())
    }
    fn list(&self, room: &RoomId, now_ms: i64) -> Result<Vec<PresenceEntry>, Error> {
        let mut entries = self.0.lock().map_err(|_| Error::Unavailable)?;
        entries.retain(|_, p| p.expires_ms > now_ms);
        Ok(entries
            .iter()
            .filter(|((r, _), _)| r == room)
            .map(|(_, p)| p.clone())
            .collect())
    }
    fn remove(&self, room: &RoomId, connection: &str) -> Result<(), Error> {
        self.0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .remove(&(room.clone(), connection.into()));
        Ok(())
    }
}
