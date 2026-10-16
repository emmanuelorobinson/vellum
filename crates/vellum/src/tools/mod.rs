//! The tool registry: the product's real API (ARCHITECTURE §3.4). MCP, the CLI and the built-in agent are front
//! ends over it; none of them knows what a tool does.
//!
//! A tool is a name, a version, a JSON Schema for its input and a function. It reaches documents only through
//! [`ToolCtx`], which routes to the daemon's targets, so a tool cannot tell a live editor from a headless file.
//! Names are stable and schema changes are additive; a breaking change is a new tool name (§2.5).

mod batch;
mod bridge;
mod content;
mod context;
mod coordinate;
mod execute;
mod library;
mod model;
mod nodes;
mod scene;
mod spec;
mod step;
mod support;
mod tokens;
mod write;

use std::fmt;
use std::path::PathBuf;

use schemars::Schema;
use serde_json::Value;
use vellum_core::guard::WriteError;
use vellum_core::ports::{DocKey, FileError, FsError, ScriptValue, TargetError};
use vellum_core::spec::StackProfiles;
use vellum_types::{Actor, AgentId, Origin, SessionId, TurnId};

use crate::daemon::Documents;

pub use bridge::RegistryBridge;
pub use coordinate::SPAWN_AGENTS;
pub(crate) use coordinate::{SPAWN_DESCRIPTION, SpawnInput};
pub(crate) use scene::free_space;
pub use spec::{SpecError, formats as spec_formats, parse_spec};
use step::StepNote;
pub use step::{Counts, Outcome, STEP_LIMIT, Step, StepTarget, THUMBNAIL_SCALE, Thumbnail, Verb};
pub(crate) use support::schema_of;
pub use support::{Index, parse_input};

/// Why a tool did not do what was asked. The message is for the agent that called it: it says what was wrong
/// and, where it can, what to do next.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolError {
    kind: ErrorKind,
    message: String,
    path: Option<String>,
    op: Option<usize>,
    line: Option<(u32, Option<u32>)>,
}

/// What sort of failure it was, so a front end can tell a bad call from a missing thing from a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The input is wrong; fix it and call again. `path()` names the field.
    InvalidInput,
    /// A tool, document or node that is not there.
    NotFound,
    /// The file could not be read or written, or the editor holding it went away.
    Document,
    /// The Guard or the file's state refused a change.
    Refused,
    /// Nothing could be drawn.
    Render,
    /// A bug in Vellum.
    Internal,
}

impl ErrorKind {
    /// The word a front end reports it by.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::Document => "document",
            Self::Refused => "refused",
            Self::Render => "render",
            Self::Internal => "internal",
        }
    }
}

