//! The MCP front end over the tool registry (ARCHITECTURE §3.4, §7.3).
//!
//! It speaks MCP's JSON-RPC to whatever the daemon's transport hands it: the stdio bridge (`vellum mcp`), the
//! `vellum tool` command and the opt-in HTTP listener all arrive here as one JSON value per request. It knows
//! nothing about what a tool does: `tools/list` is the registry's names and schemas, `tools/call` is
//! [`Registry::call`] with the result put into MCP's shapes. ADR-021 records why this is not `rmcp`.
//!
//! Requests are answered one at a time on the thread that owns the documents. Each connection has a
//! [`Session`] (its client, its working directory); the documents are shared by all of them.

pub(crate) mod cli;
mod share;

struct PermissionHost {
    prompt: Option<std::rc::Rc<dyn vellum_core::ports::PermissionPrompt>>,
    person: vellum_types::Actor,
    clock: std::rc::Rc<dyn vellum_core::ports::Clock>,
}

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};
use vellum_ipc_local::RpcHandler;
use vellum_types::brand;

use crate::daemon::{Documents, EditorId, Role};
use crate::sessions::SessionLog;
use crate::tools::{Client, Registry, Session, Step, ToolCtx, ToolError};

/// The protocol revisions with an `initialize` handshake that this server speaks, newest first.
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// Told to every client at `initialize`. Written for the model that reads it.
const INSTRUCTIONS: &str = "Vellum is a design canvas that lives in the repo. Call get_app_state first: it shows \
what is open, what the person has selected, which frames other agents hold, and any notes the person left for \
agents. Use open_document for another file. Read before changing anything: get_nodes, query_nodes, \
snapshot_layout and find_properties show the design, and screenshot draws it. Nodes are named by id. To \
change it, claim_frame the frame you work in, then use execute for anything that takes more than a few changes \
(one script, one undo step) or the typed tools (insert_nodes, update_nodes and the rest). For icons use search_icons then insert_icon, for real pictures \
fill_image, and import for an SVG or layers copied from Figma. Every call is all or \
nothing. When a call fails, the message says what to fix: the batch item (`op`), the field (`path`) or the script \
line (`line`).";

fn reply(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn fail(id: &Value, code: i64, message: impl Into<String>) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message.into()}})
}

fn server_info() -> Value {
    json!({"name": "vellum", "title": brand::NAME, "version": env!("CARGO_PKG_VERSION")})
}

fn capabilities() -> Value {
    json!({"tools": {"listChanged": false}})
}

/// An MCP server over a registry and the documents its tools work on.
pub struct McpServer {
    registry: Registry,
    docs: Documents,
    sessions: BTreeMap<u64, Session>,
    /// Where a connection that did not say where it was started is taken to be.
    default_cwd: Option<PathBuf>,
    /// Where each connection's tool calls are recorded as an external session; none in a daemon without one.
    log: Option<SessionLog>,
    cli_runs: BTreeMap<u64, cli::Run>,
    shares: Option<crate::daemon::share::Uploads>,
    permissions: Option<PermissionHost>,
    /// The editors each host connection opened (ADR-033), by document path and host-chosen name. A connection
    /// acts only for its own editors; closing it closes them.
    editors: BTreeMap<u64, Vec<(String, String)>>,
}

impl McpServer {
    /// A server whose tools work on `docs`.
    pub fn new(registry: Registry, docs: Documents) -> Self {
        Self {
            registry,
            docs,
            sessions: BTreeMap::new(),
            default_cwd: None,
            log: None,
            cli_runs: BTreeMap::new(),
            shares: None,
            permissions: None,
            editors: BTreeMap::new(),
        }
    }

    /// Hosts supply their trusted human prompt. None records unattended denials; it never grants access.
    pub fn with_permissions(
        mut self,
        prompt: Option<std::rc::Rc<dyn vellum_core::ports::PermissionPrompt>>,
        person: vellum_types::Actor,
        clock: std::rc::Rc<dyn vellum_core::ports::Clock>,
    ) -> Self {
        self.permissions = Some(PermissionHost {
            prompt,
            person,
            clock,
        });
        self
    }

