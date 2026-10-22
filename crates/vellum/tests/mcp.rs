//! TEST DATA. The MCP adapter over the tool registry (guide Prompt 29): protocol, errors, images, sessions and the
//! tool schema snapshot (ARCHITECTURE §2.5).

mod common;

use common::{Daemon, FakeEditor, design};
use serde_json::{Value, json};
use vellum::daemon::EditorId;
use vellum::mcp::McpServer;
use vellum::tools::Registry;
use vellum_ipc_local::RpcHandler;

const PATH: &str = "design/app.vellum";

fn server_over(daemon: Daemon) -> McpServer {
    McpServer::new(Registry::standard(), daemon.docs)
}

fn server() -> McpServer {
    server_over(Daemon::new(&design(), &[PATH]))
}

fn scene_server() -> McpServer {
    server_over(Daemon::with_scene(&design(), &[PATH]))
}

fn rpc(server: &mut McpServer, conn: u64, id: u64, method: &str, params: Value) -> Value {
    server
        .request(
            conn,
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        )
        .unwrap_or_else(|| panic!("{method} got no reply"))
}

fn call(server: &mut McpServer, tool: &str, arguments: Value) -> Value {
    rpc(
        server,
        1,
        1,
        "tools/call",
        json!({"name": tool, "arguments": arguments}),
    )
}

fn text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("text content in {reply}"))
}

#[test]
fn initialize_agrees_a_protocol_version_and_offers_tools() {
    let mut server = server();
    let reply = rpc(
        &mut server,
        1,
        1,
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "Claude Code", "version": "2.1.0"},
        }),
    );
    assert_eq!(reply["jsonrpc"], "2.0");
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(reply["result"]["serverInfo"]["name"], "vellum");
    assert_eq!(
        reply["result"]["capabilities"]["tools"],
        json!({"listChanged": false})
    );
    assert!(reply["result"]["capabilities"].get("resources").is_none());
    assert!(
        reply["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("get_app_state")
    );
}

#[test]
fn initialize_with_a_version_it_does_not_know_answers_with_its_newest() {
    let mut server = server();
    let reply = rpc(
        &mut server,
        1,
        1,
        "initialize",
        json!({"protocolVersion": "1999-01-01", "capabilities": {}, "clientInfo": {"name": "x", "version": "1"}}),
    );
    assert_eq!(reply["result"]["protocolVersion"], "2025-11-25");
}

#[test]
fn initialize_with_nonsense_params_is_an_invalid_params_error() {
    let mut server = server();
    let reply = rpc(
        &mut server,
        1,
        1,
        "initialize",
        json!({"protocolVersion": 7}),
    );
    assert_eq!(reply["error"]["code"], -32602);
}

#[test]
fn notifications_get_no_reply_and_ping_answers_empty() {
    let mut server = server();
    assert!(
        server
            .request(
                1,
                json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
            )
            .is_none()
    );
    assert!(server.request(1, json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 3}})).is_none());
    assert!(
        server
            .request(1, json!({"jsonrpc": "2.0", "method": "tools/nothing"}))
            .is_none(),
        "an unknown notification is ignored"
    );
    assert_eq!(
        rpc(&mut server, 1, 5, "ping", json!({}))["result"],
        json!({})
    );
}

#[test]
fn an_unknown_method_is_method_not_found() {
    let mut server = server();
    for method in ["resources/list", "prompts/list", "completion/complete"] {
        let reply = rpc(&mut server, 1, 2, method, json!({}));
        assert_eq!(reply["error"]["code"], -32601, "{method}");
    }
}

#[test]
fn server_discover_lists_versions_and_capabilities_for_stateless_clients() {
    let mut server = server();
    let reply = rpc(&mut server, 1, 1, "server/discover", json!({}));
    let result = &reply["result"];
    assert_eq!(result["resultType"], "complete");
    assert!(
        result["supportedVersions"]
            .as_array()
            .unwrap()
            .contains(&json!("2025-11-25"))
    );
    assert_eq!(
        result["capabilities"]["tools"],
        json!({"listChanged": false})
    );
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "vellum"
    );
}

