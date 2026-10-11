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
fn mcp_is_honest_that_it_is_not_built_yet() {
    let out = vellum(&["mcp"]);
    assert!(
        !out.status.success(),
        "a placeholder must not pretend to be an MCP server"
    );
    assert!(
        text(&out.stderr).contains("not implemented"),
        "stderr: {}",
        text(&out.stderr)
    );
}
