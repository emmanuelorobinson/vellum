//! A scripted AgentEngine for UI and orchestration work, and a ToolBridge that records what it was asked.
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    future::poll_fn,
    sync::Mutex,
};
use vellum_core::ports::*;
use vellum_types::TurnId;

type Handler = Box<dyn Fn(&Value) -> ToolReply>;

/// A ToolBridge over canned tools. Calls are recorded in order; unknown tools fail like the real registry.
pub struct RecordingBridge {
    tools: Vec<(BridgeTool, Handler)>,
    calls: RefCell<Vec<(String, Value)>>,
}
impl RecordingBridge {
    pub fn new() -> Self {
        Self {
            tools: vec![],
            calls: RefCell::default(),
        }
    }
    /// Adds a tool answered by `handler`.
    #[must_use]
    pub fn with(
        mut self,
        name: &str,
        read_only: bool,
        handler: impl Fn(&Value) -> ToolReply + 'static,
    ) -> Self {
        let tool = BridgeTool {
            tool: ModelTool {
                name: name.into(),
                description: format!("TEST DATA: the {name} tool."),
                parameters: json!({"type": "object"}),
            },
            read_only,
        };
        self.tools.push((tool, Box::new(handler)));
        self
    }
    /// The contract suite's bridge: one read-only `probe` tool that answers `{"ok":true}`.
    pub fn probe() -> Self {
        Self::new().with("probe", true, |_| ToolReply {
            value: json!({"ok": true}),
            failed: false,
        })
    }
    pub fn calls(&self) -> Vec<(String, Value)> {
        self.calls.borrow().clone()
    }
}
impl Default for RecordingBridge {
    fn default() -> Self {
        Self::new()
    }
}
impl ToolBridge for RecordingBridge {
    fn tools(&self) -> Vec<BridgeTool> {
        self.tools.iter().map(|(tool, _)| tool.clone()).collect()
    }
    fn call(&self, name: &str, input: Value) -> ToolReply {
        self.calls.borrow_mut().push((name.into(), input.clone()));
        match self.tools.iter().find(|(tool, _)| tool.tool.name == name) {
            Some((_, handler)) => handler(&input),
            None => ToolReply {
                value: json!({"error": {"kind": "not_found", "message": format!("There is no tool called {name}.")}}),
                failed: true,
            },
        }
    }
}

/// One thing a scripted turn does.
#[derive(Clone, Debug, PartialEq)]
pub enum FakeStep {
    /// Sends the event as it is.
    Emit(TurnEvent),
    /// Calls a tool through the bridge and reports it.
    Call { name: String, input: Value },
    /// Waits for Stop.
    Work,
}

/// An engine that replays one script per turn, oldest first. A script that runs out ends the turn Done.
pub struct FakeEngine {
    scripts: Mutex<VecDeque<Vec<FakeStep>>>,
    running: Mutex<BTreeMap<TurnId, Cancellation>>,
    requests: Mutex<Vec<TurnRequest>>,
    status: EngineStatus,
}
impl FakeEngine {
    pub fn new(scripts: Vec<Vec<FakeStep>>) -> Self {
        Self {
            scripts: Mutex::new(scripts.into()),
            running: Mutex::default(),
            requests: Mutex::default(),
            status: EngineStatus::Ready,
        }
    }
    #[must_use]
    pub fn with_status(mut self, status: EngineStatus) -> Self {
        self.status = status;
        self
    }
    /// Adds a script for a later turn, for scripts that need what an earlier turn produced (a placeholder's id).
    pub fn queue(&self, script: Vec<FakeStep>) {
        self.scripts.lock().unwrap().push_back(script);
    }
    pub fn requests(&self) -> Vec<TurnRequest> {
        self.requests.lock().unwrap().clone()
    }
}