#[test]
fn tools_list_is_the_registry_with_schemas_and_hints() {
    let mut server = server();
    let reply = rpc(&mut server, 1, 1, "tools/list", json!({}));
    let tools = reply["result"]["tools"].as_array().unwrap();
    let registry = Registry::standard();
    assert_eq!(tools.len(), registry.tools().count());
    for (listed, tool) in tools.iter().zip(registry.tools()) {
        assert_eq!(listed["name"], tool.name());
        assert_eq!(listed["description"], tool.description());
        assert_eq!(listed["inputSchema"]["type"], "object");
        assert_eq!(listed["annotations"]["readOnlyHint"], tool.read_only());
        assert_eq!(listed["_meta"]["vellum/version"], tool.version());
    }
    assert!(reply["result"].get("nextCursor").is_none());
}

/// The tool surface is a versioned boundary (ARCHITECTURE §2.5): names are stable and a schema may only grow.
/// A change to any tool shows up here. Review it as an API change, then rewrite the file with
/// `VELLUM_UPDATE_SNAPSHOTS=1 cargo test -p vellum --test mcp`.
#[test]
fn mcp_tool_schema_snapshot() {
    let mut server = server();
    let reply = rpc(&mut server, 1, 1, "tools/list", json!({}));
    let actual = serde_json::to_string_pretty(&reply["result"]).unwrap() + "\n";
    let file =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/mcp-tools.json");
    if std::env::var_os("VELLUM_UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&file, &actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&file).unwrap_or_else(|_| {
        panic!(
            "{} is missing; run with VELLUM_UPDATE_SNAPSHOTS=1",
            file.display()
        )
    });
    assert!(
        actual == expected,
        "the MCP tool schemas changed; review the diff and update tests/snapshots/mcp-tools.json"
    );
}

#[test]
fn content_tools_are_available_over_mcp_before_a_document_is_opened() {
    let mut server = server();
    let reply = call(&mut server, "list_skills", json!({}));
    assert_eq!(
        reply["result"]["structuredContent"]["skills"]
            .as_array()
            .unwrap()
            .len(),
        7
    );
    let reply = call(&mut server, "read_skill", json!({"path":"format.md"}));
    assert!(
        reply["result"]["structuredContent"]["content"]
            .as_str()
            .unwrap()
            .contains("Node specs")
    );
    let reply = call(&mut server, "get_style", json!({"name":"quiet"}));
    assert!(reply["result"]["structuredContent"]["style"]["palette"].is_object());
    let reply = call(&mut server, "read_skill", json!({"path":"../private"}));
    assert_eq!(reply["result"]["isError"], true);
    assert_eq!(
        reply["result"]["structuredContent"]["error"]["path"],
        "path"
    );
}

#[test]
fn a_tool_call_returns_its_result_as_text_and_as_structured_content() {
    let mut server = server();
    call(&mut server, "open_document", json!({"path": PATH}));
    let reply = call(&mut server, "get_nodes", json!({"ids": ["textTitle1"]}));
    assert_eq!(reply["id"], 1);
    assert!(reply["result"].get("isError").is_none() || reply["result"]["isError"] == false);
    assert_eq!(
        reply["result"]["structuredContent"]["nodes"][0]["name"],
        "Title"
    );
    let parsed: Value = serde_json::from_str(text(&reply)).unwrap();
    assert_eq!(parsed, reply["result"]["structuredContent"]);
    assert!(
        !text(&reply).contains('\n'),
        "compact JSON keeps tokens down"
    );
}

#[test]
fn a_failed_tool_call_is_an_error_result_the_agent_can_read_and_fix() {
    let mut server = server();
    let reply = call(&mut server, "get_nodes", json!({"ids": ["a"], "depht": 1}));
    assert!(
        reply.get("error").is_none(),
        "a tool failure is not a protocol failure"
    );
    assert_eq!(reply["result"]["isError"], true);
    assert!(text(&reply).contains("depht"), "{}", text(&reply));
    assert_eq!(
        reply["result"]["structuredContent"]["error"]["kind"],
        "invalid_input"
    );
}

