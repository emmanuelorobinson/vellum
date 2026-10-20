//! Postgres transactions serialize each room head. Connect only to a private local network or TLS tunnel.
use postgres::{Client, IsolationLevel, NoTls};
use sha2::{Digest, Sha256};
use std::{sync::Mutex, time::Duration};
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS vellum_sync_rooms (id TEXT PRIMARY KEY, epoch BIGINT NOT NULL, revision BIGINT NOT NULL, snapshot TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS vellum_sync_updates (room TEXT NOT NULL REFERENCES vellum_sync_rooms(id), revision BIGINT NOT NULL, bytes BYTEA NOT NULL, PRIMARY KEY(room,revision));
CREATE TABLE IF NOT EXISTS vellum_sync_grants (digest TEXT NOT NULL, room TEXT NOT NULL, grant_json TEXT NOT NULL, expires_ms BIGINT NOT NULL, owner_id TEXT, PRIMARY KEY(digest,room));
CREATE TABLE IF NOT EXISTS vellum_sync_owners (room TEXT PRIMARY KEY, actor TEXT NOT NULL, enabled BOOLEAN NOT NULL DEFAULT TRUE);
CREATE INDEX IF NOT EXISTS vellum_sync_grants_by_room ON vellum_sync_grants(room,expires_ms);";
pub struct PostgresLog {
    client: Mutex<Client>,
    products: Vec<String>,
    team_products: Vec<String>,
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
            team_products: vec![],
        })
    }
    pub fn require_cloud_sync(mut self, products: Vec<String>) -> Result<Self, Error> {
        if products.is_empty() || products.iter().any(|p| p.is_empty()) {
            return Err(Error::InvalidUpdate);
        }
        self.products = products;
        Ok(self)
    }
    pub fn require_team_collaboration(mut self, products: Vec<String>) -> Result<Self, Error> {
        if products.is_empty() || products.iter().any(|p| p.is_empty()) {
            return Err(Error::InvalidUpdate);
        }
        self.team_products = products;
        Ok(self)
    }
    /// Trusted provisioning only.
    pub fn set_owner(&self, room: &RoomId, actor: &vellum_types::Actor) -> Result<(), Error> {
        if !actor.is_person() {
            return Err(Error::InvalidUpdate);
        }
        let actor = serde_json::to_string(actor).map_err(|_| Error::InvalidUpdate)?;
        self.client.lock().map_err(|_|Error::Unavailable)?.execute("INSERT INTO vellum_sync_owners(room,actor) VALUES ($1,$2) ON CONFLICT(room) DO UPDATE SET actor=excluded.actor",&[&room.as_str(),&actor]).map_err(unavailable)?;
        Ok(())
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
    fn authorize_collaboration(
        &self,
        secret: &SecretString,
        room: &RoomId,
        now_ms: i64,
    ) -> Result<Grant, Error> {
        let grant = self.authorize(secret, room, now_ms)?;
        if self.products.is_empty() {
            return Ok(grant);
        }
        if self.team_products.is_empty() {
            return Err(Error::Forbidden);
        }
        let mut client = self.client.lock().map_err(|_| Error::Unavailable)?;
        let row=client.query_opt("SELECT owner_id FROM vellum_sync_grants WHERE digest=$1 AND room=$2 AND expires_ms>$3",&[&digest(secret),&room.as_str(),&now_ms]).map_err(unavailable)?.ok_or(Error::Forbidden)?;
        let owner: Option<String> = row.get(0);
        let owner = owner.ok_or(Error::Forbidden)?;
        let allowed=client.query_opt("SELECT 1 FROM entitlements WHERE user_id=$1 AND active AND valid_until>$2 AND product_id=ANY($3) LIMIT 1",&[&owner,&now_ms,&self.team_products]).map_err(unavailable)?;
        if allowed.is_none() {
            return Err(Error::Forbidden);
        }
        Ok(grant)
    }
    fn manage(
        &self,
        secret: &SecretString,
        room: &RoomId,
        command: SharingCommand,
        now_ms: i64,
    ) -> Result<SharingState, Error> {
        self.authorize(secret, room, now_ms)?;
        let can_live = self.authorize_collaboration(secret, room, now_ms).is_ok();
        let mut client = self.client.lock().map_err(|_| Error::Unavailable)?;
        let mut tx = client.transaction().map_err(unavailable)?;
        let row=tx.query_opt("SELECT grant_json FROM vellum_sync_grants WHERE digest=$1 AND room=$2 AND expires_ms>$3 FOR SHARE",&[&digest(secret),&room.as_str(),&now_ms]).map_err(unavailable)?.ok_or(Error::Forbidden)?;
        let grant: Grant = serde_json::from_str(row.get(0)).map_err(|_| Error::Unavailable)?;
        let row = tx
            .query_opt(
                "SELECT actor,enabled FROM vellum_sync_owners WHERE room=$1 FOR UPDATE",
                &[&room.as_str()],
            )
            .map_err(unavailable)?
            .ok_or(Error::NotFound)?;
        let owner_actor: String = row.get(0);
        let mut enabled: bool = row.get(1);
        let owner =
            owner_actor == serde_json::to_string(&grant.actor).map_err(|_| Error::InvalidUpdate)?;
        if !owner && (!enabled || !matches!(command, SharingCommand::Read)) {
            return Err(Error::Forbidden);
        }
        match command {
            SharingCommand::Read => {}
            SharingCommand::SetLive { enabled: value } => {
                if value && !can_live {
                    return Err(Error::Forbidden);
                }
                tx.execute(
                    "UPDATE vellum_sync_owners SET enabled=$1 WHERE room=$2",
                    &[&value, &room.as_str()],
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
                tx.execute("UPDATE vellum_sync_grants SET grant_json=$1 WHERE room=$2 AND grant_json::jsonb->'actor'=$3::text::jsonb",&[&updated,&room.as_str(),&target]).map_err(unavailable)?;
            }
            SharingCommand::Remove { actor: target } => {
                let target = serde_json::to_string(&target).map_err(|_| Error::InvalidUpdate)?;
                if target == owner_actor {
                    return Err(Error::Forbidden);
                }
                tx.execute("DELETE FROM vellum_sync_grants WHERE room=$1 AND grant_json::jsonb->'actor'=$2::text::jsonb",&[&room.as_str(),&target]).map_err(unavailable)?;
            }
        }
        let rows=tx.query("SELECT DISTINCT grant_json FROM vellum_sync_grants WHERE room=$1 AND expires_ms>$2 ORDER BY grant_json LIMIT 1025",&[&room.as_str(),&now_ms]).map_err(unavailable)?;
        if rows.len() > 1024 {
            return Err(Error::Limit);
        }
        let members = rows
            .into_iter()
            .map(|r| serde_json::from_str::<Grant>(r.get(0)).map_err(|_| Error::Unavailable))
            .collect::<Result<Vec<_>, _>>()?;
        tx.commit().map_err(unavailable)?;
        Ok(SharingState {
            owner_actor: serde_json::from_str(&owner_actor).map_err(|_| Error::Unavailable)?,
            owner,
            enabled,
            can_live,
            members,
        })
    }
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
        let grant: Grant = serde_json::from_str(row.get(0)).map_err(|_| Error::Unavailable)?;
        if let Some(row) = client
            .query_opt(
                "SELECT actor,enabled FROM vellum_sync_owners WHERE room=$1",
                &[&room.as_str()],
            )
            .map_err(unavailable)?
        {
            let owner: String = row.get(0);
            let enabled: bool = row.get(1);
            if !enabled
                && owner != serde_json::to_string(&grant.actor).map_err(|_| Error::Unavailable)?
            {
                return Err(Error::Forbidden);
            }
        }
        Ok(grant)
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
