//! `SessionStore` over SQLite (ADR-006): an index of sessions and an append-only event table in
//! `~/.vellum/sessions.db`, and pictures in `~/.vellum/cache/` named by the SHA-256 of their bytes.
//!
//! Events are stored as the core's JSON (`vellum_core::sessions::body_to_json`), so this adapter decides
//! nothing about their shape. The table refuses updates (a trigger), so the only ways an event leaves are
//! deleting its session or the whole database.
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use vellum_core::ports::{
    DocKey, EventSeq, ImageRef, NewEvent, NewSession, SessionEvent, SessionKind, SessionMeta,
    SessionStore, StoreError, Timestamp,
};
use vellum_core::sessions::{
    body_from_json, body_to_json, check_event, origin_from_json, origin_to_json,
};
use vellum_types::{AgentId, SessionId};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The schema this version writes (`PRAGMA user_version`).
pub const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    repo_root TEXT NOT NULL,
    path TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('engine', 'external')),
    kind_name TEXT NOT NULL,
    agent TEXT NOT NULL,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    events INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX sessions_by_doc ON sessions (repo_root, path, updated_at DESC);
CREATE TABLE events (
    session TEXT NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    at INTEGER NOT NULL,
    origin TEXT NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY (session, seq)
) WITHOUT ROWID;
CREATE TRIGGER events_append_only BEFORE UPDATE ON events
BEGIN
    SELECT RAISE(ABORT, 'session events are append-only');
END;
CREATE TABLE event_images (
    session TEXT NOT NULL,
    seq INTEGER NOT NULL,
    hash TEXT NOT NULL,
    FOREIGN KEY (session, seq) REFERENCES events (session, seq) ON DELETE CASCADE
);
CREATE INDEX event_images_by_hash ON event_images (hash);
CREATE INDEX event_images_by_event ON event_images (session, seq);
";

fn io(error: impl std::fmt::Display) -> StoreError {
    StoreError::Io(error.to_string())
}

/// Makes `dir` (and its parents) readable only by this user where the platform has modes.
fn private_dir(dir: &Path) -> Result<(), StoreError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir).map_err(io)
}

/// Pictures by content hash. Writes go to a temporary name and are renamed into place, so a reader never sees
/// half a file.
pub struct ImageCache {
    dir: PathBuf,
    temp: AtomicU64,
}

impl ImageCache {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let dir = dir.into();
        private_dir(&dir)?;
        Ok(Self {
            dir,
            temp: AtomicU64::new(0),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, image: &ImageRef) -> PathBuf {
        self.dir.join(image.as_str())
    }

    pub fn name(bytes: &[u8]) -> ImageRef {
        let digest = Sha256::digest(bytes);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        ImageRef::new(hex).expect("a SHA-256 is 64 hex digits")
    }

    pub fn put(&self, bytes: &[u8]) -> Result<ImageRef, StoreError> {
        let name = Self::name(bytes);
        let path = self.path(&name);
        if path.exists() {
            return Ok(name);
        }
        let temp = self.dir.join(format!(
            ".{}.{}.{}",
            name.as_str(),
            std::process::id(),
            self.temp.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&temp, bytes).map_err(io)?;
        fs::rename(&temp, &path).map_err(|e| {
            let _ = fs::remove_file(&temp);
            io(e)
        })?;
        Ok(name)
    }

    pub fn contains(&self, image: &ImageRef) -> bool {
        self.path(image).is_file()
    }

    pub fn get(&self, image: &ImageRef) -> Result<Option<Vec<u8>>, StoreError> {
        match fs::read(self.path(image)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(io(e)),
        }
    }

    pub fn remove(&self, image: &ImageRef) -> Result<(), StoreError> {
        match fs::remove_file(self.path(image)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(e)),
        }
    }
}

pub struct SqliteSessions {
    db: Mutex<Connection>,
    cache: ImageCache,
}

impl SqliteSessions {
    /// Opens (or creates) the database at `db` and the picture cache at `cache`.
    pub fn open(db: impl AsRef<Path>, cache: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let db = db.as_ref();
        if let Some(parent) = db.parent().filter(|p| !p.as_os_str().is_empty()) {
            private_dir(parent)?;
        }
        let conn = Connection::open(db).map_err(io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(db, fs::Permissions::from_mode(0o600)).map_err(io)?;
        }
        Self::with_connection(conn, ImageCache::open(cache)?)
    }