#[test]
fn an_unknown_tool_or_malformed_call_is_invalid_params() {
    let mut server = server();
    assert_eq!(
        rpc(
            &mut server,
            1,
            1,
            "tools/call",
            json!({"name": "insert_node", "arguments": {}})
        )["error"]["code"],
        -32602
    );
    assert_eq!(
        rpc(&mut server, 1, 1, "tools/call", json!({"arguments": {}}))["error"]["code"],
        -32602
    );
    assert_eq!(
        rpc(
            &mut server,
            1,
            1,
            "tools/call",
            json!({"name": "get_nodes", "arguments": [1]})
        )["error"]["code"],
        -32602
    );
    // No arguments at all is an empty object.
    let reply = rpc(
        &mut server,
        1,
        1,
        "tools/call",
        json!({"name": "get_app_state"}),
    );
    assert!(reply.get("error").is_none());
}

#[test]
fn a_screenshot_comes_back_as_an_image_block_not_as_text() {
    let mut server = scene_server();
    call(&mut server, "open_document", json!({"path": PATH}));
    let reply = call(&mut server, "screenshot", json!({"node": "rectCard01"}));
    let content = reply["result"]["content"].as_array().unwrap();
    let image = content
        .iter()
        .find(|c| c["type"] == "image")
        .expect("an image block");
    assert_eq!(image["mimeType"], "image/png");
    assert!(image["data"].as_str().unwrap().starts_with("iVBORw0KGgo"));
    let summary = content
        .iter()
        .find(|c| c["type"] == "text")
        .expect("a text block");
    assert!(
        !summary["text"].as_str().unwrap().contains("iVBOR"),
        "the picture is not repeated as text"
    );
    let structured = &reply["result"]["structuredContent"];
    assert_eq!(structured["image"]["mime"], "image/png");
    assert!(structured["image"].get("base64").is_none());
    assert_eq!(structured["image"]["width"], 160.0);
}

#[test]
fn each_connection_keeps_its_own_client_and_working_directory() {
    let daemon = Daemon::new(&design(), &[]);
    daemon
        .fs
        .seed("work/a/design/app.vellum", &common::encode(&design()));
    daemon
        .fs
        .seed("work/b/design/app.vellum", &common::encode(&design()));
    let mut server = server_over(daemon);
    server.opened(1, Some("/work/a".into()));
    server.opened(2, Some("/work/b".into()));

    let one = rpc(
        &mut server,
        1,
        1,
        "tools/call",
        json!({"name": "open_document", "arguments": {"path": "design/app.vellum"}}),
    );
    let two = rpc(
        &mut server,
        2,
        1,
        "tools/call",
        json!({"name": "open_document", "arguments": {"path": "design/app.vellum"}}),
    );
    assert_eq!(
        one["result"]["structuredContent"]["path"], "/work/a/design/app.vellum",
        "{one}"
    );
    assert_eq!(
        two["result"]["structuredContent"]["path"],
        "/work/b/design/app.vellum"
    );

    // Each client sees only the documents inside its own directory, though the daemon holds both.
    let a_state = rpc(
        &mut server,
        1,
        2,
        "tools/call",
        json!({"name": "get_app_state", "arguments": {}}),
    );
    assert!(a_state["result"]["isError"] != true, "{a_state}");
    let listed: Vec<_> = a_state["result"]["structuredContent"]["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["path"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(listed, ["/work/a/design/app.vellum"]);
    assert_eq!(
        a_state["result"]["structuredContent"]["active"], "/work/a/design/app.vellum",
        "B opened last, but A cannot see that"
    );
    let a_nodes = rpc(
        &mut server,
        1,
        3,
        "tools/call",
        json!({"name": "get_nodes", "arguments": {"ids": ["frameHero1"], "depth": 0}}),
    );
    assert_eq!(
        a_nodes["result"]["structuredContent"]["document"],
        "/work/a/design/app.vellum"
    );
    for tool in ["get_app_state", "get_nodes"] {
        let reply = rpc(
            &mut server,
            1,
            4,
            "tools/call",
            json!({"name": tool, "arguments": {"document": "/work/b/design/app.vellum", "ids": ["x"]}}),
        );
        assert_eq!(reply["result"]["isError"], true, "{tool}");
    }

    // A client has to stay inside the directory it was started in.
    for path in [
        "../b/design/app.vellum",
        "/work/b/design/app.vellum",
        "/etc/hosts.vellum",
    ] {
        let reply = rpc(
            &mut server,
            1,
            2,
            "tools/call",
            json!({"name": "open_document", "arguments": {"path": path}}),
        );
        assert_eq!(reply["result"]["isError"], true, "{path}");
        assert!(text(&reply).contains("outside /work/a"), "{}", text(&reply));
    }
    // The same file reached from inside is the same document, however it is spelled.
    let again = rpc(
        &mut server,
        1,
        3,
        "tools/call",
        json!({"name": "open_document", "arguments": {"path": "./design/../design/app.vellum"}}),
    );
    assert_eq!(
        again["result"]["structuredContent"]["path"], "/work/a/design/app.vellum",
        "{again}"
    );

    server.closed(1);
    assert_eq!(
        server.documents().paths().len(),
        2,
        "closing a connection does not close documents"
    );
    // A new connection from the same directory finds the document still open.
    server.opened(3, Some("/work/a".into()));
    let state = rpc(
        &mut server,
        3,
        4,
        "tools/call",
        json!({"name": "get_app_state", "arguments": {}}),
    );
    assert_eq!(
        state["result"]["structuredContent"]["active"],
        "/work/a/design/app.vellum"
    );
}

#[test]
fn the_client_name_from_initialize_is_remembered_for_the_connection() {
    let mut server = server();
    rpc(
        &mut server,
        7,
        1,
        "initialize",
        json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "Codex", "version": "0.9"}}),
    );
    let client = server.client(7).expect("a client");
    assert_eq!(
        (client.name.as_str(), client.version.as_str()),
        ("Codex", "0.9")
    );
    assert!(server.client(8).is_none());
    server.closed(7);
    assert!(server.client(7).is_none());
}

