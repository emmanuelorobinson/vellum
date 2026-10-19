//! TEST DATA. Local CLI contract; no keychain access, daemon or network.
use std::process::Command;
#[test]
fn account_commands_are_explicit_and_invalid_origins_are_payload_free() {
    for command in ["login", "logout", "whoami"] {
        let out = Command::new(env!("CARGO_BIN_EXE_vellum"))
            .args([command, "--help"])
            .output()
            .unwrap();
        assert!(out.status.success());
    }
    let out = Command::new(env!("CARGO_BIN_EXE_vellum"))
        .args([
            "login",
            "--account-url",
            "https://person:private-data@example.test",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(!text.contains("private-data"));
    assert!(text.contains("HTTPS account origin"));
}
