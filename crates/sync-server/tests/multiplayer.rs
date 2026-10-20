//! TEST DATA. @network parity scenario: two people and two agents over real authenticated WebSockets.
mod support;
use std::sync::Arc;
use vellum_sync_server::{SyncService, memory::*};
use vellum_types::sync::*;
#[test]
#[ignore = "@network: loopback listener required"]
fn four_actors_share_presence_edits_and_comments_on_local_storage() {
    let path = std::env::temp_dir().join(format!("vellum-p64-local-{}", std::process::id()));
    let snapshots = Arc::new(vellum_sync_disk::DiskSnapshots::open(&path).unwrap());
    let database = Arc::new(vellum_sync_sqlite::SqliteLog::open(path.join("sync.sqlite")).unwrap());
    let room = RoomId::new("four-actors").unwrap();
    let keys = support::keys();
    for (key, actor) in keys.iter().zip(support::actors()) {
        database
            .grant(
                key,
                &room,
                &Grant {
                    actor,
                    role: Role::Editor,
                },
                i64::MAX,
            )
            .unwrap();
    }
    let service = Arc::new(SyncService::new(database.clone(), snapshots, database));
    service
        .seed(&room, &vellum_contracts::samples::document())
        .unwrap();
    let server = vellum_sync_ws::Server::start(
        "127.0.0.1:0".parse().unwrap(),
        service,
        Some(Arc::new(MemoryPresence::default())),
        "local-parity".into(),
        vec![],
    )
    .unwrap();
    support::scenario(server.address(), &room, &keys);
    server.stop();
    std::fs::remove_dir_all(path).unwrap();
}
#[test]
#[ignore = "@cloud: disposable Postgres/Redis/private S3-compatible test bucket required"]
fn identical_four_actor_scenario_on_cloud_storage() {
    use vellum_core::ports::SecretString;
    let database = Arc::new(
        vellum_sync_postgres::PostgresLog::connect(
            &std::env::var("VELLUM_TEST_DATABASE_URL").unwrap(),
        )
        .unwrap(),
    );
    let snapshots = Arc::new(
        vellum_assets_s3::R2Snapshots::connect(
            &std::env::var("VELLUM_TEST_S3_ENDPOINT").unwrap(),
            &std::env::var("VELLUM_TEST_S3_BUCKET").unwrap(),
            SecretString::new(std::env::var("VELLUM_TEST_S3_ACCESS").unwrap()),
            SecretString::new(std::env::var("VELLUM_TEST_S3_SECRET").unwrap()),
        )
        .unwrap(),
    );
    let presence = Arc::new(
        vellum_sync_redis::RedisPresence::connect(&std::env::var("VELLUM_TEST_REDIS_URL").unwrap())
            .unwrap(),
    );
    let room = RoomId::new(format!("cloud-four-{}", std::process::id())).unwrap();
    let keys = support::keys();
    for (key, actor) in keys.iter().zip(support::actors()) {
        database
            .grant(
                key,
                &room,
                &Grant {
                    actor,
                    role: Role::Editor,
                },
                i64::MAX,
                None,
            )
            .unwrap();
    }
    let service = Arc::new(SyncService::new(database.clone(), snapshots, database));
    service
        .seed(&room, &vellum_contracts::samples::document())
        .unwrap();
    let server = vellum_sync_ws::Server::start(
        "127.0.0.1:0".parse().unwrap(),
        service,
        Some(presence),
        "cloud-parity".into(),
        vec![],
    )
    .unwrap();
    support::scenario(server.address(), &room, &keys);
    server.stop();
}