#[test]
fn an_mcp_client_reaches_a_document_an_editor_holds() {
    let mut daemon = Daemon::new(&design(), &[PATH]);
    let editor = FakeEditor::new(&design());
    daemon
        .docs
        .open_editor(PATH, EditorId("tab-1".into()), editor.link())
        .unwrap();
    let mut server = server_over(daemon);
    let state = call(&mut server, "get_app_state", json!({"document": PATH}));
    assert_eq!(
        state["result"]["structuredContent"]["documents"][0]["mode"],
        "live"
    );
    let nodes = call(
        &mut server,
        "get_nodes",
        json!({"document": PATH, "ids": ["frameHero1"], "depth": 0}),
    );
    assert_eq!(
        nodes["result"]["structuredContent"]["nodes"][0]["childCount"],
        5
    );
}

#[test]
fn a_connection_that_never_said_where_it_started_is_held_to_the_daemons_directory() {
    let daemon = Daemon::new(&design(), &[]);
    daemon
        .fs
        .seed("work/a/design/app.vellum", &common::encode(&design()));
    let mut server = server_over(daemon).with_default_cwd(Some("/work/a".into()));
    // An HTTP client never has an `opened` call.
    let ok = rpc(
        &mut server,
        9,
        1,
        "tools/call",
        json!({"name": "open_document", "arguments": {"path": "design/app.vellum"}}),
    );
    assert_eq!(
        ok["result"]["structuredContent"]["path"], "/work/a/design/app.vellum",
        "{ok}"
    );
    let outside = rpc(
        &mut server,
        9,
        2,
        "tools/call",
        json!({"name": "open_document", "arguments": {"path": "/work/b/x.vellum"}}),
    );
    assert_eq!(outside["result"]["isError"], true);
    // A client that does say overrides the default.
    server.opened(10, Some("/work/b".into()));
    let other = rpc(
        &mut server,
        10,
        1,
        "tools/call",
        json!({"name": "open_document", "arguments": {"path": "/work/a/design/app.vellum"}}),
    );
    assert_eq!(other["result"]["isError"], true);
}

// ---- Prompt 30: writes over MCP ---------------------------------------------------------------------------

