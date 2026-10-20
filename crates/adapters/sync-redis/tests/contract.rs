//! TEST DATA. @cloud: requires VELLUM_TEST_REDIS_URL for a disposable Redis instance.
#[test]
#[ignore = "@cloud: local Redis service required"]
fn redis_presence_contract() {
    let url = std::env::var("VELLUM_TEST_REDIS_URL").expect("Redis test URL");
    let store = vellum_sync_redis::RedisPresence::connect(&url).unwrap();
    let room = vellum_types::sync::RoomId::new(format!("contract-{}", std::process::id())).unwrap();
    vellum_contracts::suites::sync::presence(&store, &room);
}
