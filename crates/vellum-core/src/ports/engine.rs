//! Who runs an agent turn (ARCHITECTURE §3.4). The editor, the CLI and the daemon only talk to [`AgentEngine`];
//! an engine reaches the document only through the [`ToolBridge`] it is handed, which is the same tool registry
//! MCP clients use. Nothing here touches a clock, the network or a file.
use super::content::SkillMeta;
use super::model::{ChatMessage, ModelError, ModelTool, ModelUsage, ReasoningEffort};
use serde_json::Value;
use std::{fmt, future::Future, pin::Pin};
use vellum_types::TurnId;

/// Engine futures borrow the bridge, which belongs to one thread, so they are not `Send`.
pub type EngineFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// A tool the engine may offer its model.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeTool {
    pub tool: ModelTool,
    /// Leaves the document as it found it.
    pub read_only: bool,
}

/// What a tool call came back with. A failed call carries the error as JSON (kind, message, path, op) so the
/// model can fix its input and call again.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolReply {
    pub value: Value,
    pub failed: bool,
}

/// The engine's only way to the canvas. Each call is one tool run, attributed to the turn by the host that
/// built the bridge, and passes the Guard like any other agent write.
pub trait ToolBridge {
    fn tools(&self) -> Vec<BridgeTool>;
    fn call(&self, name: &str, input: Value) -> ToolReply;
}

/// What the Agent panel and `vellum status` say about an engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineStatus {
    Ready,
    /// Installed engines only: the program is not there.
    Missing,
    /// The engine or its model provider needs a key or a sign-in.
    SignedOut,
    UsageLimit,
    Failed,
}

/// Bounds on one turn, so a confused model cannot run forever or hammer the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnLimits {
    /// Model requests in this turn, summaries included.
    pub max_rounds: u32,
    /// Tool calls that may fail before the turn gives up.
    pub max_failed_calls: u32,
    pub max_output_tokens: u32,
    /// A tool result longer than this (characters of JSON) is cut, with a marker saying so.
    pub max_tool_result_chars: usize,
}
impl Default for TurnLimits {
    fn default() -> Self {
        Self {
            max_rounds: 40,
            max_failed_calls: 8,
            max_output_tokens: 8192,
            max_tool_result_chars: 24_000,
        }
    }
}

/// One request from a person (or an orchestrator) to an engine.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnRequest {
    pub turn: TurnId,
    pub model: String,
    /// What the person asked.
    pub prompt: String,
    /// Attached pictures, as (mime, base64).
    pub images: Vec<(String, String)>,
    /// Earlier turns of this session, oldest first.
    pub history: Vec<ChatMessage>,
    /// The skills the agent can read with `read_skill`, in stacking order.
    pub skills: Vec<SkillMeta>,
    pub effort: Option<ReasoningEffort>,
    pub limits: TurnLimits,
}
impl TurnRequest {
    pub fn new(turn: TurnId, model: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            turn,
            model: model.into(),
            prompt: prompt.into(),
            images: vec![],
            history: vec![],
            skills: vec![],
            effort: None,
            limits: TurnLimits::default(),
        }
    }
}

/// A tool result that did not reach the model whole. It was cut, never silently dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Truncation {
    pub total_chars: usize,
    pub shown_chars: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineError {
    /// The engine cannot run turns now; see `availability`.
    Unavailable,
    /// A turn with this id is already running.
    Busy,
    InvalidRequest,
    Model(ModelError),
    /// The model declined.
    Refused,
    TooManyFailedCalls,
    RoundLimit,
    /// The conversation no longer fits the model's context, even after compaction.
    ContextFull,
}
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "agent engine: {self:?}")
    }
}
impl std::error::Error for EngineError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnEnd {
    Done,
    /// A person stopped it (Invariant 12). Everything built so far stays.
    Stopped,
    Failed(EngineError),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TurnEvent {
    Text(String),
    Reasoning(String),
    ToolStarted {
        call: String,
        name: String,
        arguments: Value,
    },
    ToolFinished {
        call: String,
        name: String,
        failed: bool,
        truncated: Option<Truncation>,
    },
    /// The engine asked the model to look at what it built before finishing.
    SelfCheck,
    /// Earlier context was cut to fit: `pruned` tool results or pictures replaced by markers, `summarized`
    /// messages replaced by one summary.
    Compacted {
        pruned: usize,
        summarized: usize,
    },
    Usage(ModelUsage),
    /// The messages this turn added to the session, for the next turn's history. Sent once, before `Ended`.
    Messages(Vec<ChatMessage>),
    /// Always the last event.
    Ended(TurnEnd),
}

pub trait TurnStream {
    /// `None` only after `Ended`.
    fn next(&mut self) -> EngineFuture<'_, Option<TurnEvent>>;
}

pub trait AgentEngine: Send + Sync {
    /// "builtin", "claude-code", "codex".
    fn id(&self) -> &str;
    fn availability(&self) -> EngineStatus;
    fn run_turn<'a>(
        &'a self,
        turn: TurnRequest,
        tools: &'a dyn ToolBridge,
    ) -> Result<Box<dyn TurnStream + 'a>, EngineError>;
    /// Stops a running turn from any thread. Unknown or finished turns are ignored.
    fn interrupt(&self, turn: &TurnId);
}

/// Turn-scoped MCP transport supplied by the host to installed engines.
/// All canvas work returns to the borrowed ToolBridge; transport threads never own a document.
pub struct McpCall {
    pub name: String,
    pub input: Value,
    pub respond: Box<dyn FnOnce(ToolReply) + Send>,
}
pub trait McpChannel {
    fn config(&self) -> Value;
    fn poll_call(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<McpCall>>;
}
pub trait McpHost: Send + Sync {
    fn open(&self, tools: &[BridgeTool]) -> Result<Box<dyn McpChannel>, EngineError>;
}
