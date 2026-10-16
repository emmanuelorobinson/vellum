//! Installed Codex, with a turn-scoped MCP connection to the host's ToolBridge (ADR-007).
//! The host supplies MCP transport; this adapter never imports another adapter.
mod native;
mod protocol;
pub mod recording;
pub use native::NativeLauncher;
pub use protocol::{Installation, Launch, Mode, SUPPORTED_VERSIONS, UsageLimits, UsageWindow};

use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    future::{Future, poll_fn},
    pin::pin,
    sync::{Arc, Mutex},
    task::Poll,
};
use vellum_core::ports::*;
use vellum_types::TurnId;

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
/// Injected process boundary. Inspection is explicit; availability never runs a subprocess.
pub trait Launcher: Send + Sync {
    fn inspect(&self) -> Installation;
    fn start(&self, launch: Launch, cancel: Cancellation) -> Result<Box<dyn Process>, EngineError>;
}
pub trait Process {
    /// Bounded NDJSON records. EOF without a result is an error, never success.
    fn next(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>>;
    fn send(&mut self, value: Value) -> EngineFuture<'_, Result<(), EngineError>>;
}
pub use vellum_core::ports::{McpCall, McpChannel, McpHost};

pub struct CodexEngine {
    launcher: Arc<dyn Launcher>,
    host: Arc<dyn McpHost>,
    installation: Mutex<Installation>,
    reset_at: Mutex<Option<i64>>,
    usage_limits: Mutex<Option<protocol::UsageLimits>>,
    running: Mutex<BTreeMap<TurnId, Cancellation>>,
}
impl CodexEngine {
    pub fn new(launcher: Arc<dyn Launcher>, host: Arc<dyn McpHost>) -> Self {
        let installation = launcher.inspect();
        Self {
            launcher,
            host,
            installation: Mutex::new(installation),
            reset_at: Mutex::new(None),
            usage_limits: Mutex::new(None),
            running: Mutex::new(BTreeMap::new()),
        }
    }
    pub fn installation(&self) -> Installation {
        self.installation.lock().unwrap().clone()
    }
    pub fn recheck(&self) -> Installation {
        let next = self.launcher.inspect();
        *self.installation.lock().unwrap() = next.clone();
        *self.reset_at.lock().unwrap() = None;
        *self.usage_limits.lock().unwrap() = None;
        next
    }
    /// None when the engine did not report a reset time. UTC milliseconds; no guessed window.
    pub fn usage_resets_at(&self) -> Option<i64> {
        *self.reset_at.lock().unwrap()
    }
    pub fn usage_limits(&self) -> Option<protocol::UsageLimits> {
        self.usage_limits.lock().unwrap().clone()
    }
    fn status(&self, status: EngineStatus) {
        self.installation.lock().unwrap().status = status;
    }
}
impl AgentEngine for CodexEngine {
    fn id(&self) -> &str {
        "codex"
    }
    fn availability(&self) -> EngineStatus {
        self.installation.lock().unwrap().status
    }
    fn run_turn<'a>(
        &'a self,
        turn: TurnRequest,
        bridge: &'a dyn ToolBridge,
    ) -> Result<Box<dyn TurnStream + 'a>, EngineError> {
        if self.availability() != EngineStatus::Ready {
            return Err(EngineError::Unavailable);
        }
        protocol::validate(&turn)?;
        let mode = self.installation().mode.ok_or(EngineError::Unavailable)?;
        if mode == Mode::Exec && !turn.images.is_empty() {
            return Err(EngineError::InvalidRequest);
        }
        let cancel = Cancellation::default();
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(&turn.turn) {
                return Err(EngineError::Busy);
            }
            running.insert(turn.turn.clone(), cancel.clone());
        }
        Ok(Box::new(Handle {
            engine: self,
            mode,
            id: turn.turn.clone(),
            cancel,
            bridge,
            limits: turn.limits,
            request: Some(turn.clone()),
            pending: None,
            channel: None,
            process: None,
            decoder: protocol::Decoder::new(mode, turn),
            queue: VecDeque::new(),
            log: vec![],
            done: false,
            calls: 0,
            failures: 0,
        }))
    }
    fn interrupt(&self, turn: &TurnId) {
        if let Some(cancel) = self.running.lock().unwrap().get(turn) {
            cancel.cancel();
        }
    }
}
struct Handle<'a> {
    engine: &'a CodexEngine,
    mode: Mode,
    id: TurnId,
    cancel: Cancellation,
    bridge: &'a dyn ToolBridge,
    request: Option<TurnRequest>,
    limits: TurnLimits,
    pending: Option<(McpCall, String, Option<Value>)>,
    channel: Option<Box<dyn McpChannel>>,
    process: Option<Box<dyn Process>>,
    decoder: protocol::Decoder,
    queue: VecDeque<TurnEvent>,
    log: Vec<ChatMessage>,
    done: bool,
    calls: u64,
    failures: u32,
}
impl Handle<'_> {
    fn finish(&mut self, end: TurnEnd) {
        self.cancel.cancel(); // also terminates the owned process tree on success/failure/drop
        self.process = None;
        self.pending = None;
        self.channel = None;
        if end == TurnEnd::Stopped {
            self.queue.clear();
        }
        if !self.decoder.text.is_empty() {
            self.log.push(ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ChatPart::Text(std::mem::take(&mut self.decoder.text))],
            });
        }
        self.queue
            .push_back(TurnEvent::Messages(std::mem::take(&mut self.log)));
        self.queue.push_back(TurnEvent::Ended(end));
        self.done = true;
    }
    async fn advance(&mut self) -> Result<(), EngineError> {
        if let Some(turn) = self.request.take() {
            if self.cancel.is_cancelled() {
                self.finish(TurnEnd::Stopped);
                return Ok(());
            }
            self.log.push(protocol::user_message(&turn));
            let mode = self.mode;
            let config = if mode == Mode::Exec {
                let channel = self.engine.host.open(&self.bridge.tools())?;
                let config = channel.config();
                self.channel = Some(channel);
                config
            } else {
                Value::Null
            };
            self.decoder.set_tools(self.bridge.tools());
            let launch = Launch::for_turn(&turn, config, mode)?;
            self.process = Some(self.engine.launcher.start(launch, self.cancel.clone())?);
            if let Some(init) = self.decoder.initial() {
                self.process.as_mut().expect("started").send(init).await?;
            }
        }
        if let Some((call, id, rpc_id)) = self.pending.take() {
            if self.cancel.is_cancelled() {
                self.finish(TurnEnd::Stopped);
                return Ok(());
            }
            let reply = self.bridge.call(&call.name, call.input.clone());
            let (reply, log_value, truncated) =
                bounded_reply(reply, self.limits.max_tool_result_chars);
            self.queue.push_back(TurnEvent::ToolFinished {
                call: id.clone(),
                name: call.name.clone(),
                failed: reply.failed,
                truncated,
            });
            self.log.push(ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ChatPart::ToolCall(ModelCall {
                    id: id.clone(),
                    name: call.name.clone(),
                    arguments: call.input,
                    metadata: Value::Null,
                })],
            });
            self.log.push(ChatMessage {
                role: ChatRole::User,
                content: vec![ChatPart::ToolResult {
                    id,
                    name: call.name,
                    result: log_value,
                    failed: reply.failed,
                }],
            });
            if reply.failed {
                self.failures += 1;
            }
            if let Some(id) = rpc_id {
                self.process
                    .as_mut()
                    .expect("started")
                    .send(protocol::tool_response(id, &reply)?)
                    .await?;
            } else {
                (call.respond)(reply);
            }

            if self.failures > self.limits.max_failed_calls {
                return Err(EngineError::TooManyFailedCalls);
            }
            return Ok(());
        }
        enum Input {
            Stop,
            Call(Option<McpCall>),
            Line(Result<Option<String>, EngineError>),
        }
        let input = {
            let process = self.process.as_mut().expect("started");
            let channel = &mut self.channel;
            let line = process.next();
            let mut line = pin!(line);
            poll_fn(|cx| {
                if self.cancel.poll_cancelled(cx).is_ready() {
                    return Poll::Ready(Input::Stop);
                }
                if let Some(channel) = channel.as_mut()
                    && let Poll::Ready(call) = channel.poll_call(cx)
                {
                    return Poll::Ready(Input::Call(call));
                }
                line.as_mut().poll(cx).map(Input::Line)
            })
            .await
        };
        match input {
            Input::Stop => self.finish(TurnEnd::Stopped),
            Input::Call(None) => return Err(EngineError::Model(ModelError::Unavailable)),
            Input::Call(Some(call)) => {
                if self.cancel.is_cancelled() {
                    self.finish(TurnEnd::Stopped);
                    return Ok(());
                }
                self.calls += 1;
                let id = format!("mcp-{}", self.calls);
                self.queue.push_back(TurnEvent::ToolStarted {
                    call: id.clone(),
                    name: call.name.clone(),
                    arguments: call.input.clone(),
                });
                self.pending = Some((call, id, None));
            }

            Input::Line(Err(error)) => return Err(error),
            Input::Line(Ok(None)) => return Err(EngineError::Model(ModelError::Truncated)),
            Input::Line(Ok(Some(line))) => {
                let decoded = self.decoder.accept(&line)?;
                if let Some(call) = decoded.call {
                    self.queue.push_back(TurnEvent::ToolStarted {
                        call: call.call_id.clone(),
                        name: call.name.clone(),
                        arguments: call.input.clone(),
                    });
                    self.pending = Some((
                        McpCall {
                            name: call.name,
                            input: call.input,
                            respond: Box::new(|_| {}),
                        },
                        call.call_id,
                        Some(call.id),
                    ));
                }
                for message in decoded.outgoing {
                    self.process
                        .as_mut()
                        .expect("started")
                        .send(message)
                        .await?;
                }
                if let Some(limits) = decoded.limits {
                    *self.engine.usage_limits.lock().unwrap() = Some(limits);
                }
                if let Some(status) = decoded.status {
                    self.engine.status(status);
                    let reset = if status == EngineStatus::UsageLimit {
                        self.engine.usage_limits().and_then(|l| {
                            [l.primary, l.secondary]
                                .into_iter()
                                .flatten()
                                .filter(|w| w.used_percent >= 100)
                                .filter_map(|w| w.resets_at)
                                .max()
                        })
                    } else {
                        None
                    };
                    *self.engine.reset_at.lock().unwrap() = decoded.reset_at.or(reset);
                }
                self.queue.extend(decoded.events);
                if let Some(end) = decoded.end {
                    // Keep usage/text already decoded before Messages and Ended.
                    self.finish(end);
                }
            }
        }
        Ok(())
    }
}
impl TurnStream for Handle<'_> {
    fn next(&mut self) -> EngineFuture<'_, Option<TurnEvent>> {
        Box::pin(async move {
            loop {
                if !self.done && self.cancel.is_cancelled() {
                    self.finish(TurnEnd::Stopped);
                }
                if let Some(event) = self.queue.pop_front() {
                    return Some(event);
                }
                if self.done {
                    return None;
                }
                if let Err(error) = self.advance().await {
                    self.finish(TurnEnd::Failed(error));
                }
            }
        })
    }
}
impl Drop for Handle<'_> {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.engine.running.lock().unwrap().remove(&self.id);
    }
}

/// Pictures travel as MCP image content, outside the text-result cap. History keeps a marker, not another
/// base64 copy; the registry/session recorder already supplies step thumbnails.
fn bounded_reply(mut reply: ToolReply, limit: usize) -> (ToolReply, Value, Option<Truncation>) {
    let image = reply.value.as_object_mut().and_then(|v| v.remove("image"));
    let text = reply.value.to_string();
    let total = text.chars().count();
    let truncated = if total > limit {
        let prefix: String = text.chars().take(limit).collect();
        reply.value = serde_json::json!({"truncated":true,"total_chars":total,"text":format!("{prefix} [truncated; request a narrower result]")});
        Some(Truncation {
            total_chars: total,
            shown_chars: limit,
        })
    } else {
        None
    };
    let mut log = reply.value.clone();
    if let Some(image) = image {
        reply.value["image"] = image;
        log["image"] =
            Value::String("[picture delivered through MCP; request screenshot again]".into());
    }
    (reply, log, truncated)
}
