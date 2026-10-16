//! The tool registry: the product's real API (ARCHITECTURE §3.4). MCP, the CLI and the built-in agent are front
//! ends over it; none of them knows what a tool does.
//!
//! A tool is a name, a version, a JSON Schema for its input and a function. It reaches documents only through
//! [`ToolCtx`], which routes to the daemon's targets, so a tool cannot tell a live editor from a headless file.
//! Names are stable and schema changes are additive; a breaking change is a new tool name (§2.5).

mod context;
mod library;
mod nodes;
mod scene;
mod support;

use std::fmt;
use std::path::PathBuf;

use schemars::Schema;
use serde_json::Value;
use vellum_core::ports::{FileError, FsError, TargetError};

use crate::daemon::Documents;

pub use support::{Index, parse_input};

/// Why a tool did not do what was asked. The message is for the agent that called it: it says what was wrong
/// and, where it can, what to do next.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolError {
    kind: ErrorKind,
    message: String,
    path: Option<String>,
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
        }
    }

    /// The input is wrong at `path` (a field name, dotted for nested ones).
    pub fn invalid(path: impl Into<String>, message: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            kind: ErrorKind::InvalidInput,
            message: message.into(),
            path: (!path.is_empty()).then_some(path),
        }
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
            TargetError::Write(_) | TargetError::ReadOnly => Self::new(ErrorKind::Refused, message),
            TargetError::Gone => Self::new(ErrorKind::Document, message),
            TargetError::File(FileError::Fs(FsError::NotFound(_))) => Self::not_found(message),
            TargetError::File(_) => Self::new(ErrorKind::Document, message),
            TargetError::Render(_) => Self::new(ErrorKind::Render, message),
        }
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
    /// document outside it is refused (until the PermissionBroker decides outside reads, Prompt 35).
    pub cwd: Option<PathBuf>,
}

impl Session {
    /// A session whose client was started in `cwd`.
    pub fn in_dir(cwd: impl Into<PathBuf>) -> Self {
        Self {
            client: None,
            cwd: Some(cwd.into()),
        }
    }
}

/// What a tool run can reach.
pub struct ToolCtx<'a> {
    docs: &'a mut Documents,
    session: &'a mut Session,
}

impl<'a> ToolCtx<'a> {
    /// A context over the daemon's documents for one call.
    pub fn new(docs: &'a mut Documents, session: &'a mut Session) -> Self {
        Self { docs, session }
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
    /// Whether it can remove or overwrite something the person made. A tool that only adds says no, so a client
    /// need not ask before every call. Read-only tools never do.
    fn destructive(&self) -> bool {
        !self.read_only()
    }
    /// Runs it. A mutating tool applies one transaction, or none.
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError>;
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

    /// The tools Vellum ships, sorted by name.
    pub fn standard() -> Self {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();
        context::register(&mut tools);
        nodes::register(&mut tools);
        scene::register(&mut tools);
        library::register(&mut tools);
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

    /// Runs the tool called `name`.
    pub fn call(
        &self,
        ctx: &mut ToolCtx<'_>,
        name: &str,
        input: Value,
    ) -> Result<Value, ToolError> {
        match self.get(name) {
            Some(tool) => tool.run(ctx, input),
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
