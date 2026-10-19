//! TEST DATA. Prompt 59 exercises the shipped binary with isolated native daemons and a local model stub.
mod common;
use common::workspace::Workspace;
use serde_json::{Value, json};
use std::{
    io::Write,
    process::{Output, Stdio},
};

fn run(space: &Workspace, args: &[&str]) -> Output {
    space
        .vellum()
        .current_dir(space.repo())
        .env("NO_COLOR", "1")
        .args(args)
        .output()
        .unwrap()
}
fn value(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
        panic!(
            "stdout: {} stderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}
fn paint_space() -> Workspace {
    let doc: vellum_types::Document=serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA: CLI render","createdWith":"Vellum 0.0.0"},"pages":[{"id":"pageHome01","name":"Home","children":[{"type":"rect","id":"rectCard01","name":"Card","width":64,"height":40,"fills":[{"type":"solid","color":"#FF0000"}]}]}]})).unwrap();
    Workspace::with(&common::encode(&doc))
}

#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn version_text_and_json_goldens_and_usage_errors() {
    let space = Workspace::new();
    let out = run(&space, &["version"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("✓ vellum {}\n", env!("CARGO_PKG_VERSION"))
    );
    let out = run(&space, &["version", "--json"]);
    assert_eq!(
        value(&out),
        json!({"ok":true,"command":"version","version":env!("CARGO_PKG_VERSION")})
    );
    let out = run(&space, &["export", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(value(&out)["ok"], false);
    assert!(out.stderr.is_empty());
}
#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn new_is_exclusive_and_json_failures_are_machine_readable() {
    let space = Workspace::new();
    let out = run(&space, &["new", "design/new.vellum", "--json"]);
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(value(&out)["created"], true);
    let before = std::fs::read(space.repo().join("design/new.vellum")).unwrap();
    let out = run(&space, &["new", "design/new.vellum", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(value(&out)["ok"], false);
    assert_eq!(
        before,
        std::fs::read(space.repo().join("design/new.vellum")).unwrap()
    );
    let out = run(
        &space,
        &[
            "new",
            "design/system.vellum",
            "--system",
            "missing",
            "--json",
        ],
    );
    assert!(!out.status.success());
    assert!(!space.repo().join("design/system.vellum").exists());
}
#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn export_resolves_exact_names_all_formats_and_reports_destination_failure() {
    let mut doc = common::design();
    doc.fonts.clear();
    let space = Workspace::with(&common::encode(&doc));
    for (format, ext) in [
        ("png", "png"),
        ("jpeg", "jpg"),
        ("webp", "webp"),
        ("svg", "svg"),
        ("pdf", "pdf"),
        ("html-css", "html"),
        ("html-tailwind", "html"),
    ] {
        let dest = format!("out.{ext}");
        let out = run(
            &space,
            &[
                "export",
                "design/app.vellum",
                "--frame",
                "Card",
                "--format",
                format,
                "--scale",
                "2",
                "-o",
                &dest,
                "--json",
            ],
        );
        assert!(out.status.success(), "{format}: {:?}", out);
        assert_eq!(value(&out)["ok"], true);
        assert!(!std::fs::read(space.repo().join(dest)).unwrap().is_empty());
    }
    let out = run(
        &space,
        &[
            "export",
            "design/app.vellum",
            "--frame",
            "missing",
            "-o",
            "bad.png",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(!space.repo().join("bad.png").exists());
    let out = run(
        &space,
        &[
            "export",
            "design/app.vellum",
            "--frame",
            "Card",
            "-o",
            "no-dir/out.png",
            "--json",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(value(&out)["ok"], false);
}
#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn tokens_write_to_stdout_and_import_through_the_daemon() {
    let space = Workspace::new();
    std::fs::write(
        space.repo().join("theme.css"),
        ":root { --radius-card: 12px; }",
    )
    .unwrap();
    let out = run(
        &space,
        &[
            "tokens",
            "import",
            "design/app.vellum",
            "theme.css",
            "--json",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let out = run(
        &space,
        &[
            "tokens",
            "export",
            "design/app.vellum",
            "--format",
            "css-vars",
        ],
    );
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("--radius-card: 12px"), "{text}");
    assert!(!text.contains('✓'));
}
#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn shell_headless_output_save_and_failed_line_exit() {
    let space = Workspace::new();
    let before = std::fs::read(space.repo().join("design/app.vellum")).unwrap();
    let mut child = space
        .vellum()
        .current_dir(space.repo())
        .args([
            "shell",
            "--headless",
            "-i",
            "design/app.vellum",
            "-o",
            "design/copy.vellum",
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"execute({code: \"doc.update('rectCard01', {name: 'Updated'});\"})\nsave()\nno_such_tool({})\nget_nodes({ids: ['rectCard01']})\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    let rows: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(
        rows.iter()
            .any(|r| r["command"] == "save" && r["ok"] == true),
        "{rows:?}"
    );
    assert!(
        rows.iter()
            .any(|r| r["command"] == "no_such_tool" && r["ok"] == false)
    );
    assert!(
        String::from_utf8_lossy(&std::fs::read(space.repo().join("design/copy.vellum")).unwrap())
            .contains("Updated")
    );
    assert_eq!(
        before,
        std::fs::read(space.repo().join("design/app.vellum")).unwrap()
    );
}
#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn prompt_help_documents_every_option_and_models_catalog_is_explicit() {
    let space = Workspace::new();
    let out = run(&space, &["prompt", "--help"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for flag in [
        "--engine",
        "--model",
        "--effort",
        "--attach",
        "--repo",
        "--parallel",
        "--mode",
        "--usage",
        "--preview-each-change",
        "--max-failed-calls",
        "--export",
        "--export-scale",
        "--verbose",
        "--tasks",
    ] {
        assert!(text.contains(flag), "{flag}: {text}");
    }
    let out = run(&space, &["models", "--engine", "builtin", "--json"]);
    assert!(out.status.success());
    assert_eq!(
        value(&out)["complete"],
        false,
        "a bundled catalog cannot claim live completeness"
    );
}

#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn builtin_prompt_tasks_continue_after_failure_and_usage_never_contains_a_key() {
    use vellum_testkit::model::{StubEndpoint, StubReply};
    let space = Workspace::new();
    let stub = StubEndpoint::start(vec![
        StubReply::sse(vec![json!({"choices":[{"delta":{"content":"First complete"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":2}})], true),
        StubReply::http(401, "{}"),
        StubReply::sse(vec![json!({"choices":[{"delta":{"content":"Last complete"},"finish_reason":"stop"}],"usage":{"prompt_tokens":20,"completion_tokens":3}})], true),
    ]).unwrap();
    std::fs::write(
        space.repo().join("tasks.json"),
        serde_json::to_vec(&json!([
            {"document":"design/app.vellum","prompt":"first"},
            {"document":"design/app.vellum","prompt":"second"},
            {"document":"design/app.vellum","prompt":"last"}
        ]))
        .unwrap(),
    )
    .unwrap();
    let out = space
        .vellum()
        .current_dir(space.repo())
        .env("CI", "true")
        .env(
            "VELLUM_OPENAI_COMPAT_API_KEY",
            "cli-canary-key-do-not-persist",
        )
        .env("VELLUM_MODEL_ENDPOINT", stub.url("/chat"))
        .args([
            "prompt",
            "--tasks",
            "tasks.json",
            "--model",
            "openai-compat/fixture",
            "--usage",
            "usage.json",
            "--json",
            "--verbose",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    let rows: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let done: Vec<_> = rows.iter().filter(|r| r["event"] == "task_end").collect();
    assert_eq!(done.len(), 3, "{rows:?}");
    assert_eq!(done[0]["ok"], true);
    assert_eq!(done[1]["ok"], false);
    assert_eq!(done[2]["ok"], true);
    assert_eq!(stub.requests().len(), 3);
    let usage = std::fs::read_to_string(space.repo().join("usage.json")).unwrap();
    assert!(!usage.contains("cli-canary"));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("cli-canary"));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("cli-canary"));
    assert_eq!(
        serde_json::from_str::<Value>(&usage).unwrap()["tasks"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn prompt_writes_save_and_export_failures_are_nonzero() {
    use vellum_testkit::model::{StubEndpoint, StubReply};
    let space = paint_space();
    let stub = StubEndpoint::start(vec![
        StubReply::sse(vec![json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call1","function":{"name":"update_nodes","arguments":json!({"updates":[{"id":"rectCard01","set":{"name":"CLI updated"}}]}).to_string()}}]},"finish_reason":"tool_calls"}]})], true),
        StubReply::sse(vec![json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call2","function":{"name":"snapshot_layout","arguments":"{\"problems_only\":true}"}}]},"finish_reason":"tool_calls"}]})], true),
        StubReply::sse(vec![json!({"choices":[{"delta":{"content":"Complete"},"finish_reason":"stop"}]})], true),
    ]).unwrap();
    let out = space
        .vellum()
        .current_dir(space.repo())
        .env("CI", "true")
        .env("VELLUM_MODEL_ENDPOINT", stub.url("/chat"))
        .env("VELLUM_OPENAI_COMPAT_API_KEY", "cli-fixture-only")
        .args([
            "prompt",
            "design/app.vellum",
            "Rename the card",
            "--model",
            "openai-compat/fixture",
            "--preview-each-change",
            "--export",
            "missing/out.png",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    let saved = std::fs::read_to_string(space.repo().join("design/app.vellum")).unwrap();
    assert!(saved.contains("CLI updated"), "{:?}", out);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("\"event\":\"preview\""),
        "{out:?}"
    );
}

#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn parallel_side_by_side_allocates_distinct_reserved_areas_and_shared_turn() {
    use vellum_testkit::model::{StubEndpoint, StubReply};
    let space = Workspace::new();
    let say = || {
        StubReply::sse(
            vec![json!({"choices":[{"delta":{"content":"No changes"},"finish_reason":"stop"}]})],
            true,
        )
    };
    let stub = StubEndpoint::start(vec![say(), say()]).unwrap();
    let out = space
        .vellum()
        .current_dir(space.repo())
        .env("CI", "true")
        .env("VELLUM_MODEL_ENDPOINT", stub.url("/chat"))
        .env("VELLUM_OPENAI_COMPAT_API_KEY", "cli-fixture-only")
        .args([
            "prompt",
            "design/app.vellum",
            "Two options",
            "--model",
            "openai-compat/fixture",
            "--parallel",
            "2",
            "--mode",
            "side-by-side",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let requests = stub.requests();
    assert_eq!(requests.len(), 2);
    let a = requests[0].body.to_string();
    let b = requests[1].body.to_string();
    assert!(
        a.contains("option \\\"A\\\"") || a.contains("option \\\"B\\\""),
        "{a}"
    );
    assert_ne!(a, b);
    let rows: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let lanes: Vec<_> = rows.iter().filter(|r| r["event"] == "lane_start").collect();
    assert_eq!(lanes.len(), 2);
    assert_eq!(lanes[0]["turn"], lanes[1]["turn"]);
    assert_ne!(lanes[0]["agent"], lanes[1]["agent"]);
}

#[cfg(unix)]
#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn ctrl_c_interrupts_a_waiting_model_and_reports_stopped() {
    use std::time::{Duration, Instant};
    use vellum_testkit::model::{StubEndpoint, StubReply};
    let space = Workspace::new();
    let mut reply = StubReply::sse(
        vec![json!({"choices":[{"delta":{"content":"late"},"finish_reason":"stop"}]})],
        true,
    );
    reply.delay = Duration::from_millis(200);
    let stub = StubEndpoint::start(vec![reply]).unwrap();
    let mut child = space
        .vellum()
        .current_dir(space.repo())
        .env("CI", "true")
        .env("VELLUM_MODEL_ENDPOINT", stub.url("/chat"))
        .env("VELLUM_OPENAI_COMPAT_API_KEY", "cli-fixture-only")
        .args([
            "prompt",
            "design/app.vellum",
            "Wait",
            "--model",
            "openai-compat/fixture",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while stub.requests().is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let start = Instant::now();
    assert!(
        std::process::Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(4) {
            let _ = child.kill();
            panic!("CLI did not stop promptly");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{:?}", out);
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("Turn stopped"),
        "{:?}",
        out
    );
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn split_uses_the_proposed_items_and_explore_builds_successive_variants() {
    use vellum_testkit::model::{StubEndpoint, StubReply};
    for mode in ["split", "explore-layout", "explore-style"] {
        let space = paint_space();
        let say = || {
            StubReply::sse(
                vec![json!({"choices":[{"delta":{"content":"Complete"},"finish_reason":"stop"}]})],
                true,
            )
        };
        let mut replies = vec![];
        if mode == "split" {
            replies.push(StubReply::sse(vec![json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"plan1","function":{"name":"propose_plan","arguments":json!({"title":"Two parts","items":[{"name":"First","page":"Home","intent":"First screen","width":100,"height":80},{"name":"Second","page":"Home","intent":"Second screen","width":100,"height":80}]}).to_string()}}]},"finish_reason":"tool_calls"}]})],true));
        }
        replies.extend([say(), say()]);
        let stub = StubEndpoint::start(replies).unwrap();
        let out = space
            .vellum()
            .current_dir(space.repo())
            .env("CI", "true")
            .env("VELLUM_MODEL_ENDPOINT", stub.url("/chat"))
            .env("VELLUM_OPENAI_COMPAT_API_KEY", "cli-fixture-only")
            .args([
                "prompt",
                "design/app.vellum",
                "Two screens",
                "--model",
                "openai-compat/fixture",
                "--parallel",
                "2",
                "--mode",
                mode,
                "--json",
            ])
            .output()
            .unwrap();
        assert!(out.status.success(), "{mode}: {out:?}");
        let requests = stub.requests();
        assert_eq!(requests.len(), if mode == "split" { 3 } else { 2 });
        let last = requests.last().unwrap().body.to_string();
        assert!(
            last.contains(if mode == "split" {
                "Second screen"
            } else {
                "Variant 2 of 2"
            }),
            "{mode}: {last}"
        );
    }
}

#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn dynamic_tool_usage_errors_are_json_and_unsafe_shell_outputs_leave_nothing() {
    let space = Workspace::new();
    let out = run(&space, &["tool", "get_nodes", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(value(&out)["ok"], false);
    assert!(out.stderr.is_empty());
    let dest = space.root.join("outside.vellum");
    let out = run(
        &space,
        &[
            "shell",
            "--headless",
            "-i",
            "design/app.vellum",
            "-o",
            dest.to_str().unwrap(),
            "--json",
        ],
    );
    assert!(!out.status.success());
    assert!(
        !dest.exists(),
        "output scope must be checked before copying"
    );
}

#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn malformed_task_does_not_skip_later_tasks_and_provider_echoes_are_redacted() {
    use vellum_testkit::model::{StubEndpoint, StubReply};
    let space = Workspace::new();
    let key = "cli-echo-canary-unique";
    let stub=StubEndpoint::start(vec![StubReply::sse(vec![json!({"choices":[{"delta":{"content":key},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":3}})],true)]).unwrap();
    std::fs::write(space.repo().join("tasks.json"),serde_json::to_vec(&json!([{"document":"design/app.vellum"},{"document":"design/app.vellum","prompt":"Continue","effort":"high"}])).unwrap()).unwrap();
    let out = space
        .vellum()
        .current_dir(space.repo())
        .env("CI", "true")
        .env("VELLUM_MODEL_ENDPOINT", stub.url("/chat"))
        .env("VELLUM_OPENAI_COMPAT_API_KEY", key)
        .args([
            "prompt",
            "--tasks",
            "tasks.json",
            "--model",
            "openai-compat/fixture",
            "--json",
            "--usage",
            "usage.json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stub.requests().len(), 1, "{out:?}");
    assert_eq!(stub.requests()[0].body["reasoning_effort"], "high");
    assert!(!String::from_utf8_lossy(&out.stdout).contains(key));
    assert!(String::from_utf8_lossy(&out.stdout).contains("[redacted]"));
    assert!(
        !std::fs::read_to_string(space.repo().join("usage.json"))
            .unwrap()
            .contains(key)
    );
    for file in [
        space.root.join("state/sessions.db"),
        space.repo().join("design/app.vellum"),
    ] {
        if let Ok(bytes) = std::fs::read(file) {
            assert!(!bytes.windows(key.len()).any(|w| w == key.as_bytes()));
        }
    }
}

#[test]
#[ignore = "native-cli: private sockets and loopback model; run --test cli_complete -- --ignored"]
fn daemon_pause_all_interrupts_a_cli_waiting_on_the_provider() {
    use std::time::{Duration, Instant};
    use vellum_testkit::model::{StubEndpoint, StubReply};
    let space = paint_space();
    let mut reply = StubReply::sse(
        vec![json!({"choices":[{"delta":{"content":"late"},"finish_reason":"stop"}]})],
        true,
    );
    reply.delay = Duration::from_millis(200);
    let stub = StubEndpoint::start(vec![reply]).unwrap();
    let mut child = space
        .vellum()
        .current_dir(space.repo())
        .env("CI", "true")
        .env("VELLUM_MODEL_ENDPOINT", stub.url("/chat"))
        .env("VELLUM_OPENAI_COMPAT_API_KEY", "cli-fixture-only")
        .args([
            "prompt",
            "design/app.vellum",
            "Wait",
            "--model",
            "openai-compat/fixture",
            "--json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(5);
    while stub.requests().is_empty() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    let dir = vellum_ipc_local::RunDir::new(space.run_dir()).unwrap();
    let requests = [
        json!({"id":1,"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"TEST DATA:pause","version":"0"}}}),
        json!({"id":2,"jsonrpc":"2.0","method":"vellum/pauseAll","params":{}}),
    ];
    let start = Instant::now();
    vellum_ipc_local::exchange(
        &dir,
        std::path::Path::new(env!("CARGO_BIN_EXE_vellum")),
        &requests,
    )
    .unwrap();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(4) {
            let _ = child.kill();
            panic!("Pause all did not interrupt the CLI");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
#[ignore = "native-cli: private sockets; run --test cli_complete -- --ignored"]
fn shell_save_follows_an_explicit_document_switch() {
    let space = Workspace::new();
    let mut child = space
        .vellum()
        .current_dir(space.repo())
        .args(["shell", "design/app.vellum", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"open_document({path:'design/other.vellum',create:true})\nsave()\nexit()\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    let rows: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    let saved = rows.iter().find(|r| r["command"] == "save").unwrap();
    assert!(
        saved["document"]
            .as_str()
            .unwrap()
            .ends_with("design/other.vellum"),
        "{saved}"
    );
}
