//! TEST DATA. @cloud: requires VELLUM_TEST_DATABASE_URL for a disposable Postgres database.
#[test]
#[ignore = "@cloud: local Postgres service required"]
fn postgres_log_contract() {
    let url = std::env::var("VELLUM_TEST_DATABASE_URL").expect("Postgres test URL");
    let log = vellum_sync_postgres::PostgresLog::connect(&url).unwrap();
    let room = vellum_types::sync::RoomId::new(format!("contract-{}", std::process::id())).unwrap();
    vellum_contracts::suites::sync::update_log(&log, &room);
}

#[test]
#[ignore = "@cloud: local Postgres service required"]
fn two_connections_cannot_overwrite_the_same_revision_and_grants_expire() {
    use std::sync::{Arc, Barrier};
    use vellum_core::ports::{SecretString, sync::*};
    use vellum_types::sync::*;
    let url = std::env::var("VELLUM_TEST_DATABASE_URL").unwrap();
    let left = Arc::new(vellum_sync_postgres::PostgresLog::connect(&url).unwrap());
    let right = Arc::new(vellum_sync_postgres::PostgresLog::connect(&url).unwrap());
    let room = RoomId::new(format!("cas-{}", std::process::id())).unwrap();
    left.create(&room, "snapshot").unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = [left.clone(), right]
        .into_iter()
        .map(|store| {
            let barrier = barrier.clone();
            let room = room.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.append(&room, 1, 1, b"TEST DATA: contested")
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| **r == Ok(2)).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(Error::Conflict))
            .count(),
        1
    );
    assert_eq!(left.load(&room).unwrap().unwrap().updates.len(), 1);
    let key = SecretString::new("TEST DATA: never persist this capability");
    let grant = Grant {
        actor: vellum_contracts::samples::alice(),
        role: Role::Editor,
    };
    left.grant(&key, &room, &grant, 1000, None).unwrap();
    vellum_contracts::suites::sync::access(&*left, &key, &room, &grant, 1000);
    assert_eq!(left.authorize(&key, &room, 999), Ok(grant));
    assert_eq!(left.authorize(&key, &room, 1000), Err(Error::Forbidden));
    left.revoke(&key, &room).unwrap();
    assert_eq!(left.authorize(&key, &room, 0), Err(Error::Forbidden));
}

#[test]
#[ignore = "@cloud: local disposable Postgres service required"]
fn cloud_owner_entitlement_is_checked_on_every_request() {
    use vellum_core::ports::{SecretString, sync::*};
    use vellum_types::sync::*;
    let url = std::env::var("VELLUM_TEST_DATABASE_URL").unwrap();
    let mut sql = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    sql.batch_execute("CREATE TABLE IF NOT EXISTS entitlements (subscription_id TEXT PRIMARY KEY, user_id TEXT NOT NULL, product_id TEXT NOT NULL, version BIGINT NOT NULL, active BOOLEAN NOT NULL, valid_until BIGINT NOT NULL)").unwrap();
    let owner = format!("test-owner-{}", std::process::id());
    sql.execute(
        "INSERT INTO entitlements VALUES ($1,$1,'test-product',1,true,1000)",
        &[&owner],
    )
    .unwrap();
    let log = vellum_sync_postgres::PostgresLog::connect(&url)
        .unwrap()
        .require_cloud_sync(vec!["test-product".into()])
        .unwrap();
    let room = RoomId::new("test-entitlements").unwrap();
    let secret = SecretString::new("TEST DATA: entitled capability");
    let grant = Grant {
        actor: vellum_contracts::samples::alice(),
        role: Role::Editor,
    };
    log.grant(&secret, &room, &grant, 2000, Some(&owner))
        .unwrap();
    assert_eq!(log.authorize(&secret, &room, 999), Ok(grant));
    assert_eq!(log.authorize(&secret, &room, 1000), Err(Error::Forbidden));
    sql.execute(
        "UPDATE entitlements SET active=false WHERE user_id=$1",
        &[&owner],
    )
    .unwrap();
    assert_eq!(log.authorize(&secret, &room, 0), Err(Error::Forbidden));
}