impl ToolError {
    /// A failure of `kind` with a plain `message`.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            path: None,
            op: None,
            line: None,
        }
    }

    /// The input is wrong at `path` (a field name, dotted for nested ones).
    pub fn invalid(path: impl Into<String>, message: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            kind: ErrorKind::InvalidInput,
            message: message.into(),
            path: (!path.is_empty()).then_some(path),
            op: None,
            line: None,
        }
    }

    /// The same failure, blamed on the `op`th item of a batch (counting from 0).
    #[must_use]
    pub fn at_op(mut self, op: usize) -> Self {
        self.op = Some(op);
        self
    }

    /// The same failure, at a line (and column) of a script or spec.
    #[must_use]
    pub fn at_line(mut self, line: u32, column: Option<u32>) -> Self {
        self.line = Some((line, column));
        self
    }

    /// The same failure, at another input field.
    #[must_use]
    pub fn at_path(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        self.path = (!path.is_empty()).then_some(path);
        self
    }

    /// Something that should be there is not.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    /// What sort of failure.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// What went wrong, in words.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The input field at fault, when one is.
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Which item of a batch was at fault, counting from 0. The batch was refused whole.
    pub fn op(&self) -> Option<usize> {
        self.op
    }

    /// The line of a script or spec at fault, counting from 1.
    pub fn line(&self) -> Option<u32> {
        self.line.map(|(line, _)| line)
    }

    /// Its column, counting from 1, when known.
    pub fn column(&self) -> Option<u32> {
        self.line.and_then(|(_, column)| column)
    }

    /// The error as an agent receives it: `{"error": {kind, message, path?, op?, line?, column?}}`.
    pub fn to_json(&self) -> Value {
        let mut detail = serde_json::json!({"kind": self.kind.as_str(), "message": self.message});
        if let Some(path) = self.path() {
            detail["path"] = path.into();
        }
        if let Some(op) = self.op() {
            detail["op"] = op.into();
        }
        if let Some(line) = self.line() {
            detail["line"] = line.into();
        }
        if let Some(column) = self.column() {
            detail["column"] = column.into();
        }
        serde_json::json!({ "error": detail })
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(f, "{path}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for ToolError {}

impl From<TargetError> for ToolError {
    fn from(error: TargetError) -> Self {
        let message = error.to_string();
        match error {
            TargetError::Write(WriteError::Invalid(_)) => {
                Self::new(ErrorKind::InvalidInput, message)
            }
            TargetError::Write(WriteError::Rejected(_))
            | TargetError::ReadOnly
            | TargetError::Lease(_) => Self::new(ErrorKind::Refused, message),
            TargetError::Gone => Self::new(ErrorKind::Document, message),
            TargetError::File(FileError::Fs(FsError::NotFound(_))) => Self::not_found(message),
            TargetError::File(_) => Self::new(ErrorKind::Document, message),
            TargetError::Render(_) => Self::new(ErrorKind::Render, message),
        }
    }
}

/// A target's refusal as an agent receives it: a Guard rejection names agents as the daemon knows them ("Frame
/// leased by Teal"), not by id.
pub(crate) fn refusal(
    agents: &std::collections::BTreeMap<AgentId, String>,
    error: TargetError,
) -> ToolError {
    match &error {
        TargetError::Write(WriteError::Rejected(rejection)) => ToolError::new(
            ErrorKind::Refused,
            rejection.message(&|agent: &AgentId| {
                agents
                    .get(agent)
                    .cloned()
                    .unwrap_or_else(|| agent.to_string())
            }),
        ),
        _ => error.into(),
    }
}

/// Who is calling: what the MCP client said about itself. Writes (Prompt 30) are attributed to it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Client {
    /// The client's name, such as `Claude Code`.
    pub name: String,
    /// Its version.
    pub version: String,
}

/// State that belongs to one connection, not to a document.
#[derive(Debug, Default)]
pub struct Session {
    /// The client on the other end, once it has said.
    pub client: Option<Client>,
    /// The directory the client was started in. When known, a relative document path means a path in it and a
    /// document outside it is refused. Opening one is not yet a `PermissionBroker` request: the daemon has nobody
    /// to ask until the editor's Permission card exists (Prompt 50), so the broker would deny it anyway.
    pub cwd: Option<PathBuf>,
    /// The agent this connection is: every change it makes is attributed to it. Given at its first call.
    agent: Option<AgentId>,
    /// The agent session its changes carry.
    id: Option<SessionId>,
    /// Its tool calls, oldest first, at most [`STEP_LIMIT`].
    steps: Vec<Step>,
    /// The built-in agent's turn its changes belong to, so "Undo this turn" can find them. External calls carry
    /// none.
    turn: Option<TurnId>,
    /// The session-store session its steps go to, per document (see `crate::sessions`).
    pub(crate) logged: Vec<(DocKey, SessionId)>,
    /// The same by daemon path, for an engine turn's own document: no repository lookup needed.
    pub(crate) logged_paths: Vec<(String, SessionId)>,
    /// The document its last step named, for steps that name none.
    pub(crate) last_document: Option<String>,
}

/// What an agent from a client that did not say its name is called.
const UNNAMED_CLIENT: &str = "MCP client";

impl Session {
    /// A session whose client was started in `cwd`.
    pub fn in_dir(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: Some(cwd.into()),
            ..Self::default()
        }
    }

    /// The connection an engine's turns run through: its changes are attributed to `agent`, named `name` (a
    /// built-in hue such as "Teal"), in the session-store session `id` its host made.
    pub fn for_agent(name: impl Into<String>, agent: AgentId, id: SessionId) -> Self {
        Self {
            client: Some(Client {
                name: name.into(),
                version: String::new(),
            }),
            agent: Some(agent),
            id: Some(id),
            ..Self::default()
        }
    }

    /// The agent this connection is, once it has called a tool.
    pub fn agent(&self) -> Option<&AgentId> {
        self.agent.as_ref()
    }

    /// The agent session its changes carry, once it has called a tool.
    pub fn id(&self) -> Option<&SessionId> {
        self.id.as_ref()
    }

    /// The turn this connection's changes are part of, from now on (`None` ends it).
    pub fn set_turn(&mut self, turn: Option<TurnId>) {
        self.turn = turn;
    }

    /// The turn its changes are part of, when one is running.
    pub fn turn(&self) -> Option<&TurnId> {
        self.turn.as_ref()
    }

    /// Its recent steps, oldest first.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Its steps, which are removed: what the session store calls to record them.
    pub fn take_steps(&mut self) -> Vec<Step> {
        std::mem::take(&mut self.steps)
    }

    /// Records this connection's steps on `doc` into the session-store session `id` (an engine's session, made by
    /// its host). Without this, steps go to an external session made on first use.
    pub fn log_as(&mut self, doc: DocKey, id: SessionId) {
        self.logged.retain(|(known, _)| known != &doc);
        self.logged.push((doc, id));
    }

    /// Records this connection's steps on the document at daemon path `path` into the session-store session
    /// `id`: what an orchestrator does for an engine turn's document, whatever repository it is in.
    pub fn log_path_as(&mut self, path: impl Into<String>, id: SessionId) {
        let path = path.into();
        self.logged_paths.retain(|(known, _)| known != &path);
        self.logged_paths.push((path, id));
    }

    fn record(&mut self, step: Step) {
        if self.steps.len() == STEP_LIMIT {
            self.steps.remove(0);
        }
        self.steps.push(step);
    }

    /// The name this connection's agent goes by: its client's (guide: agents are named by client).
    pub fn agent_name(&self) -> String {
        self.client
            .as_ref()
            .map(|c| c.name.trim())
            .filter(|name| !name.is_empty())
            .unwrap_or(UNNAMED_CLIENT)
            .to_owned()
    }
}

