//! Deterministic sync fakes. No services, files, wall clock or random ids.
use std::{collections::BTreeMap, sync::Mutex};
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
#[derive(Default)]
pub struct FakeUpdateLog(Mutex<BTreeMap<RoomId, RoomLog>>);
impl UpdateLog for FakeUpdateLog {
    fn load(&self, room: &RoomId) -> Result<Option<RoomLog>, Error> {
        Ok(self
            .0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(room)
            .cloned())
    }
    fn create(&self, room: &RoomId, snapshot: &str) -> Result<(), Error> {
        let mut logs = self.0.lock().map_err(|_| Error::Unavailable)?;
        if logs.contains_key(room) {
            return Err(Error::Conflict);
        }
        logs.insert(
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
        let mut logs = self.0.lock().map_err(|_| Error::Unavailable)?;
        let log = logs.get_mut(room).ok_or(Error::Conflict)?;
        if log.revision != expected || log.epoch != epoch {
            return Err(Error::Conflict);
        }
        let next = expected
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        log.updates.push(LoggedUpdate {
            revision: next,
            bytes: bytes.to_vec(),
        });
        log.revision = next;
        Ok(next)
    }
    fn compact(
        &self,
        room: &RoomId,
        expected: u64,
        epoch: u64,
        snapshot: &str,
    ) -> Result<u64, Error> {
        let mut logs = self.0.lock().map_err(|_| Error::Unavailable)?;
        let log = logs.get_mut(room).ok_or(Error::Conflict)?;
        if log.revision != expected || log.epoch != epoch {
            return Err(Error::Conflict);
        }
        let next = epoch
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        let revision = expected
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        *log = RoomLog {
            epoch: next,
            revision,
            snapshot: snapshot.into(),
            updates: vec![],
        };
        Ok(next)
    }
}
#[derive(Default)]
pub struct FakeSnapshotStore(Mutex<Vec<Vec<u8>>>);
impl SnapshotStore for FakeSnapshotStore {
    fn put(&self, bytes: &[u8]) -> Result<String, Error> {
        let mut blobs = self.0.lock().map_err(|_| Error::Unavailable)?;
        if let Some(index) = blobs.iter().position(|b| b == bytes) {
            return Ok(index.to_string());
        }
        let key = blobs.len().to_string();
        blobs.push(bytes.to_vec());
        Ok(key)
    }
    fn get(&self, key: &str) -> Result<Vec<u8>, Error> {
        self.0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(key.parse::<usize>().map_err(|_| Error::NotFound)?)
            .cloned()
            .ok_or(Error::NotFound)
    }
}
#[derive(Default)]
pub struct FakeAccess(Mutex<BTreeMap<(String, RoomId), (Grant, i64)>>);
impl FakeAccess {
    pub fn grant(&self, key: &str, room: &RoomId, grant: Grant, expires_ms: i64) {
        self.0
            .lock()
            .expect("fake access")
            .insert((key.into(), room.clone()), (grant, expires_ms));
    }
}
impl AccessControl for FakeAccess {
    fn authorize(&self, key: &SecretString, room: &RoomId, now_ms: i64) -> Result<Grant, Error> {
        self.0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .get(&(key.expose().into(), room.clone()))
            .filter(|(_, expiry)| *expiry > now_ms)
            .map(|(grant, _)| grant.clone())
            .ok_or(Error::Forbidden)
    }
}
#[derive(Default)]
pub struct FakePresence(Mutex<BTreeMap<(RoomId, String), PresenceEntry>>);
impl Presence for FakePresence {
    fn touch(&self, room: &RoomId, entry: &PresenceEntry, now_ms: i64) -> Result<(), Error> {
        if entry.expires_ms <= now_ms || entry.expires_ms > now_ms.saturating_add(30_000) {
            return Err(Error::InvalidUpdate);
        }
        self.0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .insert((room.clone(), entry.connection.clone()), entry.clone());
        Ok(())
    }
    fn list(&self, room: &RoomId, now_ms: i64) -> Result<Vec<PresenceEntry>, Error> {
        Ok(self
            .0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .iter()
            .filter(|((r, _), p)| r == room && p.expires_ms > now_ms)
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
