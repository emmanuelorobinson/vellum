//! Postgres transactions serialize each room head. Connect only to a private local network or TLS tunnel.
use postgres::{Client, IsolationLevel, NoTls};
use sha2::{Digest, Sha256};
use std::{sync::Mutex, time::Duration};
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS vellum_sync_rooms (id TEXT PRIMARY KEY, epoch BIGINT NOT NULL, revision BIGINT NOT NULL, snapshot TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS vellum_sync_updates (room TEXT NOT NULL REFERENCES vellum_sync_rooms(id), revision BIGINT NOT NULL, bytes BYTEA NOT NULL, PRIMARY KEY(room,revision));
CREATE TABLE IF NOT EXISTS vellum_sync_grants (digest TEXT NOT NULL, room TEXT NOT NULL, grant_json TEXT NOT NULL, expires_ms BIGINT NOT NULL, owner_id TEXT, PRIMARY KEY(digest,room));";
pub struct PostgresLog {
    client: Mutex<Client>,
    products: Vec<String>,
}
fn unavailable(_: postgres::Error) -> Error {
    Error::Unavailable
}
fn signed(value: u64) -> Result<i64, Error> {
    if value > MAX_SYNC_COUNTER {
        return Err(Error::Limit);
    }
    i64::try_from(value).map_err(|_| Error::Limit)
}
fn digest(secret: &SecretString) -> String {
    Sha256::digest(secret.expose().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
impl PostgresLog {
    pub fn connect(url: &str) -> Result<Self, Error> {
        let mut config: postgres::Config = url.parse().map_err(unavailable)?;
        config.connect_timeout(Duration::from_secs(5));
        let mut client = config.connect(NoTls).map_err(unavailable)?;
        client
            .batch_execute("SET statement_timeout = '5s'; SET lock_timeout = '5s';")
            .map_err(unavailable)?;
        let mut migration = client.transaction().map_err(unavailable)?;
        migration
            .query_one("SELECT pg_advisory_xact_lock($1)", &[&6_332_701_i64])
            .map_err(unavailable)?;
        migration.batch_execute(SCHEMA).map_err(unavailable)?;
        migration.commit().map_err(unavailable)?;
        Ok(Self {
            client: Mutex::new(client),
            products: vec![],
        })
    }
    pub fn require_cloud_sync(mut self, products: Vec<String>) -> Result<Self, Error> {
        if products.is_empty() || products.iter().any(|p| p.is_empty()) {
            return Err(Error::InvalidUpdate);
        }
        self.products = products;
        Ok(self)
    }
    pub fn grant(
        &self,
        secret: &SecretString,
        room: &RoomId,
        grant: &Grant,
        expires_ms: i64,
        owner_id: Option<&str>,
    ) -> Result<(), Error> {
        let json = serde_json::to_string(grant).map_err(|_| Error::InvalidUpdate)?;
        self.client.lock().map_err(|_| Error::Unavailable)?.execute("INSERT INTO vellum_sync_grants VALUES ($1,$2,$3,$4,$5) ON CONFLICT(digest,room) DO UPDATE SET grant_json=excluded.grant_json, expires_ms=excluded.expires_ms, owner_id=excluded.owner_id", &[&digest(secret), &room.as_str(), &json, &expires_ms, &owner_id]).map_err(unavailable)?;
        Ok(())
    }
    pub fn revoke(&self, secret: &SecretString, room: &RoomId) -> Result<(), Error> {
        self.client
            .lock()
            .map_err(|_| Error::Unavailable)?
            .execute(
                "DELETE FROM vellum_sync_grants WHERE digest=$1 AND room=$2",
                &[&digest(secret), &room.as_str()],
            )
            .map_err(unavailable)?;
        Ok(())
    }
}
impl AccessControl for PostgresLog {
    fn authorize(&self, secret: &SecretString, room: &RoomId, now_ms: i64) -> Result<Grant, Error> {
        let mut client = self.client.lock().map_err(|_| Error::Unavailable)?;
        let row = client.query_opt("SELECT grant_json,owner_id FROM vellum_sync_grants WHERE digest=$1 AND room=$2 AND expires_ms>$3", &[&digest(secret), &room.as_str(), &now_ms]).map_err(unavailable)?.ok_or(Error::Forbidden)?;
        if !self.products.is_empty() {
            let owner: Option<String> = row.get(1);
            let owner = owner.ok_or(Error::Forbidden)?;
            let allowed = client.query_opt("SELECT 1 FROM entitlements WHERE user_id=$1 AND active AND valid_until>$2 AND product_id=ANY($3) LIMIT 1", &[&owner, &now_ms, &self.products]).map_err(unavailable)?;
            if allowed.is_none() {
                return Err(Error::Forbidden);
            }
        }
        serde_json::from_str(row.get(0)).map_err(|_| Error::Unavailable)
    }
}
impl UpdateLog for PostgresLog {
    fn load(&self, room: &RoomId) -> Result<Option<RoomLog>, Error> {
        let mut client = self.client.lock().map_err(|_| Error::Unavailable)?;
        let mut tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .map_err(unavailable)?;
        let Some(row) = tx
            .query_opt(
                "SELECT epoch,revision,snapshot FROM vellum_sync_rooms WHERE id=$1",
                &[&room.as_str()],
            )
            .map_err(unavailable)?
        else {
            return Ok(None);
        };
        let epoch: i64 = row.get(0);
        let revision: i64 = row.get(1);
        let updates = tx
            .query(
                "SELECT revision,bytes FROM vellum_sync_updates WHERE room=$1 ORDER BY revision",
                &[&room.as_str()],
            )
            .map_err(unavailable)?
            .into_iter()
            .map(|row| {
                let r: i64 = row.get(0);
                Ok(LoggedUpdate {
                    revision: u64::try_from(r).map_err(|_| Error::InvalidUpdate)?,
                    bytes: row.get(1),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Some(RoomLog {
            epoch: u64::try_from(epoch).map_err(|_| Error::InvalidUpdate)?,
            revision: u64::try_from(revision).map_err(|_| Error::InvalidUpdate)?,
            snapshot: row.get(2),
            updates,
        }))
    }
    fn create(&self, room: &RoomId, snapshot: &str) -> Result<(), Error> {
        let n = self
            .client
            .lock()
            .map_err(|_| Error::Unavailable)?
            .execute(
                "INSERT INTO vellum_sync_rooms VALUES ($1,1,1,$2) ON CONFLICT DO NOTHING",
                &[&room.as_str(), &snapshot],
            )
            .map_err(unavailable)?;
        if n == 1 { Ok(()) } else { Err(Error::Conflict) }
    }
    fn append(&self, room: &RoomId, expected: u64, epoch: u64, bytes: &[u8]) -> Result<u64, Error> {
        let next = expected.checked_add(1).ok_or(Error::Limit)?;
        let mut client = self.client.lock().map_err(|_| Error::Unavailable)?;
        let mut tx = client.transaction().map_err(unavailable)?;
        let n = tx
            .execute(
                "UPDATE vellum_sync_rooms SET revision=$1 WHERE id=$2 AND revision=$3 AND epoch=$4",
                &[
                    &signed(next)?,
                    &room.as_str(),
                    &signed(expected)?,
                    &signed(epoch)?,
                ],
            )
            .map_err(unavailable)?;
        if n != 1 {
            return Err(Error::Conflict);
        }
        tx.execute(
            "INSERT INTO vellum_sync_updates VALUES ($1,$2,$3)",
            &[&room.as_str(), &signed(next)?, &bytes],
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
        let next = epoch.checked_add(1).ok_or(Error::Limit)?;
        let revision = expected.checked_add(1).ok_or(Error::Limit)?;
        let mut client = self.client.lock().map_err(|_| Error::Unavailable)?;
        let mut tx = client.transaction().map_err(unavailable)?;
        let n = tx.execute("UPDATE vellum_sync_rooms SET revision=$1,epoch=$2,snapshot=$3 WHERE id=$4 AND revision=$5 AND epoch=$6", &[&signed(revision)?, &signed(next)?, &snapshot, &room.as_str(), &signed(expected)?, &signed(epoch)?]).map_err(unavailable)?;
        if n != 1 {
            return Err(Error::Conflict);
        }
        tx.execute(
            "DELETE FROM vellum_sync_updates WHERE room=$1",
            &[&room.as_str()],
        )
        .map_err(unavailable)?;
        tx.commit().map_err(unavailable)?;
        Ok(next)
    }
}
