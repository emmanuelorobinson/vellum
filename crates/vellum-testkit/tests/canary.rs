//! TEST DATA. Scanner fails on leaks, including across read boundaries; reports only paths.
use vellum_testkit::model::scan_canary;
#[test]
fn canary_scanner_detects_binary_logs_and_cross_buffer_matches() {
    let root = std::env::temp_dir().join(format!("vellum-canary-scan-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let marker = b"scanner-test-private-marker";
    let mut bytes = vec![0; 8190];
    bytes.extend_from_slice(marker);
    std::fs::write(root.join("leak.bin"), bytes).unwrap();
    std::fs::write(root.join("clean.log"), b"SecretString([redacted])").unwrap();
    let hits = scan_canary(std::slice::from_ref(&root), marker).unwrap();
    assert_eq!(hits, [root.join("leak.bin")]);
    std::fs::remove_dir_all(root).unwrap();
}
