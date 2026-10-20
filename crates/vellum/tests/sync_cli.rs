//! TEST DATA. Argument parsing only; no keychain, listener or account access.
#[test]
fn bundled_binary_exposes_the_native_sync_bridge() {
    let help = std::process::Command::new(env!("CARGO_BIN_EXE_vellum"))
        .args(["sync", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for flag in ["--endpoint", "--room", "--key"] {
        assert!(text.contains(flag));
    }
    let missing = std::process::Command::new(env!("CARGO_BIN_EXE_vellum"))
        .args(["sync"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
}
