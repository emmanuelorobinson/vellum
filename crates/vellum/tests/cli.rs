use std::process::{Command, Output};

fn vellum(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vellum"))
        .args(args)
        .output()
        .expect("run vellum")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn version_exits_zero() {
    let out = vellum(&["--version"]);
    assert!(out.status.success(), "stderr: {}", text(&out.stderr));
    assert!(
        text(&out.stdout).starts_with("vellum "),
        "stdout: {}",
        text(&out.stdout)
    );
}

#[test]
fn mcp_help_exits_zero() {
    let out = vellum(&["mcp", "--help"]);
    assert!(out.status.success(), "stderr: {}", text(&out.stderr));
    assert!(text(&out.stdout).contains("mcp"));
}

#[test]
fn daemon_help_describes_opt_in_tcp() {
    let out = vellum(&["daemon", "--help"]);
    assert!(out.status.success());
    assert!(text(&out.stdout).contains("--listen"));
    assert!(text(&out.stdout).contains("--allow-origin"));
}
