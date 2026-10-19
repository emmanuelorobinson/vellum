//! TEST DATA. CLI lifecycle over the real MCP handler and a fake live editor; no network or installed engine.
mod common;
use serde_json::{Value, json};
use vellum::{daemon::EditorId, mcp::McpServer, sessions::SessionLog, tools::Registry};
use vellum_core::ports::EventBody;
use vellum_ipc_local::RpcHandler;

#[test]
fn cli_turn_reaches_live_target_logs_engine_events_and_disconnect_blocks_late_writes() {
    let doc = common::design();
    let mut daemon = common::Daemon::new(&doc, &["repo/design/app.vellum"]);
    let editor = common::FakeEditor::new(&doc);
    daemon
        .docs
        .open_editor(
            "/repo/design/app.vellum",
            EditorId("live".into()),
            editor.link(),
        )
        .unwrap();
    let root = std::env::temp_dir().join(format!("vellum-cli-lifecycle-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut server = McpServer::new(Registry::standard(), daemon.docs)
        .with_default_cwd(Some("/repo".into()))
        .with_session_log(Some(SessionLog::open(&root).unwrap()));
    let rpc = |s: &mut McpServer, conn, method: &str, params: Value| {
        s.request(
            conn,
            json!({"id":1,"jsonrpc":"2.0","method":method,"params":params}),
        )
        .unwrap()
    };
    let refused = rpc(
        &mut server,
        1,
        "vellum/cli",
        json!({"action":"turn","document":"design/app.vellum","engine":"builtin","model":"fixture","prompt":"Update"}),
    );
    assert!(refused.get("error").is_some());
    for conn in [1, 2] {
        rpc(
            &mut server,
            conn,
            "initialize",
            json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"vellum-cli","version":"0"}}),
        );
    }
    let began = rpc(
        &mut server,
        1,
        "vellum/cli",
        json!({"action":"turn","document":"design/app.vellum","engine":"builtin","model":"fixture","prompt":"Update"}),
    );
    assert!(began.get("error").is_none(), "{began}");
    let joined = rpc(
        &mut server,
        2,
        "vellum/cli",
        json!({"action":"turn","document":"design/app.vellum","engine":"builtin","model":"fixture","prompt":"Update","join":began["result"]["turn"]}),
    );
    assert_eq!(joined["result"]["turn"], began["result"]["turn"]);
    let changed = rpc(
        &mut server,
        2,
        "tools/call",
        json!({"name":"update_nodes","arguments":{"document":"design/app.vellum","updates":[{"id":"rectCard01","set":{"name":"Live CLI"}}]}}),
    );
    assert_eq!(changed["result"]["isError"], false, "{changed}");
    let origin = editor.0.borrow().origins.last().unwrap().clone();
    assert_eq!(
        origin.turn.as_ref().map(|t| t.as_str()),
        began["result"]["turn"].as_str()
    );
    assert_eq!(
        origin.session.as_ref().map(|s| s.as_str()),
        began["result"]["session"].as_str()
    );
    let saved = rpc(
        &mut server,
        1,
        "vellum/cli",
        json!({"action":"save","document":"design/app.vellum"}),
    );
    assert_eq!(saved["result"]["saved"], true);
    rpc(
        &mut server,
        1,
        "vellum/cli",
        json!({"action":"usage","usage":{"input_tokens":7,"output_tokens":3}}),
    );
    server.closed(1);
    let late = rpc(
        &mut server,
        2,
        "tools/call",
        json!({"name":"update_nodes","arguments":{"document":"design/app.vellum","updates":[{"id":"rectCard01","set":{"name":"Too late"}}]}}),
    );
    assert_eq!(
        late["result"]["isError"], true,
        "a dead owner must stop every lane: {late}"
    );
    let events = server
        .session_log()
        .unwrap()
        .store()
        .events(origin.session.as_ref().unwrap(), None)
        .unwrap();
    assert!(events.iter().any(|e| matches!(e.body, EventBody::Usage(_))));
    drop(server);
    std::fs::remove_dir_all(root).unwrap();
}