    /// Records every connection's tool calls in `log` (ARCHITECTURE §7.6: external sessions keep tool calls only).
    pub fn with_session_log(mut self, log: Option<SessionLog>) -> Self {
        self.log = log;
        self
    }

    /// The session log, when there is one.
    pub fn session_log(&self) -> Option<&SessionLog> {
        self.log.as_ref()
    }

    /// Treats a connection that did not say where its client was started (an HTTP client) as started in `cwd`,
    /// so its documents are held to the same rule as everyone's.
    pub fn with_default_cwd(mut self, cwd: Option<PathBuf>) -> Self {
        self.default_cwd = cwd;
        self
    }

    fn session(&mut self, conn: u64) -> &mut Session {
        let default = &self.default_cwd;
        self.sessions.entry(conn).or_insert_with(|| match default {
            Some(cwd) => Session::in_dir(cwd.clone()),
            None => Session::default(),
        })
    }

    /// The documents, for the daemon's own business: editors attaching, the watcher.
    pub fn documents(&mut self) -> &mut Documents {
        &mut self.docs
    }

    /// The client on connection `conn`, once it has said who it is.
    pub fn client(&self, conn: u64) -> Option<&Client> {
        self.sessions.get(&conn)?.client.as_ref()
    }

    /// The agent connection `conn` is, once it has called a tool.
    pub fn agent(&self, conn: u64) -> Option<&vellum_types::AgentId> {
        self.sessions.get(&conn)?.agent()
    }

    /// The steps connection `conn` has taken, for the Agent panel.
    pub fn steps(&self, conn: u64) -> &[Step] {
        self.sessions.get(&conn).map_or(&[], Session::steps)
    }

