//! SQLite keeps checkpoint publication and pruning in the same transaction. Raw tokens never reach SQL.
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::Mutex, time::Duration};
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sync_rooms (id TEXT PRIMARY KEY, epoch INTEGER NOT NULL, revision INTEGER NOT NULL, snapshot TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sync_updates (room TEXT NOT NULL REFERENCES sync_rooms(id), revision INTEGER NOT NULL, bytes BLOB NOT NULL, PRIMARY KEY(room,revision));
CREATE TABLE IF NOT EXISTS sync_grants (digest TEXT NOT NULL, room TEXT NOT NULL, grant_json TEXT NOT NULL, expires_ms INTEGER NOT NULL, PRIMARY KEY(digest,room));
CREATE TABLE IF NOT EXISTS sync_owners (room TEXT PRIMARY KEY, actor TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1);
CREATE INDEX IF NOT EXISTS sync_grants_by_room ON sync_grants(room,expires_ms);";
pub struct SqliteLog(Mutex<Connection>);
fn unavailable(_: rusqlite::Error) -> Error {
    Error::Unavailable
}
fn digest(secret: &SecretString) -> String {
    Sha256::digest(secret.expose().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
impl SqliteLog {
    /// Trusted provisioning only; never exposed as a public room command.
    pub fn set_owner(&self, room: &RoomId, actor: &vellum_types::Actor) -> Result<(), Error> {
        if !actor.is_person() {
            return Err(Error::InvalidUpdate);
        }
        let actor = serde_json::to_string(actor).map_err(|_| Error::InvalidUpdate)?;
        self.0.lock().map_err(|_|Error::Unavailable)?.execute("INSERT INTO sync_owners(room,actor) VALUES (?1,?2) ON CONFLICT(room) DO UPDATE SET actor=excluded.actor",params![room.as_str(),actor]).map_err(unavailable)?;
        Ok(())
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let connection = Connection::open(path).map_err(unavailable)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(unavailable)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;",
            )
            .map_err(unavailable)?;
        connection.execute_batch(SCHEMA).map_err(unavailable)?;
        Ok(Self(Mutex::new(connection)))
    }
    pub fn grant(
        &self,
        secret: &SecretString,
        room: &RoomId,
        grant: &Grant,
        expires_ms: i64,
    ) -> Result<(), Error> {
        let json = serde_json::to_string(grant).map_err(|_| Error::InvalidUpdate)?;
        self.0.lock().map_err(|_| Error::Unavailable)?.execute("INSERT INTO sync_grants VALUES (?1,?2,?3,?4) ON CONFLICT(digest,room) DO UPDATE SET grant_json=excluded.grant_json, expires_ms=excluded.expires_ms", params![digest(secret), room.as_str(), json, expires_ms]).map_err(unavailable)?;
        Ok(())
    }
    pub fn revoke(&self, secret: &SecretString, room: &RoomId) -> Result<(), Error> {
        self.0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .execute(
                "DELETE FROM sync_grants WHERE digest=?1 AND room=?2",
                params![digest(secret), room.as_str()],
            )
            .map_err(unavailable)?;
        Ok(())
    }
}
impl AccessControl for SqliteLog {
    fn manage(
        &self,
        secret: &SecretString,
        room: &RoomId,
        command: SharingCommand,
        now_ms: i64,
    ) -> Result<SharingState, Error> {
        let mut connection = self.0.lock().map_err(|_| Error::Unavailable)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let json: Option<String> = tx
            .query_row(
                "SELECT grant_json FROM sync_grants WHERE digest=?1 AND room=?2 AND expires_ms>?3",
                params![digest(secret), room.as_str(), now_ms],
                |r| r.get(0),
            )
            .optional()
            .map_err(unavailable)?;
        let grant: Grant =
            serde_json::from_str(&json.ok_or(Error::Forbidden)?).map_err(|_| Error::Unavailable)?;
        let (owner_actor, mut enabled): (String, bool) = tx
            .query_row(
                "SELECT actor,enabled FROM sync_owners WHERE room=?1",
                [room.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(unavailable)?
            .ok_or(Error::NotFound)?;
        let actor = serde_json::to_string(&grant.actor).map_err(|_| Error::InvalidUpdate)?;
        let owner = owner_actor == actor;
        if !owner && (!enabled || !matches!(command, SharingCommand::Read)) {
            return Err(Error::Forbidden);
        }
        match command {
            SharingCommand::Read => {}
            SharingCommand::SetLive { enabled: value } => {
                tx.execute(
                    "UPDATE sync_owners SET enabled=?1 WHERE room=?2",
                    params![value, room.as_str()],
                )
                .map_err(unavailable)?;
                enabled = value;
            }
            SharingCommand::SetRole {
                actor: target,
                role,
            } => {
                let target = serde_json::to_string(&target).map_err(|_| Error::InvalidUpdate)?;
                if target == owner_actor {
                    return Err(Error::Forbidden);
                }
                let updated = serde_json::to_string(&Grant {
                    actor: serde_json::from_str(&target).map_err(|_| Error::InvalidUpdate)?,
                    role,
                })
                .map_err(|_| Error::InvalidUpdate)?;
                tx.execute("UPDATE sync_grants SET grant_json=?1 WHERE room=?2 AND json_extract(grant_json,'$.actor')=?3",params![updated,room.as_str(),target]).map_err(unavailable)?;
            }
            SharingCommand::Remove { actor: target } => {
                let target = serde_json::to_string(&target).map_err(|_| Error::InvalidUpdate)?;
                if target == owner_actor {
                    return Err(Error::Forbidden);
                }
                tx.execute("DELETE FROM sync_grants WHERE room=?1 AND json_extract(grant_json,'$.actor')=?2",params![room.as_str(),target]).map_err(unavailable)?;
            }
        }
        let members = {
            let mut stmt=tx.prepare("SELECT DISTINCT grant_json FROM sync_grants WHERE room=?1 AND expires_ms>?2 ORDER BY grant_json LIMIT 1025").map_err(unavailable)?;
            let rows = stmt
                .query_map(params![room.as_str(), now_ms], |r| r.get::<_, String>(0))
                .map_err(unavailable)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(unavailable)?;
            if rows.len() > 1024 {
                return Err(Error::Limit);
            }
            rows.into_iter()
                .map(|s| serde_json::from_str(&s).map_err(|_| Error::Unavailable))
                .collect::<Result<Vec<Grant>, _>>()?
        };
        tx.commit().map_err(unavailable)?;
        Ok(SharingState {
            owner_actor: serde_json::from_str(&owner_actor).map_err(|_| Error::Unavailable)?,
            owner,
            enabled,
            can_live: true,
            members,
        })
    }
    fn authorize(&self, secret: &SecretString, room: &RoomId, now_ms: i64) -> Result<Grant, Error> {
        let connection = self.0.lock().map_err(|_| Error::Unavailable)?;
        let json: Option<String> = connection
            .query_row(
                "SELECT grant_json FROM sync_grants WHERE digest=?1 AND room=?2 AND expires_ms>?3",
                params![digest(secret), room.as_str(), now_ms],
                |row| row.get(0),
            )
            .optional()
            .map_err(unavailable)?;
        let grant: Grant =
            serde_json::from_str(&json.ok_or(Error::Forbidden)?).map_err(|_| Error::Unavailable)?;
        let state: Option<(String, bool)> = connection
            .query_row(
                "SELECT actor,enabled FROM sync_owners WHERE room=?1",
                [room.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(unavailable)?;
        if let Some((owner, false)) = state
            && owner != serde_json::to_string(&grant.actor).map_err(|_| Error::Unavailable)?
        {
            return Err(Error::Forbidden);
        }
        Ok(grant)
    }
}
impl UpdateLog for SqliteLog {
    fn load(&self, room: &RoomId) -> Result<Option<RoomLog>, Error> {
        let mut connection = self.0.lock().map_err(|_| Error::Unavailable)?;
        let tx = connection.transaction().map_err(unavailable)?;
        let log = tx
            .query_row(
                "SELECT epoch,revision,snapshot FROM sync_rooms WHERE id=?1",
                [room.as_str()],
                |row| {
                    Ok(RoomLog {
                        epoch: unsigned(row, 0)?,
                        revision: unsigned(row, 1)?,
                        snapshot: row.get(2)?,
                        updates: vec![],
                    })
                },
            )
            .optional()
            .map_err(unavailable)?;
        let Some(mut log) = log else {
            return Ok(None);
        };
        let mut stmt = tx
            .prepare("SELECT revision,bytes FROM sync_updates WHERE room=?1 ORDER BY revision")
            .map_err(unavailable)?;
        log.updates = stmt
            .query_map([room.as_str()], |row| {
                Ok(LoggedUpdate {
                    revision: unsigned(row, 0)?,
                    bytes: row.get(1)?,
                })
            })
            .map_err(unavailable)?
            .collect::<Result<_, _>>()
            .map_err(unavailable)?;
        Ok(Some(log))
    }
    fn create(&self, room: &RoomId, snapshot: &str) -> Result<(), Error> {
        let n = self
            .0
            .lock()
            .map_err(|_| Error::Unavailable)?
            .execute(
                "INSERT OR IGNORE INTO sync_rooms VALUES (?1,1,1,?2)",
                params![room.as_str(), snapshot],
            )
            .map_err(unavailable)?;
        if n == 1 { Ok(()) } else { Err(Error::Conflict) }
    }
    fn append(&self, room: &RoomId, expected: u64, epoch: u64, bytes: &[u8]) -> Result<u64, Error> {
        let next = expected
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        let mut connection = self.0.lock().map_err(|_| Error::Unavailable)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let n = tx
            .execute(
                "UPDATE sync_rooms SET revision=?1 WHERE id=?2 AND revision=?3 AND epoch=?4",
                params![
                    signed(next)?,
                    room.as_str(),
                    signed(expected)?,
                    signed(epoch)?
                ],
            )
            .map_err(unavailable)?;
        if n != 1 {
            return Err(Error::Conflict);
        }
        tx.execute(
            "INSERT INTO sync_updates VALUES (?1,?2,?3)",
            params![room.as_str(), signed(next)?, bytes],
        )
        .map_err(unavailable)?;
        tx.commit().map_err(unavailable)?;
        Ok(next)
    }
    fn compact(
        &self,
        room: &RoomId,
        expected: u64,
        epoch: u64,
        snapshot: &str,
    ) -> Result<u64, Error> {
        let next = epoch
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        let revision = expected
            .checked_add(1)
            .filter(|n| *n <= MAX_SYNC_COUNTER)
            .ok_or(Error::Limit)?;
        let mut connection = self.0.lock().map_err(|_| Error::Unavailable)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(unavailable)?;
        let n = tx.execute("UPDATE sync_rooms SET revision=?1,epoch=?2,snapshot=?3 WHERE id=?4 AND revision=?5 AND epoch=?6", params![signed(revision)?, signed(next)?, snapshot, room.as_str(), signed(expected)?, signed(epoch)?]).map_err(unavailable)?;
        if n != 1 {
            return Err(Error::Conflict);
        }
        tx.execute("DELETE FROM sync_updates WHERE room=?1", [room.as_str()])
            .map_err(unavailable)?;
        tx.commit().map_err(unavailable)?;
        Ok(next)
    }
}

fn signed(value: u64) -> Result<i64, Error> {
    if value > MAX_SYNC_COUNTER {
        return Err(Error::Limit);
    }
    i64::try_from(value).map_err(|_| Error::Limit)
}
fn unsigned(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    value
        .try_into()
        .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
