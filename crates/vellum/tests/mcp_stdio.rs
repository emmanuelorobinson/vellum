//! TEST DATA. `vellum mcp` over real stdio, bridged to a real daemon on a real socket, working on real files
//! (guide Prompt 29, "MCP integration over stdio, headless").

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::time::{Duration, Instant};

use common::workspace::Workspace;
use common::{design, encode};
use serde_json::{Value, json};

/// `vellum mcp` started in the repo, as an MCP client would start it.
fn client(space: &Workspace) -> Client {
    let mut child = space
        .vellum()
        .arg("mcp")
        .current_dir(space.repo())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start vellum mcp");
    Client {
        stdin: child.stdin.take().unwrap(),
        stdout: BufReader::new(child.stdout.take().unwrap()),
        child,
        next: 0,
    }
}

struct Client {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    child: Child,
    next: u64,
}

impl Client {
    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn receive(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("a reply");
        assert!(!line.is_empty(), "the bridge closed before replying");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"))
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        let reply = self.receive();
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let reply = self.request("tools/call", json!({"name": tool, "arguments": arguments}));
        assert!(reply.get("error").is_none(), "{reply}");
        reply["result"].clone()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn text_of(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn an_mcp_client_reads_a_real_design_through_the_daemon() {
    let space = Workspace::new();
    let mut client = client(&space);

    let init = client.request(
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}),
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "vellum");
    client.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let tools = client.request("tools/list", json!({}));
    let names: Vec<_> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    for tool in ["get_app_state", "open_document", "get_nodes", "screenshot"] {
        assert!(names.contains(&tool.to_owned()), "{tool} in {names:?}");
    }

    // A relative path means a path in the directory the client was started in.
    let opened = client.call("open_document", json!({"path": "design/app.vellum"}));
    let document = opened["structuredContent"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    let expected = space
        .repo()
        .canonicalize()
        .unwrap()
        .join("design/app.vellum");
    assert_eq!(Path::new(&document), expected);
    assert_eq!(opened["structuredContent"]["mode"], "headless");
    assert_eq!(opened["structuredContent"]["pages"][0]["nodes"], 9);

    let nodes = client.call("get_nodes", json!({"ids": ["frameHero1"], "depth": 0}));
    assert_eq!(nodes["structuredContent"]["nodes"][0]["name"], "Hero");

    let state = client.call("get_app_state", json!({}));
    assert_eq!(state["structuredContent"]["active"], document);
    assert_eq!(
        state["structuredContent"]["context"][0]["node"],
        "ctxHero001"
    );

    // The real renderer, through the real daemon.
    let shot = client.call("screenshot", json!({"node": "rectCard01"}));
    let image = shot["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "image")
        .expect("an image");
    assert!(image["data"].as_str().unwrap().starts_with("iVBORw0KGgo"));

    // Documents are .vellum files inside the directory the client started in; nothing else is reachable.
    for path in ["../../../etc/passwd.vellum", "/etc/hosts", "design"] {
        let refused = client.call("open_document", json!({"path": path}));
        assert_eq!(refused["isError"], true, "{path}");
    }
}

#[test]
fn an_edit_made_outside_is_seen_by_the_next_read() {
    let space = Workspace::new();
    let mut client = client(&space);
    client.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}));
    client.call("open_document", json!({"path": "design/app.vellum"}));
    assert_eq!(
        client.call("get_nodes", json!({"ids": ["frameHero1"], "depth": 0}))["structuredContent"]["nodes"]
            [0]["name"],
        "Hero"
    );

    // Someone saves the file in another program.
    let mut doc = design();
    let mut json = serde_json::to_value(&doc).unwrap();
    json["pages"][0]["children"][0]["name"] = json!("Hero banner");
    doc = serde_json::from_value(json).unwrap();
    std::fs::write(space.repo().join("design/app.vellum"), encode(&doc)).unwrap();

    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let name = client.call("get_nodes", json!({"ids": ["frameHero1"], "depth": 0}))["structuredContent"]["nodes"][0]["name"].clone();
        if name == "Hero banner" {
            break;
        }
        assert!(
            Instant::now() < until,
            "the outside edit was never seen (still {name})"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn two_clients_share_one_daemon_and_one_document() {
    let space = Workspace::new();
    let mut first = client(&space);
    let mut second = client(&space);
    for client in [&mut first, &mut second] {
        client.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}));
    }
    first.call("open_document", json!({"path": "design/app.vellum"}));
    let seen = second.call("get_app_state", json!({}));
    assert_eq!(
        seen["structuredContent"]["documents"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(text_of(&seen).contains("design/app.vellum"));
}