    fn with_connection(conn: Connection, cache: ImageCache) -> Result<Self, StoreError> {
        conn.pragma_update(None, "foreign_keys", true).map_err(io)?;
        conn.pragma_update(None, "journal_mode", "wal")
            .map_err(io)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(io)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(io)?;
        match version {
            0 => {
                conn.execute_batch(&format!(
                    "BEGIN; {SCHEMA} PRAGMA user_version = {SCHEMA_VERSION}; COMMIT;"
                ))
                .map_err(io)?;
            }
            SCHEMA_VERSION => {}
            newer => {
                return Err(StoreError::Corrupt(format!(
                    "the session database is schema {newer}; this version reads {SCHEMA_VERSION}"
                )));
            }
        }
        Ok(Self {
            db: Mutex::new(conn),
            cache,
        })
    }

    pub fn cache(&self) -> &ImageCache {
        &self.cache
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves SQLite consistent (transactions roll back), so keep going.
        self.db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn kind_columns(kind: &SessionKind) -> (&'static str, &str) {
    match kind {
        SessionKind::Engine { engine } => ("engine", engine),
        SessionKind::External { client } => ("external", client),
    }
}

const META_COLUMNS: &str =
    "id, repo_root, path, kind, kind_name, agent, title, created_at, updated_at, events";

fn meta_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<[String; 7]> {
    Ok([
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
    ])
}

fn meta_from(
    text: [String; 7],
    created_at: i64,
    updated_at: i64,
    events: i64,
) -> Result<SessionMeta, StoreError> {
    let [id, root, path, kind, name, agent, title] = text;
    let bad = |what: &str| StoreError::Corrupt(format!("session {what}"));
    Ok(SessionMeta {
        id: SessionId::new(id).map_err(|_| bad("id"))?,
        doc: DocKey::new(root, path).map_err(|_| bad("document"))?,
        kind: match kind.as_str() {
            "engine" => SessionKind::Engine { engine: name },
            "external" => SessionKind::External { client: name },
            _ => return Err(bad("kind")),
        },
        agent: AgentId::new(agent).map_err(|_| bad("agent"))?,
        title,
        created_at: Timestamp(created_at),
        updated_at: Timestamp(updated_at),
        events: u64::try_from(events).map_err(|_| bad("event count"))?,
    })
}

fn query_meta(
    conn: &Connection,
    filter: &str,
    args: impl rusqlite::Params,
) -> Result<Vec<SessionMeta>, StoreError> {
    let mut stmt = conn
        .prepare(&format!("SELECT {META_COLUMNS} FROM sessions {filter}"))
        .map_err(io)?;
    let rows = stmt
        .query_map(args, |row| {
            Ok((meta_row(row)?, row.get(7)?, row.get(8)?, row.get(9)?))
        })
        .map_err(io)?;
    rows.map(|row| {
        let (text, created, updated, events) = row.map_err(io)?;
        meta_from(text, created, updated, events)
    })
    .collect()
}

impl SessionStore for SqliteSessions {
    fn create(&self, session: NewSession) -> Result<(), StoreError> {
        let conn = self.lock();
        let (kind, name) = kind_columns(&session.kind);
        let inserted = conn
            .execute(
                "INSERT OR IGNORE INTO sessions
                 (id, repo_root, path, kind, kind_name, agent, title, created_at, updated_at, events)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, 0)",
                params![
                    session.id.as_str(),
                    session.doc.repo_root(),
                    session.doc.path(),
                    kind,
                    name,
                    session.agent.as_str(),
                    session.title,
                    session.created_at.0,
                ],
            )
            .map_err(io)?;
        if inserted == 0 {
            return Err(StoreError::Exists);
        }
        Ok(())
    }

    fn sessions(&self, doc: &DocKey) -> Result<Vec<SessionMeta>, StoreError> {
        query_meta(
            &self.lock(),
            "WHERE repo_root = ?1 AND path = ?2 ORDER BY updated_at DESC, id DESC",
            params![doc.repo_root(), doc.path()],
        )
    }

    fn session(&self, id: &SessionId) -> Result<Option<SessionMeta>, StoreError> {
        Ok(
            query_meta(&self.lock(), "WHERE id = ?1", params![id.as_str()])?
                .into_iter()
                .next(),
        )
    }

    fn rename(&self, id: &SessionId, title: &str) -> Result<(), StoreError> {
        let changed = self
            .lock()
            .execute(
                "UPDATE sessions SET title = ?2 WHERE id = ?1",
                params![id.as_str(), title],
            )
            .map_err(io)?;
        if changed == 0 {
            return Err(StoreError::UnknownSession);
        }
        Ok(())
    }

    fn append(&self, session: &SessionId, event: NewEvent) -> Result<EventSeq, StoreError> {
        let mut conn = self.lock();
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(io)?;
        let row: Option<(String, String, i64)> = tx
            .query_row(
                "SELECT kind, kind_name, events FROM sessions WHERE id = ?1",
                params![session.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(io)?;
        let (kind, name, count) = row.ok_or(StoreError::UnknownSession)?;
        let kind = match kind.as_str() {
            "engine" => SessionKind::Engine { engine: name },
            _ => SessionKind::External { client: name },
        };
        check_event(session, &kind, &event)?;
        let images = event.body.images();
        if let Some(missing) = images.iter().find(|image| !self.cache.contains(image)) {
            return Err(StoreError::MissingImage((*missing).clone()));
        }
        let seq = count + 1;
        tx.execute(
            "INSERT INTO events (session, seq, at, origin, body) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session.as_str(),
                seq,
                event.at.0,
                origin_to_json(&event.origin).to_string(),
                body_to_json(&event.body).to_string(),
            ],
        )
        .map_err(io)?;
        for image in images {
            tx.execute(
                "INSERT INTO event_images (session, seq, hash) VALUES (?1, ?2, ?3)",
                params![session.as_str(), seq, image.as_str()],
            )
            .map_err(io)?;
        }
        tx.execute(
            "UPDATE sessions SET events = ?2, updated_at = max(updated_at, ?3) WHERE id = ?1",
            params![session.as_str(), seq, event.at.0],
        )
        .map_err(io)?;
        tx.commit().map_err(io)?;
        Ok(EventSeq(seq as u64))
    }

    fn events(
        &self,
        session: &SessionId,
        since: Option<EventSeq>,
    ) -> Result<Vec<SessionEvent>, StoreError> {
        let conn = self.lock();
        let known: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM sessions WHERE id = ?1",
                params![session.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(io)?;
        if known.is_none() {
            return Err(StoreError::UnknownSession);
        }
        let after = i64::try_from(since.map_or(0, |s| s.0)).unwrap_or(i64::MAX);
        let mut stmt = conn
            .prepare(
                "SELECT seq, at, origin, body FROM events WHERE session = ?1 AND seq > ?2 ORDER BY seq",
            )
            .map_err(io)?;
        let rows = stmt
            .query_map(params![session.as_str(), after], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(io)?;
        rows.map(|row| {
            let (seq, at, origin, body) = row.map_err(io)?;
            let parse = |text: &str| {
                serde_json::from_str(text)
                    .map_err(|_| StoreError::Corrupt("an event is not JSON".into()))
            };
            Ok(SessionEvent {
                seq: EventSeq(seq as u64),
                at: Timestamp(at),
                origin: origin_from_json(parse(&origin)?)?,
                body: body_from_json(&parse(&body)?)?,
            })
        })
        .collect()
    }

    fn put_image(&self, bytes: &[u8]) -> Result<ImageRef, StoreError> {
        self.cache.put(bytes)
    }

    fn image(&self, image: &ImageRef) -> Result<Option<Vec<u8>>, StoreError> {
        self.cache.get(image)
    }

    fn delete(&self, id: &SessionId) -> Result<(), StoreError> {
        let mut conn = self.lock();
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(io)?;
        let hashes: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT DISTINCT hash FROM event_images WHERE session = ?1")
                .map_err(io)?;
            stmt.query_map(params![id.as_str()], |row| row.get(0))
                .map_err(io)?
                .collect::<Result<_, _>>()
                .map_err(io)?
        };
        tx.execute("DELETE FROM sessions WHERE id = ?1", params![id.as_str()])
            .map_err(io)?;
        let mut orphans = vec![];
        for hash in hashes {
            let named: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM event_images WHERE hash = ?1 LIMIT 1",
                    params![hash],
                    |row| row.get(0),
                )
                .optional()
                .map_err(io)?;
            if named.is_none() {
                orphans.extend(ImageRef::new(hash));
            }
        }
        tx.commit().map_err(io)?;
        // After the commit: a crash here leaves an unnamed picture behind, never an event naming a missing one.
        for image in orphans {
            self.cache.remove(&image)?;
        }
        Ok(())
    }
}