/// What a tool run can reach.
pub struct ToolCtx<'a> {
    docs: &'a mut Documents,
    session: &'a mut Session,
    note: StepNote,
}

impl<'a> ToolCtx<'a> {
    /// A context over the daemon's documents for one call.
    pub fn new(docs: &'a mut Documents, session: &'a mut Session) -> Self {
        Self {
            docs,
            session,
            note: StepNote::default(),
        }
    }

    /// Gives this connection its agent and session ids, the first time, and keeps its name current.
    fn introduce(&mut self) {
        if self.session.agent.is_none() {
            let agent = AgentId::new(self.docs.ulid()).expect("a ULID");
            self.session.agent = Some(agent);
            self.session.id = Some(SessionId::new(self.docs.ulid()).expect("a ULID"));
        }
        let agent = self.session.agent.clone().expect("just given");
        let name = self.session.agent_name();
        if self.docs.agents().get(&agent) != Some(&name) {
            self.docs.name_agent(agent, name);
        }
    }

    /// Who this call's changes are by: this connection's agent, in its session.
    pub fn origin(&mut self) -> Origin {
        self.introduce();
        let session = self.session.id.clone().expect("introduced");
        let agent = self.session.agent.clone().expect("introduced");
        let origin = Origin::new(Actor::Agent { agent }).in_session(session);
        match self.session.turn.clone() {
            Some(turn) => origin.in_turn(turn),
            None => origin,
        }
    }

    /// The daemon's documents.
    pub fn docs(&mut self) -> &mut Documents {
        self.docs
    }

    /// This connection.
    pub fn session(&self) -> &Session {
        self.session
    }
}

/// A step's thumbnail as PNG bytes: the frame drawn as `screenshot` draws it. `None` when it cannot be drawn
/// (a build without the scene, a node that is gone).
pub fn thumbnail_png(
    docs: &mut Documents,
    session: &mut Session,
    document: &str,
    thumbnail: &Thumbnail,
) -> Option<Vec<u8>> {
    let mut ctx = ToolCtx::new(docs, session);
    let image = ctx
        .target(document)
        .ok()?
        .render(&thumbnail.node, thumbnail.scale)
        .ok()?;
    match image {
        ScriptValue::Map(fields) => match fields.get("base64") {
            Some(ScriptValue::Text(data)) => vellum_core::base64::decode(data),
            _ => None,
        },
        _ => None,
    }
}

