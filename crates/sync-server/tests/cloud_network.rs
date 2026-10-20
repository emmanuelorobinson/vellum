//! TEST DATA. @cloud: disposable Postgres/Redis and private S3-compatible test bucket.
use std::sync::Arc;
use vellum_contracts::{fakes::SequentialIds, samples::*};
use vellum_core::{
    ops,
    ports::{DocStore, SecretString, SyncCursor},
};
use vellum_sync_server::SyncService;
use vellum_types::sync::*;
#[test]
#[ignore = "@cloud: VELLUM_TEST_DATABASE_URL, REDIS_URL and S3 test configuration required"]
fn websocket_uses_postgres_redis_and_private_s3_together() {
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
    let room = RoomId::new(format!("production-ports-{}", std::process::id())).unwrap();
    let key = SecretString::new("TEST-DATA-native-storage-capability");
    database
        .grant(
            &key,
            &room,
            &Grant {
                actor: alice(),
                role: Role::Editor,
            },
            i64::MAX,
            None,
        )
        .unwrap();
    let service = Arc::new(SyncService::new(database.clone(), snapshots, database));
    service.seed(&room, &document()).unwrap();
    let server = vellum_sync_ws::Server::start(
        "127.0.0.1:0".parse().unwrap(),
        service.clone(),
        Some(presence),
        "test-storage-instance".into(),
        vec![],
    )
    .unwrap();
    let mut socket =
        vellum_sync_ws::testing::Connection::connect(server.address(), &room, &key).unwrap();
    socket
        .send(&ClientFrame {
            v: 1,
            adapter: SYNC_ADAPTER.into(),
            epoch: 0,
            request: SyncRequest::Pull { after: 0 },
        })
        .unwrap();
    let SyncReply::State {
        snapshot: Some(bytes),
        ..
    } = socket.read().unwrap().body
    else {
        panic!("S3 bootstrap");
    };
    let doc = vellum_codec_json::JsonCodec::new().decode(&bytes).unwrap();
    let mut client = vellum_doc_loro::LoroDocStore::new(&doc, 888);
    ops::apply(
        &mut client,
        &mut SequentialIds::new(),
        tx(
            &alice(),
            vec![rename("rect000001", "TEST DATA: production ports")],
        ),
    )
    .unwrap();
    socket
        .send(&ClientFrame {
            v: 1,
            adapter: SYNC_ADAPTER.into(),
            epoch: 1,
            request: SyncRequest::Push {
                bytes: client.export_updates(&SyncCursor::empty()).bytes,
            },
        })
        .unwrap();
    assert!(matches!(socket.read().unwrap().body, SyncReply::Ack));
    assert_eq!(service.compact(&key, &room, 1, 0), Ok(2));
    let mut canonical = None;
    for _ in 0..8 {
        let frame = socket.read().unwrap();
        if frame.epoch == 2
            && let SyncReply::State {
                snapshot: Some(bytes),
                ..
            } = frame.body
        {
            canonical = Some(bytes);
            break;
        }
    }
    let rebuilt = vellum_codec_json::JsonCodec::new()
        .decode(&canonical.expect("new S3 checkpoint"))
        .unwrap();
    assert_eq!(
        find(&rebuilt, "rect000001").unwrap().name(),
        "TEST DATA: production ports"
    );
    socket.close().unwrap();
    server.stop();
}
