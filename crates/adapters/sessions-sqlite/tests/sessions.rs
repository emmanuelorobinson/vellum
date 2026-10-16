//! The SQLite session store: the contract, persistence, append-only events, the picture cache. TEST DATA.
mod sessions {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use vellum_contracts::suites::{
        self,
        sessions::{doc, new_session, session_id},
    };
    use vellum_core::ports::*;
    use vellum_sessions_sqlite::{ImageCache, SCHEMA_VERSION, SqliteSessions};
    use vellum_types::{Actor, AgentId, Origin};

    fn scratch() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vellum-sessions-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }
    fn open(dir: &Path) -> SqliteSessions {
        SqliteSessions::open(dir.join("sessions.db"), dir.join("cache")).unwrap()
    }
    fn builtin() -> SessionKind {
        SessionKind::Engine {
            engine: "builtin".into(),
        }
    }
    fn step(thumbnail: Option<ImageRef>) -> EventBody {
        EventBody::Step(StepRecord {
            tool: "set_properties".into(),
            arguments: "{}".into(),
            result: "{}".into(),
            thumbnail,
            started_at: Timestamp(1),
            duration_ms: 2,
            outcome: StepOutcome::Failed,
            detail: serde_json::json!({}),
        })
    }
    fn by_agent(n: u32) -> Origin {
        Origin::new(Actor::Agent {
            agent: AgentId::new(format!("01J{:023}", 800)).unwrap(),
        })
        .in_session(session_id(n))
    }

    #[test]
    fn passes_the_contract() {
        suites::sessions::run(|| open(&scratch()));
    }

    #[test]
    fn pictures_are_named_by_sha256() {
        let dir = scratch();
        let store = open(&dir);
        let abc = store.put_image(b"abc").unwrap();
        assert_eq!(
            abc.as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            std::fs::read(dir.join("cache").join(abc.as_str())).unwrap(),
            b"abc"
        );
        assert_eq!(ImageCache::name(b"abc"), abc);
        // No temporary files are left behind.
        let names: Vec<_> = std::fs::read_dir(dir.join("cache"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn survives_reopening() {
        let dir = scratch();
        {
            let store = open(&dir);
            store
                .create(new_session(1, doc("a.vellum"), builtin(), 5))
                .unwrap();
            let picture = store.put_image(b"TEST DATA thumb").unwrap();
            store
                .append(
                    &session_id(1),
                    NewEvent {
                        at: Timestamp(6),
                        origin: by_agent(1),
                        body: step(Some(picture)),
                    },
                )
                .unwrap();
        }
        let store = open(&dir);
        let events = store.events(&session_id(1), None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].body,
            step(Some(ImageCache::name(b"TEST DATA thumb")))
        );
        assert_eq!(
            store.sessions(&doc("a.vellum")).unwrap()[0].updated_at,
            Timestamp(6)
        );
        assert_eq!(
            store.append(
                &session_id(1),
                NewEvent {
                    at: Timestamp(7),
                    origin: by_agent(1),
                    body: step(None)
                }
            ),
            Ok(EventSeq(2))
        );
    }

    #[test]
    fn events_cannot_be_rewritten_and_delete_cascades() {
        let dir = scratch();
        let store = open(&dir);
        store
            .create(new_session(1, doc("a.vellum"), builtin(), 0))
            .unwrap();
        let picture = store.put_image(b"TEST DATA p").unwrap();
        store
            .append(
                &session_id(1),
                NewEvent {
                    at: Timestamp(1),
                    origin: by_agent(1),
                    body: step(Some(picture)),
                },
            )
            .unwrap();
        let raw = rusqlite::Connection::open(dir.join("sessions.db")).unwrap();
        let rewrite = raw.execute("UPDATE events SET body = '{}'", []);
        assert!(rewrite.unwrap_err().to_string().contains("append-only"));
        store.delete(&session_id(1)).unwrap();
        for table in ["sessions", "events", "event_images"] {
            let n: i64 = raw
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 0, "{table} after delete");
        }
        assert_eq!(std::fs::read_dir(dir.join("cache")).unwrap().count(), 0);
    }

    #[test]
    fn refuses_a_newer_schema() {
        let dir = scratch();
        drop(open(&dir));
        let raw = rusqlite::Connection::open(dir.join("sessions.db")).unwrap();
        raw.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(raw);
        let error = SqliteSessions::open(dir.join("sessions.db"), dir.join("cache"))
            .err()
            .unwrap();
        assert!(matches!(error, StoreError::Corrupt(m) if m.contains("schema 2")));
    }

    #[test]
    fn damaged_rows_are_reported_not_guessed() {
        let dir = scratch();
        let store = open(&dir);
        store
            .create(new_session(1, doc("a.vellum"), builtin(), 0))
            .unwrap();
        let raw = rusqlite::Connection::open(dir.join("sessions.db")).unwrap();
        raw.execute(
            "INSERT INTO events (session, seq, at, origin, body) VALUES (?1, 1, 0, ?2, '{\"type\":\"chat\"}')",
            rusqlite::params![session_id(1).as_str(), serde_json::to_string(&by_agent(1)).unwrap()],
        ).unwrap();
        assert!(matches!(
            store.events(&session_id(1), None),
            Err(StoreError::Corrupt(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn only_this_user_can_read_it() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch();
        drop(open(&dir));
        let mode = |p: PathBuf| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(dir.join("sessions.db")), 0o600);
        assert_eq!(mode(dir.join("cache")), 0o700);
        assert_eq!(mode(dir.clone()), 0o700);
    }
}
