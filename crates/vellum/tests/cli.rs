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

// ---- the tool command, generated from the registry (guide Prompt 29) ----------------------------------------

mod common;

use common::workspace::Workspace;
use serde_json::Value;

/// `vellum tool ...` in the workspace's repo, against its daemon.
fn tool(space: &Workspace, args: &[&str]) -> Output {
    space
        .vellum()
        .arg("tool")
        .args(args)
        .current_dir(space.repo())
        .output()
        .expect("run vellum tool")
}

fn json_of(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {} / {}", text(&out.stdout), text(&out.stderr)))
}

#[test]
fn tool_help_lists_every_tool_in_the_registry_without_a_daemon() {
    let out = vellum(&["tool", "--help"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let help = text(&out.stdout);
    for name in vellum::tools::Registry::standard()
        .tools()
        .map(|t| t.name())
    {
        assert!(help.contains(name), "{name} in {help}");
    }
}

#[test]
fn a_tools_flags_come_from_its_schema() {
    let out = vellum(&["tool", "query_nodes", "--help"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let help = text(&out.stdout);
    assert!(
        help.contains("--type <type>") || help.contains("--type <TYPE>"),
        "{help}"
    );
    assert!(
        help.contains("frame") && help.contains("instance"),
        "the node types are listed: {help}"
    );
    assert!(
        help.contains("--limit") && help.contains("--variable"),
        "{help}"
    );

    let help = text(&vellum(&["tool", "snapshot_layout", "--help"]).stdout);
    assert!(
        help.contains("--problems-only"),
        "snake_case fields are kebab-case flags: {help}"
    );
}

#[test]
fn a_missing_required_flag_or_a_wrong_value_is_a_usage_error() {
    let out = vellum(&["tool", "get_nodes"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("--ids"), "{}", text(&out.stderr));
    let out = vellum(&["tool", "query_nodes", "--type", "button"]);
    assert_eq!(out.status.code(), Some(2));
    let out = vellum(&["tool", "query_nodes", "--limit", "many"]);
    assert_eq!(out.status.code(), Some(2));
    let out = vellum(&["tool", "no_such_tool"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn a_tool_runs_against_the_daemon_and_prints_its_result_as_json() {
    let space = Workspace::new();
    let opened = tool(&space, &["open_document", "--path", "design/app.vellum"]);
    assert!(opened.status.success(), "{}", text(&opened.stderr));
    assert_eq!(json_of(&opened)["pages"][0]["name"], "Home");

    // The document stays open in the daemon, so the next call needs no --document.
    let nodes = tool(
        &space,
        &[
            "get_nodes",
            "--ids",
            "frameHero1",
            "--ids",
            "textTitle1",
            "--depth",
            "0",
        ],
    );
    assert!(nodes.status.success(), "{}", text(&nodes.stderr));
    let nodes = json_of(&nodes);
    assert_eq!(nodes["nodes"][0]["name"], "Hero");
    assert_eq!(nodes["nodes"][1]["name"], "Title");

    let found = tool(
        &space,
        &[
            "query_nodes",
            "--type",
            "text",
            "--name",
            "title",
            "--document",
            "design/app.vellum",
        ],
    );
    assert_eq!(json_of(&found)["nodes"][0]["id"], "textTitle1");

    let layout = tool(
        &space,
        &["snapshot_layout", "--problems-only", "--node", "frameHero1"],
    );
    assert!(layout.status.success(), "{}", text(&layout.stderr));
    assert!(json_of(&layout)["problems"].is_array());

    let theme = tool(&space, &["get_variables", "--theme", r#"{"Mode":"dark"}"#]);
    assert_eq!(
        json_of(&theme)["theme"],
        serde_json::json!({"Mode": "dark"})
    );
}

#[test]
fn a_failing_tool_exits_non_zero_and_says_why_on_stderr() {
    let space = Workspace::new();
    let out = tool(
        &space,
        &[
            "get_nodes",
            "--ids",
            "frameHero1",
            "--document",
            "design/missing.vellum",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "{}", text(&out.stdout));
    assert!(
        text(&out.stderr).contains("missing.vellum"),
        "{}",
        text(&out.stderr)
    );

    let out = tool(&space, &["get_nodes", "--ids", "frameHero1"]);
    assert_eq!(out.status.code(), Some(1), "no document is open yet");
    assert!(
        text(&out.stderr).contains("open_document"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn json_prints_the_whole_mcp_result_even_for_a_failure() {
    let space = Workspace::new();
    let out = tool(
        &space,
        &[
            "--json",
            "get_nodes",
            "--ids",
            "x",
            "--document",
            "design/missing.vellum",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    let result = json_of(&out);
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["error"]["kind"], "not_found");
}

#[test]
fn a_screenshot_is_saved_to_the_file_named_by_image_out() {
    let space = Workspace::new();
    let file = space.root.join("card.png");
    let out = tool(
        &space,
        &[
            "screenshot",
            "--document",
            "design/app.vellum",
            "--node",
            "rectCard01",
            "--image-out",
            file.to_str().unwrap(),
        ],
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    let png = std::fs::read(&file).expect("the picture was written");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(json_of(&out)["image"]["mime"], "image/png");
    assert!(
        !text(&out.stdout).contains("iVBOR"),
        "the picture is not printed"
    );
}

// ---- Prompt 30: writes through the shipped binary -----------------------------------------------------------

#[test]
fn writes_and_scripts_reach_the_file_through_the_real_daemon() {
    let space = Workspace::new();
    let file = space.repo().join("design/app.vellum");
    let opened = tool(&space, &["open_document", "--path", "design/app.vellum"]);
    assert!(opened.status.success(), "{}", text(&opened.stderr));

    let inserted = tool(
        &space,
        &[
            "insert_nodes",
            "--parent",
            "frameFoot1",
            "--spec",
            r#"<rect name="Rule" width={390} height={1}/>"#,
        ],
    );
    assert!(inserted.status.success(), "{}", text(&inserted.stderr));
    let id = json_of(&inserted)["created"][0]
        .as_str()
        .unwrap()
        .to_owned();
    // Headless, every applied transaction is saved: the file is the truth.
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains(&id) && saved.contains("\"Rule\""), "{saved}");

    // The production build runs scripts in the QuickJS sandbox.
    let ran = tool(
        &space,
        &[
            "execute",
            "--code",
            "const f = doc.insert(doc.page('Other'), {type: 'frame', name: 'Made by a script', width: 10, height: 10, position: {type: 'absolute', x: 0, y: 0}}); return f;",
        ],
    );
    assert!(ran.status.success(), "{}", text(&ran.stderr));
    let ran = json_of(&ran);
    assert_eq!(ran["committed"], true);
    assert_eq!(ran["changes"]["created"], 1);
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("Made by a script")
    );

    // A bad batch exits non-zero, names the item and field, and changes nothing.
    let before = std::fs::read_to_string(&file).unwrap();
    let refused = tool(
        &space,
        &[
            "update_nodes",
            "--updates",
            r#"[{"id":"textFoot01","set":{"name":"x"}},{"id":"nopeNope01","set":{"name":"y"}}]"#,
        ],
    );
    assert_eq!(refused.status.code(), Some(1));
    assert!(
        text(&refused.stderr).contains("updates[1].id"),
        "{}",
        text(&refused.stderr)
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn skills_and_styles_reach_the_shipped_cli_without_opening_a_document() {
    let space = Workspace::new();
    for (args, key, count) in [
        (vec!["list_skills"], "skills", 8),
        (vec!["read_skill"], "skills", 8),
        (vec!["get_style"], "styles", 6),
    ] {
        let result = tool(&space, &args);
        assert!(result.status.success(), "{}", text(&result.stderr));
        assert_eq!(json_of(&result)[key].as_array().unwrap().len(), count);
    }
    let result = tool(&space, &["read_skill", "--path", "execute.md"]);
    assert!(result.status.success(), "{}", text(&result.stderr));
    assert!(
        json_of(&result)["content"]
            .as_str()
            .unwrap()
            .contains("doc.apiVersion")
    );
    let result = tool(&space, &["get_style", "--name", "quiet"]);
    assert!(result.status.success(), "{}", text(&result.stderr));
    assert_eq!(json_of(&result)["style"]["name"], "quiet");
    let result = tool(&space, &["read_skill", "--path", "missing.md"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(text(&result.stderr).contains("path"));
}