    fn tools_list(&self) -> Value {
        let tools: Vec<Value> = self
            .registry
            .tools()
            .map(|tool| {
                json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "inputSchema": tool.input_schema(),
                    "annotations": {
                        "readOnlyHint": tool.read_only(),
                        "destructiveHint": tool.destructive(),
                        "openWorldHint": tool.open_world(),
                    },
                    "_meta": {"vellum/version": tool.version()},
                })
            })
            .collect();
        json!({"tools": tools})
    }

    fn initialize(&mut self, conn: u64, params: &Value) -> Result<Value, String> {
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .ok_or("initialize needs a protocolVersion string")?;
        let version = VERSIONS
            .iter()
            .find(|v| **v == requested)
            .unwrap_or(&VERSIONS[0]);
        let info = params.get("clientInfo");
        let text = |key: &str| {
            info.and_then(|i| i.get(key))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        self.session(conn).client = Some(Client {
            name: text("name"),
            version: text("version"),
        });
        Ok(json!({
            "protocolVersion": version,
            "capabilities": capabilities(),
            "serverInfo": server_info(),
            "instructions": INSTRUCTIONS,
        }))
    }

    fn discover() -> Value {
        json!({
            "resultType": "complete",
            "supportedVersions": VERSIONS,
            "capabilities": capabilities(),
            "instructions": INSTRUCTIONS,
            "ttlMs": 0,
            "cacheScope": "private",
            "_meta": {"io.modelcontextprotocol/serverInfo": server_info()},
        })
    }

    fn tools_call(&mut self, conn: u64, params: &Value) -> Result<Value, String> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or("tools/call needs a tool name")?;
        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => json!({}),
            Some(args @ Value::Object(_)) => args.clone(),
            Some(_) => return Err("arguments is an object".into()),
        };
        if self.registry.get(name).is_none() {
            return Err(format!(
                "Unknown tool {name}. The tools are: {}.",
                self.registry
                    .tools()
                    .map(|t| t.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if self.cli_runs.get(&conn).is_some_and(|r| r.is_ended())
            && self
                .registry
                .get(name)
                .is_some_and(|tool| !tool.read_only())
        {
            return Ok(tool_error(&ToolError::new(
                crate::tools::ErrorKind::Refused,
                "This CLI turn has ended; start a new turn before changing the document",
            )));
        }
        // Whatever changed on disk since the last call is applied first, so a read sees the file as it is.
        self.docs.poll();
        self.session(conn);
        if let (Some(host), Some(log)) = (&self.permissions, &self.log) {
            let broker = vellum_core::permissions::Broker::new(
                Default::default(),
                log.shared_store(),
                host.clock.clone(),
                host.prompt.clone(),
                host.person.clone(),
            );
            self.docs
                .set_outside(std::rc::Rc::new(crate::permissions::Brokered::new(
                    std::rc::Rc::new(broker),
                    None,
                )));
        }
        let session = self.sessions.get_mut(&conn).expect("just made");
        let mut ctx = ToolCtx::new(&mut self.docs, session);
        let result = self.registry.call(&mut ctx, name, arguments);
        let step = session.steps().last().cloned();
        if let Some(log) = &self.log
            && let Err(error) = log.record_steps(&mut self.docs, session)
        {
            // The call itself worked; a log that cannot be written must not fail it. The message holds no
            // event contents.
            eprintln!("vellum: could not record the session: {error}");
        }
        let mut result = match result {
            Ok(value) => tool_result(value),
            Err(error) => tool_error(&error),
        };
        if self.cli_runs.contains_key(&conn) {
            result["_meta"] = json!({"vellum/step": step});
            self.cli_fill(conn);
        }
        Ok(result)
    }

    // Host metadata only: never document contents, event bodies, credentials or discovery data.
    fn host_state(&self, conn: u64, params: &Value) -> Result<Value, (i64, String)> {
        use std::path::{Component, Path};
        use vellum_core::ports::SessionKind;
        let invalid = || {
            (
                -32602,
                "Host state needs at most 20 design paths inside the host workspace".to_owned(),
            )
        };
        if self.client(conn).is_none() {
            return Err((-32002, "Initialize the host connection first".to_owned()));
        }
        let object = params.as_object().ok_or_else(invalid)?;
        if object.len() != 1 {
            return Err(invalid());
        }
        let paths = object
            .get("paths")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        if paths.len() > 20 {
            return Err(invalid());
        }
        let cwd = self.sessions.get(&conn).and_then(|s| s.cwd.as_ref());
        let mut sessions = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for value in paths {
            let path = Path::new(value.as_str().ok_or_else(invalid)?);
            if !path.is_absolute()
                || path.extension().is_none_or(|e| e != "vellum")
                || path
                    .components()
                    .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
                || cwd.is_none_or(|root| !path.starts_with(root))
            {
                return Err(invalid());
            }
            if let Some(log) = &self.log {
                for meta in log
                    .metadata(path)
                    .map_err(|_| (-32000, "Sessions unavailable".to_owned()))?
                {
                    if !seen.insert(meta.id.clone()) {
                        continue;
                    }
                    let client = match meta.kind {
                        SessionKind::Engine { engine } => engine,
                        SessionKind::External { client } => client,
                    };
                    let turns = log
                        .store()
                        .events(
                            &meta.id,
                            Some(vellum_core::ports::EventSeq(
                                meta.events.saturating_sub(100),
                            )),
                        )
                        .map_err(|_| (-32000, "Session state unavailable".to_owned()))?;
                    let turn = turns.iter().rev().find_map(|event| {
                        let vellum_core::ports::EventBody::Turn(transition) = &event.body else {
                            return None;
                        };
                        use vellum_core::turns::TurnState;
                        let kind = match transition.to {
                            TurnState::Done => "done",
                            TurnState::Failed => "failed",
                            TurnState::AwaitingPermission => "permission",
                            TurnState::Stopped => "stopped",
                            _ => "working",
                        };
                        Some(json!({"seq":event.seq.0,"kind":kind}))
                    });
                    sessions.push(json!({"id":meta.id.as_str(), "path":path.to_string_lossy(), "title":meta.title, "client":client, "updatedAt":meta.updated_at.0,"turn":turn}));
                }
            }
        }
        sessions.sort_by_key(|s| std::cmp::Reverse(s["updatedAt"].as_i64().unwrap_or(0)));
        sessions.truncate(100);
        let clients: std::collections::BTreeSet<_> = self
            .sessions
            .values()
            .filter_map(|s| s.client.as_ref().map(|c| c.name.as_str()))
            .collect();
        Ok(
            json!({"editorProtocol":1,"clients":clients,"toolCount":self.registry.tools().count(),"sessions":sessions,"sessionsAvailable":self.log.is_some()}),
        )
    }

    fn host_recovery(&mut self, conn: u64, params: &Value) -> Result<Value, (i64, String)> {
        use std::path::{Component, Path};
        let invalid = || {
            (
                -32602,
                "Recovery needs a design path in the host workspace and a supported action"
                    .to_owned(),
            )
        };
        if self.client(conn).is_none() {
            return Err((-32002, "Initialize the host connection first".to_owned()));
        }
        let object = params.as_object().ok_or_else(invalid)?;
        let path = object
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let action = object
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let root = self.sessions.get(&conn).and_then(|s| s.cwd.as_ref());
        let scoped = |p: &str| {
            let p = Path::new(p);
            p.is_absolute()
                && p.extension().is_some_and(|e| e == "vellum")
                && !p
                    .components()
                    .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
                && root.is_some_and(|r| p.starts_with(r))
        };
        if !scoped(path) || object.len() != if action == "saveAs" { 3 } else { 2 } {
            return Err(invalid());
        }
        match action {
            "find" => {
                let (copies, _) = self.docs.recoveries();
                let saved_at = copies.iter().find(|c| c.path == path).map(|c| c.saved_at.0);
                Ok(json!({"savedAt": saved_at}))
            }
            "save" => {
                self.docs.restore(path).and_then(|_| self.docs.save(path))
                    .map_err(|_| (-32000, "Recovery could not be saved; the backup is retained until a successful save".to_owned()))?;
                Ok(json!({"done": true}))
            }
            "saveAs" => {
                let destination = object
                    .get("destination")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid)?;
                if !scoped(destination) {
                    return Err(invalid());
                }
                self.docs.restore_as(path, destination).map_err(|_| {
                    (
                        -32000,
                        "Recovery could not be saved as a new file; the backup is retained"
                            .to_owned(),
                    )
                })?;
                Ok(json!({"done": true}))
            }
            "discard" => {
                self.docs
                    .discard_recovery(path)
                    .map_err(|_| (-32000, "Backup could not be discarded".to_owned()))?;
                Ok(json!({"done": true}))
            }
            "closeHeadless" => {
                if !self.docs.close_headless(path) {
                    return Err((-32000, "A live editor holds this file".to_owned()));
                }
                Ok(json!({"done": true}))
            }
            _ => Err(invalid()),
        }
    }

    /// The Agent tab's feed for one document: its sessions, their events since a point, a step's picture, and the
    /// person's controls (remove a session, undo a turn, stop its agent). Host-only, outside the agent tool
    /// registry; scoped to the connection's workspace like `vellum/editor`.
    fn host_agent_feed(&mut self, conn: u64, params: &Value) -> Result<Value, (i64, String)> {
        use vellum_core::ports::{EventBody, EventSeq, ImageRef, SessionKind};
        use vellum_core::sessions::{body_to_json, origin_to_json};
        let invalid = || {
            (
                -32602,
                "The agent feed needs a design path in the host workspace and a supported action"
                    .to_owned(),
            )
        };
        let unavailable = || (-32000, "Sessions are unavailable".to_owned());
        if self.client(conn).is_none() {
            return Err((-32002, "Initialize the host connection first".to_owned()));
        }
        let object = params.as_object().ok_or_else(invalid)?;
        let text = |key: &str| object.get(key).and_then(Value::as_str).ok_or_else(invalid);
        let action = text("action")?;
        let path = text("path")?;
        let root = self
            .sessions
            .get(&conn)
            .and_then(|s| s.cwd.clone())
            .or_else(|| self.default_cwd.clone());
        if !scoped_design(root.as_ref(), path) {
            return Err(invalid());
        }
        let log = self.log.as_ref().ok_or_else(unavailable)?;
        if action == "create" {
            use vellum_core::ports::NewSession;
            let id = vellum_types::SessionId::new(self.docs.ulid()).map_err(|_| invalid())?;
            let agent = vellum_types::AgentId::new(self.docs.ulid()).map_err(|_| invalid())?;
            let now = self.docs.now();
            let doc = log
                .document_key(std::path::Path::new(path))
                .map_err(|_| unavailable())?;
            log.store()
                .create(NewSession {
                    id: id.clone(),
                    doc,
                    kind: SessionKind::Engine {
                        engine: "codex".into(),
                    },
                    agent: agent.clone(),
                    title: "New session".into(),
                    created_at: now,
                })
                .map_err(|_| unavailable())?;
            return Ok(
                json!({"id":id.as_str(),"agent":agent.as_str(),"kind":{"type":"engine","engine":"codex"},"title":"New session","createdAt":now.0,"updatedAt":now.0,"state":null,"agents":1}),
            );
        }
        let metas = log
            .metadata(std::path::Path::new(path))
            .map_err(|_| unavailable())?;
        if action == "sessions" {
            let connected: std::collections::BTreeSet<_> = self
                .sessions
                .values()
                .filter_map(|s| s.agent().cloned())
                .collect();
            let mut rows = vec![];
            for meta in &metas {
                let recent = log
                    .store()
                    .events(&meta.id, Some(EventSeq(meta.events.saturating_sub(100))))
                    .map_err(|_| unavailable())?;
                let state = recent.iter().rev().find_map(|e| match &e.body {
                    EventBody::Turn(change) => Some(change.to.as_str()),
                    _ => None,
                });
                let (kind, external) = match &meta.kind {
                    SessionKind::Engine { engine } => {
                        (json!({"type":"engine","engine":engine}), false)
                    }
                    SessionKind::External { client } => {
                        (json!({"type":"external","client":client}), true)
                    }
                };
                let mut row = json!({
                    "id": meta.id.as_str(), "kind": kind, "agent": meta.agent.as_str(),
                    "title": meta.title, "createdAt": meta.created_at.0, "updatedAt": meta.updated_at.0,
                    "state": state, "agents": 1,
                });
                if external {
                    row["connected"] = json!(connected.contains(&meta.agent));
                }
                rows.push(row);
            }
            rows.sort_by_key(|r| std::cmp::Reverse(r["updatedAt"].as_i64().unwrap_or(0)));
            return Ok(Value::Array(rows));
        }
        if action == "image" {
            let hash = ImageRef::new(text("hash")?).ok_or_else(invalid)?;
            let bytes = log
                .store()
                .image(&hash)
                .map_err(|_| unavailable())?
                .ok_or((-32000, "No such picture".to_owned()))?;
            return Ok(json!({"mime":"image/png","base64":vellum_core::base64::encode(&bytes)}));
        }
        let meta = metas
            .iter()
            .find(|m| Some(m.id.as_str()) == object.get("session").and_then(Value::as_str))
            .ok_or((-32000, "No such session for this document".to_owned()))?
            .clone();
        match action {
            "events" => {
                let since = object.get("since").and_then(Value::as_u64).unwrap_or(0);
                let events = log
                    .store()
                    .events(&meta.id, Some(EventSeq(since)))
                    .map_err(|_| unavailable())?;
                Ok(Value::Array(
                    events
                        .iter()
                        .filter(|e| e.seq.0 > since)
                        .map(|e| {
                            json!({"seq": e.seq.0, "at": e.at.0, "origin": origin_to_json(&e.origin),
                                   "body": body_to_json(&e.body)})
                        })
                        .collect(),
                ))
            }
            "remove" => {
                log.store().delete(&meta.id).map_err(|_| unavailable())?;
                Ok(json!({"done": true}))
            }
            "stop" => {
                self.docs
                    .control_all(&vellum_core::guard::AgentControl::Stop(meta.agent));
                Ok(json!({"done": true}))
            }
            "undoTurn" => {
                let device = vellum_types::DeviceId::new(text("device")?).map_err(|_| invalid())?;
                let turn = match object.get("turn").and_then(Value::as_str) {
                    Some(turn) => vellum_types::TurnId::new(turn).map_err(|_| invalid())?,
                    None => log
                        .store()
                        .events(&meta.id, None)
                        .map_err(|_| unavailable())?
                        .iter()
                        .rev()
                        .find_map(|e| match &e.body {
                            EventBody::Turn(change) => Some(change.turn.clone()),
                            _ => None,
                        })
                        .ok_or((-32000, "The session has no turn to undo".to_owned()))?,
                };
                let undone = self
                    .docs
                    .target(path)
                    .and_then(|t| t.undo_turn(&turn, &vellum_types::Actor::Person { device }))
                    .map_err(|e| (-32000, e.to_string()))?;
                Ok(json!({"undone": undone != vellum_core::ports::TurnUndone::Nothing}))
            }
            _ => Err(invalid()),
        }
    }

    /// An editor's replica of a document (ADR-033): open, push its person's updates, pull everyone else's, record
    /// its save, close. Host-only, outside the agent tool registry. Editors are named per connection, so one
    /// connection never acts for another's editor.
    fn host_editor(&mut self, conn: u64, params: &Value) -> Result<Value, (i64, String)> {
        use vellum_core::base64;
        use vellum_core::ports::{SyncCursor, SyncPayload};
        let invalid = || {
            (
                -32602,
                "Editors need a design path in the host workspace, an editor name and a supported action"
                    .to_owned(),
            )
        };
        if self.client(conn).is_none() {
            return Err((-32002, "Initialize the host connection first".to_owned()));
        }
        let object = params.as_object().ok_or_else(invalid)?;
        let text = |key: &str| object.get(key).and_then(Value::as_str).ok_or_else(invalid);
        let (action, path, name) = (text("action")?, text("path")?, text("editor")?);
        let extra: &[&str] = match action {
            "open" => &["device"],
            "push" => &["updates"],
            "pull" => &["cursor"],
            "replace" => &["bytes", "reason"],
            "saved" | "close" | "snapshot" | "save" | "revert" => &[],
            _ => return Err(invalid()),
        };
        let root = self.sessions.get(&conn).and_then(|s| s.cwd.as_ref());
        if !scoped_design(root, path)
            || name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || object.len() != 3 + extra.len()
            || extra.iter().any(|k| !object.contains_key(*k))
        {
            return Err(invalid());
        }
        let id = EditorId(format!("{conn}/{name}"));
        let mine = self
            .editors
            .get(&conn)
            .is_some_and(|list| list.iter().any(|(p, n)| p == path && n == name));
        if action != "open" && !mine {
            return Err((-32000, "This editor is not open".to_owned()));
        }
        let failed = |e: vellum_core::ports::TargetError| (-32000, e.to_string());
        match action {
            "open" => {
                let device = vellum_types::DeviceId::new(text("device")?).map_err(|_| invalid())?;
                let (start, recovery) = self
                    .docs
                    .attach_editor(path, id, vellum_types::Actor::Person { device })
                    .map_err(failed)?;
                if !mine {
                    self.editors
                        .entry(conn)
                        .or_default()
                        .push((path.to_owned(), name.to_owned()));
                }
                let genesis = vellum_codec_json::JsonCodec::new()
                    .encode(&start.genesis)
                    .map_err(|e| (-32000, e.to_string()))?;
                let revision = self.docs.editor_revision(path).map_err(failed)?;
                Ok(json!({
                    "revision": revision,
                    "role": if start.role == Role::Writer { "writer" } else { "attached" },
                    "adapter": start.updates.adapter,
                    "genesis": String::from_utf8_lossy(&genesis),
                    "updates": base64::encode(&start.updates.bytes),
                    "recovery": recovery.map(|r| json!({"savedAt": r.saved_at.0})),
                }))
            }
            "push" => {
                let bytes = base64::decode(text("updates")?).ok_or_else(invalid)?;
                let adapter = self.docs.editor_adapter(path).map_err(failed)?;
                let changes = self
                    .docs
                    .editor_push(
                        path,
                        &id,
                        &SyncPayload {
                            adapter: adapter.into(),
                            bytes,
                        },
                    )
                    .map_err(failed)?;
                let revision = self.docs.editor_revision(path).map_err(failed)?;
                Ok(json!({"changed": !changes.is_empty(), "revision": revision}))
            }
            "pull" => {
                let cursor = SyncCursor(base64::decode(text("cursor")?).ok_or_else(invalid)?);
                let updates = self.docs.editor_pull(path, &cursor).map_err(failed)?;
                let revision = self.docs.editor_revision(path).map_err(failed)?;
                Ok(json!({"updates": base64::encode(&updates.bytes), "revision": revision}))
            }
            "saved" => {
                self.docs.editor_wrote(path).map_err(failed)?;
                Ok(json!({"done": true}))
            }
            "snapshot" => {
                let (revision, doc) = self.docs.editor_snapshot(path).map_err(failed)?;
                let bytes = vellum_codec_json::JsonCodec::new()
                    .encode(&doc)
                    .map_err(|e| (-32000, e.to_string()))?;
                Ok(json!({"revision": revision, "bytes": base64::encode(&bytes)}))
            }
            "revert" => {
                let revision = self.docs.editor_revert(path).map_err(failed)?;
                Ok(json!({"revision": revision}))
            }
            "save" => {
                self.docs.save(path).map_err(failed)?;
                let revision = self.docs.editor_revision(path).map_err(failed)?;
                Ok(json!({"revision": revision}))
            }
            "replace" => {
                let revision = match text("reason")? {
                    "recovery" => {
                        let bytes = base64::decode(text("bytes")?).ok_or_else(invalid)?;
                        let doc = vellum_codec_json::decode(&bytes)
                            .map_err(|e| (-32000, e.to_string()))?;
                        self.docs
                            .editor_replace(path, &doc, crate::daemon::Replace::Recovery)
                    }
                    // The daemon's watcher merges external changes itself; the host only learns where it is.
                    "external" => {
                        self.docs.poll();
                        self.docs.editor_revision(path)
                    }
                    _ => return Err(invalid()),
                }
                .map_err(failed)?;
                Ok(json!({"revision": revision}))
            }
            _ => {
                self.docs.detach_editor(path, &id);
                if let Some(list) = self.editors.get_mut(&conn) {
                    list.retain(|(p, n)| !(p == path && n == name));
                }
                Ok(json!({"done": true}))
            }
        }
    }

    /// Native CLI lifecycle actions stay outside the agent tool registry. Resolve through the same
    /// session scope and document target as tool calls; no direct document-store access.
    fn cli_action(&mut self, conn: u64, params: &Value) -> Result<Value, (i64, String)> {
        if self.client(conn).is_none() {
            return Err((-32002, "Initialize the CLI connection first".into()));
        }
        let action = params["action"]
            .as_str()
            .ok_or((-32602, "Missing CLI action".into()))?;
        if !matches!(action, "save" | "new") {
            return self.cli_turn_action(conn, params);
        }
        let file = params["document"]
            .as_str()
            .ok_or((-32602, "Missing document".into()))?;
        self.session(conn);
        let session = self.sessions.get_mut(&conn).expect("session");
        let ctx = ToolCtx::new(&mut self.docs, session);
        let path = ctx.resolve(file).map_err(|e| (-32602, e.to_string()))?;
        match action {
            "save" => {
                self.docs.save(&path).map_err(|e| (-32000, e.to_string()))?;
                Ok(json!({"saved":true,"document":path}))
            }
            "new" => {
                if self
                    .docs
                    .exists(&path)
                    .map_err(|e| (-32000, e.to_string()))?
                {
                    return Err((
                        -32000,
                        "The destination already exists; choose another file".into(),
                    ));
                }
                let name = std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Design");
                self.docs
                    .create_blank(&path, name)
                    .map_err(|e| (-32000, e.to_string()))?;
                Ok(json!({"created":true,"document":path}))
            }
            _ => Err((-32602, "Unknown CLI action".into())),
        }
    }

    /// Answers one JSON-RPC request. `None` for a notification.
    pub fn handle(&mut self, conn: u64, request: &Value) -> Option<Value> {
        let method = request.get("method")?.as_str()?;
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        // No id, no answer: whatever it was, it is a notification.
        let id = request.get("id")?;
        let outcome = match method {
            "vellum/share" => self.share_action(conn, &params),
            "vellum/cli" => self.cli_action(conn, &params),
            "vellum/hostState" => self.host_state(conn, &params),
            "vellum/recovery" => self.host_recovery(conn, &params),
            "vellum/editor" => self.host_editor(conn, &params),
            "vellum/agentFeed" => self.host_agent_feed(conn, &params),
            "initialize" => self.initialize(conn, &params).map_err(|m| (-32602, m)),
            "ping" => Ok(json!({})),
            // Host control is separate from the agent tool registry. The local authenticated connection
            // still initializes first; the browser can request this action only through its host.
            "vellum/pauseAll" => {
                if self.client(conn).is_none() {
                    Err((-32002, "Initialize the host connection first".to_owned()))
                } else if params.as_object().is_none_or(|p| !p.is_empty()) {
                    Err((-32602, "Pause all takes no parameters".to_owned()))
                } else {
                    self.docs
                        .control_all(&vellum_core::guard::AgentControl::PauseAll);
                    Ok(json!({"paused": true}))
                }
            }
            "server/discover" => Ok(Self::discover()),
            "tools/list" => Ok(self.tools_list()),
            "tools/call" => self.tools_call(conn, &params).map_err(|m| (-32602, m)),
            other => Err((-32601, format!("Method not found: {other}"))),
        };
        Some(match outcome {
            Ok(result) => reply(id, result),
            Err((code, message)) => fail(id, code, message),
        })
    }
}

/// A tool's value as MCP content: the value as compact JSON, and a picture as an image block instead of text.
pub(crate) fn tool_result(mut value: Value) -> Value {
    let mut content = Vec::new();
    let mut image = None;
    if let Some(picture) = value.get_mut("image").and_then(Value::as_object_mut)
        && let (Some(Value::String(data)), Some(Value::String(mime))) =
            (picture.remove("base64"), picture.get("mime").cloned())
    {
        image = Some((data, mime));
    }
    content.push(json!({"type": "text", "text": value.to_string()}));
    if let Some((data, mime)) = image {
        content.push(json!({"type": "image", "data": data, "mimeType": mime}));
    }
    json!({"content": content, "structuredContent": value, "isError": false})
}

fn tool_error(error: &ToolError) -> Value {
    json!({
        "content": [{"type": "text", "text": error.to_string()}],
        "structuredContent": error.to_json(),
        "isError": true,
    })
}

impl RpcHandler for McpServer {
    fn opened(&mut self, conn: u64, cwd: Option<String>) {
        let cwd = cwd.map(PathBuf::from);
        let session = self.session(conn);
        if cwd.is_some() {
            session.cwd = cwd;
        }
    }

    fn request(&mut self, conn: u64, request: Value) -> Option<Value> {
        self.handle(conn, &request)
    }

    fn idle(&mut self) {
        self.docs.poll();
        self.docs.tick();
    }

    fn closed(&mut self, conn: u64) {
        for (path, name) in self.editors.remove(&conn).unwrap_or_default() {
            self.docs
                .detach_editor(&path, &EditorId(format!("{conn}/{name}")));
        }
        self.cli_disconnected(conn);
        self.sessions.remove(&conn);
    }
}

/// A `.vellum` path inside the host connection's workspace, with no `.` or `..` parts.
fn scoped_design(root: Option<&PathBuf>, path: &str) -> bool {
    use std::path::{Component, Path};
    let p = Path::new(path);
    p.is_absolute()
        && p.extension().is_some_and(|e| e == "vellum")
        && !p
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        && root.is_some_and(|r| p.starts_with(r))
}
