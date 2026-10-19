//! TEST DATA. Prompt 59 exercises the shipped binary with isolated native daemons and a local model stub.
mod common;
use common::workspace::Workspace;
use serde_json::{Value, json};
use std::{io::Write, process::{Output, Stdio}};

fn run(space: &Workspace, args: &[&str]) -> Output {
    space.vellum().current_dir(space.repo()).env("NO_COLOR", "1").args(args).output().unwrap()
}
fn value(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|_| panic!("stdout: {} stderr: {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}
#[test]
fn version_text_and_json_goldens_and_usage_errors() {
    let space = Workspace::new();
    let out = run(&space, &["version"]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), format!("✓ vellum {}\n", env!("CARGO_PKG_VERSION")));
    let out = run(&space, &["version", "--json"]);
    assert_eq!(value(&out), json!({"ok":true,"command":"version","version":env!("CARGO_PKG_VERSION")}));
    let out = run(&space, &["export", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(value(&out)["ok"], false);
    assert!(out.stderr.is_empty());
}
#[test]
fn new_is_exclusive_and_json_failures_are_machine_readable() {
    let space = Workspace::new();
    let out = run(&space, &["new", "design/new.vellum", "--json"]);
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(value(&out)["created"], true);
    let before = std::fs::read(space.repo().join("design/new.vellum")).unwrap();
    let out = run(&space, &["new", "design/new.vellum", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(value(&out)["ok"], false);
    assert_eq!(before, std::fs::read(space.repo().join("design/new.vellum")).unwrap());
    let out = run(&space, &["new", "design/system.vellum", "--system", "missing", "--json"]);
    assert!(!out.status.success());
    assert!(!space.repo().join("design/system.vellum").exists());
}
#[test]
fn export_resolves_exact_names_all_formats_and_reports_destination_failure() {
    let space = Workspace::new();
    for (format, ext) in [("png","png"),("jpeg","jpg"),("webp","webp"),("svg","svg"),("pdf","pdf"),("html-css","html"),("html-tailwind","html")] {
        let dest = format!("out.{ext}");
        let out = run(&space, &["export", "design/app.vellum", "--frame", "Card", "--format", format, "--scale", "2", "-o", &dest, "--json"]);
        assert!(out.status.success(), "{format}: {:?}", out);
        assert_eq!(value(&out)["ok"], true);
        assert!(!std::fs::read(space.repo().join(dest)).unwrap().is_empty());
    }
    let out = run(&space, &["export", "design/app.vellum", "--frame", "missing", "-o", "bad.png", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(!space.repo().join("bad.png").exists());
    let out = run(&space, &["export", "design/app.vellum", "--frame", "Card", "-o", "no-dir/out.png", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(value(&out)["ok"], false);
}
#[test]
fn tokens_write_to_stdout_and_import_through_the_daemon() {
    let space = Workspace::new();
    std::fs::write(space.repo().join("theme.css"), ":root { --radius-card: 12px; }").unwrap();
    let out = run(&space, &["tokens", "import", "design/app.vellum", "theme.css", "--json"]);
    assert!(out.status.success(), "{:?}", out);
    let out = run(&space, &["tokens", "export", "design/app.vellum", "--format", "css-vars"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--radius-card: 12px"), "{text}");
    assert!(!text.contains('✓'));
}
#[test]
fn shell_headless_output_save_and_failed_line_exit() {
    let space = Workspace::new();
    let before = std::fs::read(space.repo().join("design/app.vellum")).unwrap();
    let mut child = space.vellum().current_dir(space.repo()).args(["shell", "--headless", "-i", "design/app.vellum", "-o", "design/copy.vellum", "--json"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"execute({code: \"doc.update('rectCard01', {name: 'Updated'});\"})\nsave()\nno_such_tool({})\nget_nodes({ids: ['rectCard01']})\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    let rows: Vec<Value> = String::from_utf8(out.stdout).unwrap().lines().map(|s| serde_json::from_str(s).unwrap()).collect();
    assert!(rows.iter().any(|r| r["command"] == "save" && r["ok"] == true), "{rows:?}");
    assert!(rows.iter().any(|r| r["command"] == "no_such_tool" && r["ok"] == false));
    assert!(String::from_utf8_lossy(&std::fs::read(space.repo().join("design/copy.vellum")).unwrap()).contains("Updated"));
    assert_eq!(before, std::fs::read(space.repo().join("design/app.vellum")).unwrap());
}
#[test]
fn prompt_help_documents_every_option_and_models_catalog_is_explicit() {
    let space = Workspace::new();
    let out = run(&space, &["prompt", "--help"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for flag in ["--engine", "--model", "--effort", "--attach", "--repo", "--parallel", "--mode", "--usage", "--preview-each-change", "--max-failed-calls", "--export", "--export-scale", "--verbose", "--tasks"] { assert!(text.contains(flag), "{flag}: {text}"); }
    let out = run(&space, &["models", "--engine", "builtin", "--json"]);
    assert!(out.status.success());
    assert_eq!(value(&out)["complete"], false, "a bundled catalog cannot claim live completeness");
}