#[test]
fn write_tools_are_listed_with_their_hints() {
    let mut server = server();
    let reply = rpc(&mut server, 1, 1, "tools/list", json!({}));
    let tools = reply["result"]["tools"].as_array().unwrap();
    let hints = |name: &str| {
        let tool = tools
            .iter()
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("{name} is listed"));
        tool["annotations"].clone()
    };
    assert_eq!(
        hints("insert_nodes"),
        json!({"readOnlyHint": false, "destructiveHint": false, "openWorldHint": false})
    );
    assert_eq!(hints("delete_nodes")["destructiveHint"], true);
    assert_eq!(hints("execute")["readOnlyHint"], false);
}

#[test]
fn a_rejected_batch_says_which_item_and_field_to_fix() {
    let mut server = server();
    call(&mut server, "open_document", json!({"path": PATH}));
    let reply = call(
        &mut server,
        "update_nodes",
        json!({"updates": [{"id": "textTitle1", "set": {"name": "A"}}, {"id": "nopeNope01", "set": {"name": "B"}}]}),
    );
    assert_eq!(reply["result"]["isError"], true);
    let error = &reply["result"]["structuredContent"]["error"];
    assert_eq!(error["op"], 1);
    assert_eq!(error["path"], "updates[1].id");
    assert_eq!(error["kind"], "invalid_input");
    assert!(text(&reply).contains("nopeNope01"), "{}", text(&reply));
}

#[test]
fn writes_over_mcp_are_attributed_to_the_connection_and_leave_steps() {
    let mut daemon = Daemon::new(&design(), &[PATH]);
    let editor = FakeEditor::new(&design());
    daemon
        .docs
        .open_editor(PATH, EditorId("tab-1".into()), editor.link())
        .unwrap();
    let mut server = server_over(daemon);
    rpc(
        &mut server,
        1,
        1,
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "Claude Code", "version": "2"}}),
    );
    let reply = call(
        &mut server,
        "insert_nodes",
        json!({"document": PATH, "parent": "frameFoot1", "nodes": [{"type": "rect", "width": 4, "height": 4}]}),
    );
    assert_eq!(reply["result"]["isError"], false, "{reply}");
    let agent = server.agent(1).expect("the connection is an agent").clone();
    assert_eq!(
        editor.0.borrow().actors,
        vec![vellum_types::Actor::Agent { agent }]
    );
    let steps = server.steps(1);
    assert_eq!(steps.last().unwrap().tool, "insert_nodes");
    let state = call(&mut server, "get_app_state", json!({}));
    assert_eq!(
        state["result"]["structuredContent"]["you"]["name"],
        "Claude Code"
    );
}

#[test]
fn host_pause_all_requires_initialize_and_pauses_every_open_document() {
    let mut server = server();
    let denied = rpc(&mut server, 8, 1, "vellum/pauseAll", json!({}));
    assert!(denied.get("error").is_some());
    rpc(
        &mut server,
        8,
        2,
        "initialize",
        json!({
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "vellum-vscode", "version": "0.0.0"}
        }),
    );
    let invalid = rpc(
        &mut server,
        8,
        3,
        "vellum/pauseAll",
        json!({"unexpected": true}),
    );
    assert!(invalid.get("error").is_some());
    let paused = rpc(&mut server, 8, 4, "vellum/pauseAll", json!({}));
    assert_eq!(paused["result"]["paused"], true);
    // A subsequent agent write still goes through the Guard and is rejected.
    call(&mut server, "open_document", json!({"path": PATH}));
    let write = call(
        &mut server,
        "update_nodes",
        json!({"updates": [{"id": "frameHero1", "set": {"name": "paused"}}]}),
    );
    assert_eq!(write["result"]["isError"], true);
    assert!(text(&write).contains("paused"));
}

