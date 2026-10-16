//! Turn-scoped MCP for installed engines. The ordinary `vellum mcp` bridge connects to a private endpoint,
//! whose requests are handed back to the engine's ToolBridge on the document thread. No second Session,
//! external-agent identity, or independent writer is created (ADR-007).
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    task::{Context, Poll},
    thread,
    time::{Duration, Instant},
};
use vellum_core::ports::{BridgeTool, EngineError, ToolReply};
use vellum_engine_claude_code::{
    ClaudeEngine, McpCall, McpChannel, McpHost, NativeLauncher, mailbox,
};
use vellum_ipc_local::{DaemonConfig, RpcHandler, RunDir};

pub struct LocalMcpHost {
    binary: PathBuf,
    base: PathBuf,
}
impl LocalMcpHost {
    /// `base` is the host's private runtime directory. A fresh random subdirectory belongs to each turn.
    pub fn new(binary: PathBuf, base: PathBuf) -> Self {
        Self { binary, base }
    }
}
/// Composition for later daemon/CLI/editor runners. Credentials stay with the installed program.
pub fn claude(
    program: PathBuf,
    repo: PathBuf,
    binary: PathBuf,
    private_runtime: PathBuf,
) -> ClaudeEngine {
    ClaudeEngine::new(
        Arc::new(NativeLauncher::new(program, repo)),
        Arc::new(LocalMcpHost::new(binary, private_runtime)),
    )
}
impl McpHost for LocalMcpHost {
    fn open(&self, tools: &[BridgeTool]) -> Result<Box<dyn McpChannel>, EngineError> {
        let base = RunDir::new(self.base.clone()).map_err(|_| EngineError::Unavailable)?;
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|_| EngineError::Unavailable)?;
        let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let dir = RunDir::new(base.path().join(name)).map_err(|_| EngineError::Unavailable)?;
        let (sender, receiver) = mailbox::channel();
        let schemas:Vec<Value>=tools.iter().map(|t|json!({"name":t.tool.name,"description":t.tool.description,"inputSchema":t.tool.parameters,"annotations":{"readOnlyHint":t.read_only}})).collect();
        let config = DaemonConfig {
            dir: dir.clone(),
            listen: None,
            allowed_origins: vec![],
            handler: Box::new(move || {
                Box::new(Handler {
                    sender,
                    tools: schemas,
                })
            }),
        };
        let worker = thread::Builder::new()
            .name("vellum-engine-mcp".into())
            .spawn(move || vellum_ipc_local::run(config))
            .map_err(|_| EngineError::Unavailable)?;
        let mut channel = LocalChannel {
            dir,
            receiver: Some(receiver),
            worker: Some(worker),
            binary: self.binary.clone(),
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if vellum_ipc_local::status(&channel.dir).is_ok() {
                return Ok(Box::new(channel));
            }
            if channel.worker.as_ref().is_some_and(|t| t.is_finished())
                || Instant::now() >= deadline
            {
                channel.close();
                return Err(EngineError::Unavailable);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
struct LocalChannel {
    dir: RunDir,
    receiver: Option<mailbox::Receiver>,
    worker: Option<thread::JoinHandle<std::io::Result<()>>>,
    binary: PathBuf,
}
impl LocalChannel {
    fn close(&mut self) {
        self.receiver.take(); // discard pending calls before waiting for the transport
        let _ = vellum_ipc_local::stop(&self.dir);
        if let Some(worker) = self.worker.take() {
            // stop acknowledges before draining. The server bounds that drain to three seconds.
            let _ = worker.join();
        }
        let _ = std::fs::remove_dir_all(self.dir.path());
    }
}
impl Drop for LocalChannel {
    fn drop(&mut self) {
        self.close();
    }
}
impl McpChannel for LocalChannel {
    fn config(&self) -> Value {
        json!({"mcpServers":{"vellum":{"type":"stdio","command":self.binary,"args":["--run-dir",self.dir.path(),"mcp"]}}})
    }
    fn poll_call(&mut self, cx: &mut Context<'_>) -> Poll<Option<McpCall>> {
        self.receiver
            .as_mut()
            .map_or(Poll::Ready(None), |r| r.poll(cx))
    }
}
struct Handler {
    sender: mailbox::Sender,
    tools: Vec<Value>,
}
fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
impl RpcHandler for Handler {
    fn request(&mut self, _: u64, request: Value) -> Option<Value> {
        let id = request.get("id")?.clone();
        let result = match request["method"].as_str()? {
            "initialize" => {
                json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"vellum","version":env!("CARGO_PKG_VERSION")},"instructions":"Use get_app_state first. Every canvas action uses the active turn."})
            }
            "ping" => json!({}),
            "tools/list" => json!({"tools":self.tools}),
            "tools/call" => {
                let params = &request["params"];
                let Some(name) = params["name"]
                    .as_str()
                    .filter(|n| self.tools.iter().any(|t| t["name"] == *n))
                else {
                    return Some(rpc_error(id, -32602, "Unknown tool"));
                };
                let input = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                if !input.is_object() {
                    return Some(rpc_error(id, -32602, "Tool arguments must be an object"));
                }
                let (tx, rx) = mpsc::sync_channel(1);
                if self
                    .sender
                    .send(McpCall {
                        name: name.into(),
                        input,
                        respond: Box::new(move |reply| {
                            let _ = tx.send(reply);
                        }),
                    })
                    .is_err()
                {
                    return Some(rpc_error(id, -32000, "Turn disconnected"));
                }
                let reply = loop {
                    match rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(reply) => break reply,
                        Err(mpsc::RecvTimeoutError::Timeout) if !self.sender.is_closed() => {
                            continue;
                        }
                        _ => return Some(rpc_error(id, -32000, "Turn disconnected")),
                    }
                };
                mcp_result(reply)
            }
            _ => return Some(rpc_error(id, -32601, "Method not found")),
        };
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}
/// Preserve screenshots as MCP image content, as on the ordinary daemon MCP endpoint.
fn mcp_result(reply: ToolReply) -> Value {
    let mut result = crate::mcp::tool_result(reply.value);
    result["isError"] = Value::Bool(reply.failed);
    result
}
