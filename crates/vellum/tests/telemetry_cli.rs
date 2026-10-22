//! TEST DATA: isolated consent state, no daemon, keychain or network.
use std::{path::PathBuf, process::Command};
struct State(PathBuf);
impl Drop for State {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn telemetry_is_explicit_persistent_and_payload_free_on_invalid_input() {
    let state =
        State(std::env::temp_dir().join(format!("vellum-telemetry-{}", std::process::id())));
    std::fs::create_dir(&state.0).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_vellum"))
            .arg("--run-dir")
            .arg(state.0.join("run"))
            .args(["--json", "telemetry"])
            .args(args)
            .output()
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        (output.status.success(), value)
    };
    let (ok, status) = run(&["status"]);
    assert!(ok);
    assert_eq!(status["enabled"], false);
    assert_eq!(
        status["events"],
        serde_json::json!(["cli_started", "daemon_started"])
    );
    let (ok, bad) = run(&["on", "--origin", "https://TEST-DATA-CANARY@example.test"]);
    assert!(!ok);
    assert!(!bad.to_string().contains("TEST-DATA-CANARY"));
    assert!(run(&["on", "--origin", "https://collector.example.test"]).0);
    assert_eq!(run(&["status"]).1["enabled"], true);
    assert_eq!(run(&["off"]).1["enabled"], false);
    std::fs::write(state.0.join("telemetry.json"), r#"{"enabled":"true"}"#).unwrap();
    assert_eq!(run(&["status"]).1["enabled"], false);
}
