//! TEST DATA. @cloud: private S3-compatible MinIO/R2 service, never public object URLs.
#[test]
#[ignore = "@cloud: private S3 endpoint and test credentials required"]
fn snapshots_contract() {
    use vellum_core::ports::SecretString;
    let store = vellum_assets_s3::R2Snapshots::connect(
        &std::env::var("VELLUM_TEST_S3_ENDPOINT").unwrap(),
        &std::env::var("VELLUM_TEST_S3_BUCKET").unwrap(),
        SecretString::new(std::env::var("VELLUM_TEST_S3_ACCESS").unwrap()),
        SecretString::new(std::env::var("VELLUM_TEST_S3_SECRET").unwrap()),
    )
    .unwrap();
    vellum_contracts::suites::sync::snapshots(&store);
}
