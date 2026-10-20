//! TEST DATA.
use vellum_core::ports::{SecretString, sync::*};
use vellum_sync_sqlite::SqliteLog;
use vellum_types::sync::*;
#[test]
fn sqlite_log_contract() {
    vellum_contracts::suites::sync::update_log(
        &SqliteLog::open(":memory:").unwrap(),
        &RoomId::new("contract").unwrap(),
    );
}
#[test]
fn grants_expire_revoke_and_never_persist_raw_tokens() {
    let root = std::env::temp_dir().join(format!("vellum-sync-sqlite-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("test.sqlite");
    let log = SqliteLog::open(&path).unwrap();
    let key = SecretString::new("TEST DATA: ephemeral credential never in database");
    let room = RoomId::new("grants").unwrap();
    let grant = Grant {
        actor: vellum_contracts::samples::alice(),
        role: Role::Editor,
    };
    log.grant(&key, &room, &grant, 1000).unwrap();
    vellum_contracts::suites::sync::access(&log, &key, &room, &grant, 1000);
    assert_eq!(log.authorize(&key, &room, 999).unwrap(), grant);
    assert_eq!(log.authorize(&key, &room, 1000), Err(Error::Forbidden));
    log.revoke(&key, &room).unwrap();
    assert_eq!(log.authorize(&key, &room, 0), Err(Error::Forbidden));
    drop(log);
    for entry in std::fs::read_dir(&root).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(
            !bytes
                .windows(key.expose().len())
                .any(|w| w == key.expose().as_bytes())
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