struct FakeTurn<'a> {
    engine: &'a FakeEngine,
    turn: TurnId,
    bridge: &'a dyn ToolBridge,
    steps: VecDeque<FakeStep>,
    cancel: Cancellation,
    queue: VecDeque<TurnEvent>,
    messages: Vec<ChatMessage>,
    ended: bool,
}
impl FakeTurn<'_> {
    fn end(&mut self, end: TurnEnd) {
        self.queue
            .push_back(TurnEvent::Messages(std::mem::take(&mut self.messages)));
        self.queue.push_back(TurnEvent::Ended(end));
        self.ended = true;
        self.engine.running.lock().unwrap().remove(&self.turn);
    }
}
impl TurnStream for FakeTurn<'_> {
    fn next(&mut self) -> EngineFuture<'_, Option<TurnEvent>> {
        Box::pin(async move {
            loop {
                if let Some(event) = self.queue.pop_front() {
                    return Some(event);
                }
                if self.ended {
                    return None;
                }
                if self.cancel.is_cancelled() {
                    self.end(TurnEnd::Stopped);
                    continue;
                }
                match self.steps.pop_front() {
                    None => self.end(TurnEnd::Done),
                    Some(FakeStep::Emit(TurnEvent::Ended(end))) => self.end(end),
                    Some(FakeStep::Emit(event)) => self.queue.push_back(event),
                    Some(FakeStep::Call { name, input }) => {
                        let call = format!("fake{}", self.messages.len());
                        self.queue.push_back(TurnEvent::ToolStarted {
                            call: call.clone(),
                            name: name.clone(),
                            arguments: input.clone(),
                        });
                        let reply = self.bridge.call(&name, input.clone());
                        self.messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: vec![ChatPart::ToolCall(ModelCall {
                                id: call.clone(),
                                name: name.clone(),
                                arguments: input,
                                metadata: Value::Null,
                            })],
                        });
                        self.messages.push(ChatMessage {
                            role: ChatRole::User,
                            content: vec![ChatPart::ToolResult {
                                id: call.clone(),
                                name: name.clone(),
                                result: reply.value,
                                failed: reply.failed,
                            }],
                        });
                        self.queue.push_back(TurnEvent::ToolFinished {
                            call,
                            name,
                            failed: reply.failed,
                            truncated: None,
                        });
                    }
                    Some(FakeStep::Work) => {
                        let cancel = self.cancel.clone();
                        poll_fn(|cx| cancel.poll_cancelled(cx)).await;
                        self.end(TurnEnd::Stopped);
                    }
                }
            }
        })
    }
}
impl AgentEngine for FakeEngine {
    fn id(&self) -> &str {
        "fake"
    }
    fn availability(&self) -> EngineStatus {
        self.status
    }
    fn run_turn<'a>(
        &'a self,
        turn: TurnRequest,
        tools: &'a dyn ToolBridge,
    ) -> Result<Box<dyn TurnStream + 'a>, EngineError> {
        if self.status != EngineStatus::Ready {
            return Err(EngineError::Unavailable);
        }
        let cancel = Cancellation::default();
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(&turn.turn) {
                return Err(EngineError::Busy);
            }
            running.insert(turn.turn.clone(), cancel.clone());
        }
        let steps = self.scripts.lock().unwrap().pop_front().unwrap_or_default();
        let id = turn.turn.clone();
        self.requests.lock().unwrap().push(turn);
        Ok(Box::new(FakeTurn {
            engine: self,
            turn: id,
            bridge: tools,
            steps: steps.into(),
            cancel,
            queue: VecDeque::new(),
            messages: vec![],
            ended: false,
        }))
    }
    fn interrupt(&self, turn: &TurnId) {
        if let Some(cancel) = self.running.lock().unwrap().get(turn) {
            cancel.cancel();
        }
    }
}

/// A model that never answers: it waits for cancellation, or, when `deaf`, ignores it too. For proving an engine
/// stops on time whatever its provider does.
pub struct StalledModel {
    pub deaf: bool,
}
struct StalledStream {
    cancel: Cancellation,
    deaf: bool,
    done: bool,
}
impl ModelStream for StalledStream {
    fn next(&mut self) -> ModelFuture<'_, Option<Result<ModelEvent, ModelError>>> {
        Box::pin(async move {
            if self.done {
                return None;
            }
            if self.deaf {
                std::future::pending::<()>().await;
            }
            let cancel = self.cancel.clone();
            poll_fn(|cx| cancel.poll_cancelled(cx)).await;
            self.done = true;
            Some(Err(ModelError::Cancelled))
        })
    }
}
impl ModelProvider for StalledModel {
    fn id(&self) -> &str {
        "stalled"
    }
    fn capabilities(&self) -> ModelCaps {
        ModelCaps {
            tools: true,
            vision: true,
            reasoning: true,
            context_tokens: None,
        }
    }
    fn status(&self) -> ProviderStatus {
        ProviderStatus::ConfiguredExternally
    }
    fn complete(
        &self,
        _req: ChatRequest,
        cancel: Cancellation,
    ) -> ModelFuture<'_, Result<ChatStream, ModelError>> {
        let deaf = self.deaf;
        Box::pin(async move {
            Ok(Box::new(StalledStream {
                cancel,
                deaf,
                done: false,
            }) as ChatStream)
        })
    }
}