#[test]
fn host_state_is_initialized_scoped_and_not_an_agent_tool() {
    let mut server = server();
    let state = |server: &mut McpServer, params| rpc(server, 1, 12, "vellum/hostState", params);
    assert_eq!(
        state(&mut server, json!({"paths":[]}))["error"]["code"],
        -32002
    );
    rpc(
        &mut server,
        1,
        1,
        "initialize",
        json!({"protocolVersion":"2025-11-25", "clientInfo":{"name":"TEST DATA client"}}),
    );
    let result = state(&mut server, json!({"paths":[]}));
    assert_eq!(result["result"]["clients"], json!(["TEST DATA client"]));
    assert!(result["result"]["toolCount"].as_u64().unwrap() > 18);
    assert_eq!(result["result"]["sessions"], json!([]));
    assert_eq!(result["result"]["sessionsAvailable"], false);
    assert_eq!(
        state(&mut server, json!({"paths":[], "extra":true}))["error"]["code"],
        -32602
    );
    assert_eq!(
        state(&mut server, json!({"paths":["/outside/private.vellum"]}))["error"]["code"],
        -32602
    );
    let list = rpc(&mut server, 1, 13, "tools/list", json!({}));
    assert!(!list.to_string().contains("hostState"));
    server.closed(1);
    rpc(
        &mut server,
        2,
        1,
        "initialize",
        json!({"protocolVersion":"2025-11-25", "clientInfo":{"name":"TEST DATA second"}}),
    );
    assert_eq!(
        rpc(&mut server, 2, 14, "vellum/hostState", json!({"paths":[]}))["result"]["clients"],
        json!(["TEST DATA second"])
    );
}

#[test]
fn host_recovery_is_initialized_scoped_metadata_and_not_an_agent_tool() {
    let mut server = server();
    assert_eq!(
        rpc(
            &mut server,
            1,
            1,
            "vellum/recovery",
            json!({"path":"/project/a.vellum","action":"find"})
        )["error"]["code"],
        -32002
    );
    server.opened(1, Some("/project".to_owned()));
    rpc(
        &mut server,
        1,
        2,
        "initialize",
        json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"TEST DATA host"}}),
    );
    let reply = rpc(
        &mut server,
        1,
        3,
        "vellum/recovery",
        json!({"path":"/project/a.vellum","action":"find"}),
    );
    assert_eq!(reply["result"], json!({"savedAt":null}));
    for params in [
        json!({"path":"/outside/a.vellum","action":"find"}),
        json!({"path":"/project/../outside.vellum","action":"discard"}),
        json!({"path":"/project/a.vellum","action":"find","extra":true}),
    ] {
        assert_eq!(
            rpc(&mut server, 1, 4, "vellum/recovery", params)["error"]["code"],
            -32602
        );
    }
    assert!(
        !rpc(&mut server, 1, 5, "tools/list", json!({}))
            .to_string()
            .contains("vellum/recovery")
    );
}