/// One operation an agent can ask for.
pub trait Tool {
    /// The name agents call it by: `snake_case`, stable.
    fn name(&self) -> &'static str;
    /// Bumped when the output grows a field or the behaviour changes without a schema change.
    fn version(&self) -> u32;
    /// What it does and when to use it, in two or three plain sentences. Shown to the agent.
    fn description(&self) -> &'static str;
    /// The JSON Schema of its input: an object that refuses fields it does not know.
    fn input_schema(&self) -> Schema;
    /// Whether it leaves the document as it found it.
    fn read_only(&self) -> bool;
    /// What its step shows in the Agent panel.
    fn verb(&self) -> Verb {
        Verb::Read
    }
    /// Whether it can remove or overwrite something the person made. A tool that only adds says no, so a client
    /// need not ask before every call. Read-only tools never do.
    fn destructive(&self) -> bool {
        !self.read_only()
    }
    /// Runs it. A mutating tool applies one transaction, or none.
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError>;
}

/// Agent settings the registry depends on (ARCHITECTURE §11 `[agents]`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentSettings {
    /// External clients may start helper agents with `spawn_agents`. Off by default; engines Vellum runs have the
    /// tool either way.
    pub spawn_agents_external: bool,
}

/// Every tool, by name.
pub struct Registry {
    tools: Vec<Box<dyn Tool>>,
}

impl Registry {
    /// A registry holding `tools`.
    pub fn new(mut tools: Vec<Box<dyn Tool>>) -> Self {
        tools.sort_by_key(|tool| tool.name());
        Self { tools }
    }

    /// The tools Vellum ships, sorted by name, with the default settings.
    pub fn standard() -> Self {
        Self::standard_with(&AgentSettings::default())
    }

    /// The tools Vellum ships, with `settings` deciding the optional ones.
    pub fn standard_with(settings: &AgentSettings) -> Self {
        Self::configured(
            std::rc::Rc::new(crate::content::BuiltinSkills),
            std::rc::Rc::new(crate::content::BuiltinStyles),
            settings,
            StackProfiles::builtin(),
        )
    }

    /// The tools Vellum ships, with `get_spec` knowing these stacks instead of the built-in three.
    pub fn with_stacks(stacks: StackProfiles) -> Self {
        Self::configured(
            std::rc::Rc::new(crate::content::BuiltinSkills),
            std::rc::Rc::new(crate::content::BuiltinStyles),
            &AgentSettings::default(),
            stacks,
        )
    }

    /// Hosts supply explicitly authorized guideline/skill folders here; MCP and CLI keep the same registry.
    pub fn with_sources(
        skills: std::rc::Rc<dyn vellum_core::ports::SkillSource>,
        styles: std::rc::Rc<dyn vellum_core::ports::StyleLibrary>,
    ) -> Self {
        Self::configured(
            skills,
            styles,
            &AgentSettings::default(),
            StackProfiles::builtin(),
        )
    }

    fn configured(
        skills: std::rc::Rc<dyn vellum_core::ports::SkillSource>,
        styles: std::rc::Rc<dyn vellum_core::ports::StyleLibrary>,
        settings: &AgentSettings,
        stacks: StackProfiles,
    ) -> Self {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();
        content::register(&mut tools, skills, styles);
        context::register(&mut tools);
        nodes::register(&mut tools);
        scene::register(&mut tools);
        library::register(&mut tools);
        write::register(&mut tools);
        model::register(&mut tools);
        coordinate::register(&mut tools, settings);
        execute::register(&mut tools);
        tokens::register(&mut tools, stacks);
        Self::new(tools)
    }

    /// The tools, sorted by name.
    pub fn tools(&self) -> impl Iterator<Item = &dyn Tool> {
        self.tools.iter().map(|tool| &**tool)
    }

    /// The tool called `name`.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools().find(|tool| tool.name() == name)
    }

    /// Runs the tool called `name` and records its step in the session.
    pub fn call(
        &self,
        ctx: &mut ToolCtx<'_>,
        name: &str,
        input: Value,
    ) -> Result<Value, ToolError> {
        match self.get(name) {
            Some(tool) => {
                let started = ctx.docs.now().0;
                ctx.introduce();
                ctx.note = StepNote::default();
                let arguments = step::preview(&input);
                let result = tool.run(ctx, input);
                let note = std::mem::take(&mut ctx.note);
                let mut step =
                    Step::new(name, tool.verb(), note, &result, started, ctx.docs.now().0);
                step.arguments = arguments;
                ctx.session.record(step);
                result
            }
            None => Err(ToolError::not_found(format!(
                "There is no tool called {name}. The tools are: {}.",
                self.tools()
                    .map(|t| t.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
}
