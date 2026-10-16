//! Asking before an agent does anything outside the canvas (ARCHITECTURE §7.7, guide Invariant 11).
//!
//! Canvas edits never come here: they are attributed, visible and undoable. Reading or writing a file, running a
//! command, and the built-in agent's network access beyond its model call do. [`PermissionBroker`] decides (a
//! policy rule, an earlier "Allow for this session", an installed engine's auto mode the person turned on, "Skip
//! all permission prompts", or the person through a [`PermissionPrompt`]), records the decision in the session
//! store, and hands back a [`crate::permissions::Permit`] only when the action is allowed and recorded. Whatever
//! performs the action takes the permit, so no code path can act without an `Allowed` record.
use super::engine::EngineFuture;
use super::sessions::EventSeq;
use std::fmt;
use vellum_types::{AgentId, Origin};

/// Something outside the canvas an agent wants to do. Paths are absolute, as the host resolved them (symbolic
/// links followed), so "inside the repository" means where the bytes really are.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    ReadFile {
        path: String,
    },
    WriteFile {
        path: String,
    },
    /// A program and its arguments, run in `cwd`. Nothing goes through a shell.
    RunCommand {
        program: String,
        args: Vec<String>,
        cwd: String,
    },
    /// The built-in agent reaching the network beyond its model call (installed engines do their own).
    Network {
        url: String,
    },
}

impl Action {
    /// The command as one line, for prompts, rules and logs: the program and its arguments, separated by spaces.
    pub fn command_line(program: &str, args: &[String]) -> String {
        std::iter::once(program)
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The kinds the defaults and settings talk about (§7.7's table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PermissionKind {
    /// Read a file in the session's repository. Ask once per session, or allowed by a rule.
    ReadRepo,
    /// Read a file outside it. Ask every time.
    ReadOutside,
    /// Write a file in the repository. Ask.
    WriteRepo,
    /// Write a file outside it. Ask every time.
    WriteOutside,
    /// Run a command. Ask.
    RunCommand,
    /// Network access by the built-in agent. Ask every time.
    Network,
}

impl PermissionKind {
    pub const ALL: [PermissionKind; 6] = [
        Self::ReadRepo,
        Self::ReadOutside,
        Self::WriteRepo,
        Self::WriteOutside,
        Self::RunCommand,
        Self::Network,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadRepo => "read_repo",
            Self::ReadOutside => "read_outside",
            Self::WriteRepo => "write_repo",
            Self::WriteOutside => "write_outside",
            Self::RunCommand => "run_command",
            Self::Network => "network",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }

    /// Whether "Allow for this session" is offered. Reads and writes outside the repository and network access
    /// are asked every time.
    pub fn session_grant_offered(self) -> bool {
        matches!(self, Self::ReadRepo | Self::WriteRepo | Self::RunCommand)
    }
}

/// One request. The origin names the agent, its session and (for engine turns) the turn; the store refuses a
/// record whose origin names another session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionRequest {
    pub origin: Origin,
    pub action: Action,
    /// The engine whose turn asks ("builtin", "claude-code"), or `None` for an external MCP client.
    pub engine: Option<String>,
    /// What an installed engine's own permission system decided, when it has one. Used only when the person
    /// turned that engine's auto mode on.
    pub engine_verdict: Option<bool>,
}

/// What the person can answer on the Permission card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Answer {
    AllowOnce,
    AllowForSession,
    Deny,
}

impl Answer {
    pub fn allows(self) -> bool {
        !matches!(self, Self::Deny)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AllowOnce => "allow_once",
            Self::AllowForSession => "allow_for_session",
            Self::Deny => "deny",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        [Self::AllowOnce, Self::AllowForSession, Self::Deny]
            .into_iter()
            .find(|answer| answer.as_str() == name)
    }
}

/// Who or what decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionSource {
    /// The person answered the prompt.
    Person,
    /// The person's "Allow for this session", recorded at `grant`.
    SessionGrant { grant: EventSeq },
    /// A rule from settings or the command line, quoted as written (`read_repo = "allow"`, `deny run rm *`).
    Rule { rule: String },
    /// An installed engine's own permission system, with its auto mode turned on by the person.
    EngineAuto { engine: String },
    /// "Skip all permission prompts" is on.
    SkipAll,
    /// Nobody could be asked (a headless run with no rule for it), so it was denied.
    Unattended,
    /// The person stopped the turn while it waited, so it was denied.
    TurnStopped,
    /// The request could not be recorded or checked (no session, a store failure), so it was denied.
    Failed { reason: String },
}

impl DecisionSource {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::SessionGrant { .. } => "session_grant",
            Self::Rule { .. } => "rule",
            Self::EngineAuto { .. } => "engine_auto",
            Self::SkipAll => "skip_all",
            Self::Unattended => "unattended",
            Self::TurnStopped => "turn_stopped",
            Self::Failed { .. } => "failed",
        }
    }
}

/// The decision record the session store keeps, one per request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionRecord {
    pub action: Action,
    pub kind: PermissionKind,
    pub engine: Option<String>,
    pub answer: Answer,
    pub source: DecisionSource,
}

impl PermissionRecord {
    pub fn allowed(&self) -> bool {
        self.answer.allows()
    }
}

/// What the Permission card (or the terminal) shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub agent: Option<AgentId>,
    pub engine: Option<String>,
    pub action: Action,
    pub kind: PermissionKind,
    /// The answers offered, in display order.
    pub offered: Vec<Answer>,
}

/// Why an action was not allowed. Carries the record that says so, so the agent can be told why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// Boxed: a refusal travels in `Err` beside a small permit.
    pub record: Box<PermissionRecord>,
    /// Where the refusal was recorded; `None` only when recording itself failed.
    pub seq: Option<EventSeq>,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match &self.record.action {
            Action::ReadFile { path } => format!("read {path}"),
            Action::WriteFile { path } => format!("write {path}"),
            Action::RunCommand { program, args, .. } => {
                format!("run {}", Action::command_line(program, args))
            }
            Action::Network { url } => format!("reach {url}"),
        };
        let why = match &self.record.source {
            DecisionSource::Person => "the person denied it".to_owned(),
            DecisionSource::SessionGrant { .. } => "denied".to_owned(),
            DecisionSource::Rule { rule } => format!("a rule denies it ({rule})"),
            DecisionSource::EngineAuto { engine } => format!("{engine}'s auto mode denied it"),
            DecisionSource::SkipAll => "denied".to_owned(),
            DecisionSource::Unattended => "nobody is there to allow it".to_owned(),
            DecisionSource::TurnStopped => "the turn was stopped".to_owned(),
            DecisionSource::Failed { reason } => reason.clone(),
        };
        write!(f, "Not allowed to {what}: {why}.")
    }
}

impl std::error::Error for Refusal {}

/// Decides requests. The editor, the CLI and the daemon share one implementation
/// ([`crate::permissions::Broker`]); they differ in their [`PermissionPrompt`].
pub trait PermissionBroker {
    fn request<'a>(
        &'a self,
        request: PermissionRequest,
    ) -> EngineFuture<'a, Result<crate::permissions::Permit, Refusal>>;
}

/// Asks the person: the editor's Permission card, or the terminal. `None` means the question went away without an
/// answer (dismissed, the window closed), which is a denial.
pub trait PermissionPrompt {
    fn ask<'a>(&'a self, question: Question) -> EngineFuture<'a, Option<Answer>>;
}