#[test]
fn host_editors_replicate_the_daemons_engine_over_updates() {
    use vellum_contracts::fakes::{InMemoryDocStore, SequentialIds};
    use vellum_contracts::samples::{alice, rename};
    use vellum_core::base64;
    use vellum_core::ports::{DocStore, SyncPayload};
    let mut server = server();
    let editor =
        |server: &mut McpServer, conn, params: Value| rpc(server, conn, 9, "vellum/editor", params);
    let open = json!({"action":"open","path":"/design/app.vellum","editor":"tab-1",
        "device":"01JA0000000000000000000A11"});
    assert_eq!(
        editor(&mut server, 1, open.clone())["error"]["code"],
        -32002
    );
    server.opened(1, Some("/design".to_owned()));
    rpc(
        &mut server,
        1,
        1,
        "initialize",
        json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"TEST DATA host"}}),
    );
    for params in [
        json!({"action":"open","path":"/elsewhere/app.vellum","editor":"tab-1","device":"01JA0000000000000000000A11"}),
        json!({"action":"open","path":"/design/app.vellum","editor":"tab 1","device":"01JA0000000000000000000A11"}),
        json!({"action":"open","path":"/design/app.vellum","editor":"tab-1","device":"not a device"}),
        json!({"action":"wipe","path":"/design/app.vellum","editor":"tab-1"}),
    ] {
        assert_eq!(editor(&mut server, 1, params)["error"]["code"], -32602);
    }
    let opened = editor(&mut server, 1, open)["result"].clone();
    assert_eq!(opened["role"], "writer");
    assert_eq!(opened["recovery"], Value::Null);
    let genesis: vellum_types::Document =
        serde_json::from_str(opened["genesis"].as_str().unwrap()).unwrap();
    let mut replica = InMemoryDocStore::new(&genesis, 21);
    let payload = |b64: &Value, store: &dyn DocStore| SyncPayload {
        adapter: store.adapter_id().into(),
        bytes: base64::decode(b64.as_str().unwrap()).unwrap(),
    };
    replica
        .import_updates(&payload(&opened["updates"], &replica))
        .unwrap();
    // A person edit travels up as an update.
    let before = replica.cursor();
    vellum_core::ops::apply(
        &mut replica,
        &mut SequentialIds::new(),
        vellum_core::ops::Transaction::new(
            vellum_types::Origin::new(alice()),
            vec![rename("frameHero1", "Renamed in the editor")],
        ),
    )
    .unwrap();
    let up = base64::encode(&replica.export_updates(&before).bytes);
    let pushed = editor(
        &mut server,
        1,
        json!({"action":"push","path":"/design/app.vellum","editor":"tab-1","updates":up}),
    );
    assert_eq!(pushed["result"]["changed"], true, "{pushed}");
    // Another connection cannot push as this editor.
    server.opened(2, Some("/design".to_owned()));
    rpc(
        &mut server,
        2,
        1,
        "initialize",
        json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"TEST DATA other"}}),
    );
    assert!(
        editor(
            &mut server,
            2,
            json!({"action":"push","path":"/design/app.vellum","editor":"tab-1","updates":up})
        )
        .get("error")
        .is_some()
    );
    // An agent's write is pulled back down, and the file waits for the editor's save.
    call(
        &mut server,
        "open_document",
        json!({"path": "/design/app.vellum"}),
    );
    let write = call(
        &mut server,
        "update_nodes",
        json!({"updates": [{"id": "frameHero1", "set": {"name": "Renamed by an agent"}}]}),
    );
    assert_eq!(write["result"]["isError"], false, "{write}");
    let cursor = base64::encode(&replica.cursor().0);
    let pulled = editor(
        &mut server,
        1,
        json!({"action":"pull","path":"/design/app.vellum","editor":"tab-1","cursor":cursor}),
    )["result"]
        .clone();
    replica
        .import_updates(&payload(&pulled["updates"], &replica))
        .unwrap();
    assert_eq!(
        vellum_contracts::samples::find(&replica.snapshot(), "frameHero1")
            .unwrap()
            .name(),
        "Renamed by an agent"
    );
    // The host's Save, snapshot and revert go through the daemon's engine.
    let snapshot = editor(
        &mut server,
        1,
        json!({"action":"snapshot","path":"/design/app.vellum","editor":"tab-1"}),
    )["result"]
        .clone();
    let bytes = base64::decode(snapshot["bytes"].as_str().unwrap()).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("Renamed by an agent"));
    let reverted = editor(
        &mut server,
        1,
        json!({"action":"revert","path":"/design/app.vellum","editor":"tab-1"}),
    );
    assert!(
        reverted["result"]["revision"].as_u64().unwrap() > snapshot["revision"].as_u64().unwrap(),
        "{reverted}"
    );
    assert_eq!(
        editor(
            &mut server,
            1,
            json!({"action":"replace","path":"/design/app.vellum","editor":"tab-1","bytes":"","reason":"nonsense"})
        )["error"]["code"],
        -32602,
        "an unknown reason is refused"
    );
    assert_eq!(
        editor(
            &mut server,
            1,
            json!({"action":"replace","path":"/design/app.vellum","editor":"tab-1","bytes":"","reason":"recovery"})
        )["error"]["code"],
        -32000,
        "an empty backup cannot be read"
    );
    assert!(
        editor(
            &mut server,
            1,
            json!({"action":"save","path":"/design/app.vellum","editor":"tab-1"})
        )["result"]["revision"]
            .is_u64()
    );
    // Closing the connection lets go of its editors: the unsaved work goes with them.
    server.closed(1);
    assert!(server.documents().writer("/design/app.vellum").is_none());
    assert!(
        editor(
            &mut server,
            2,
            json!({"action":"pull","path":"/design/app.vellum","editor":"tab-1","cursor":""})
        )
        .get("error")
        .is_some(),
        "an editor that is gone cannot be pulled for"
    );
    assert!(
        !rpc(&mut server, 2, 5, "tools/list", json!({}))
            .to_string()
            .contains("vellum/editor")
    );
}
