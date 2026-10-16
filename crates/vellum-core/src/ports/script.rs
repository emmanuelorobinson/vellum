//! Sandboxed JavaScript for the `execute` tool and script nodes. Today: QuickJS via `rquickjs`.
//!
//! No network, file system, timers or async. `Math.random` is seeded per run, so the same source gives the
//! same output (Invariant 8). Everything a script changes goes through [`ScriptApi`], a thin binding over ops,
//! so a script can do nothing the op layer does not allow.

use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

/// A value that crosses between a script and the host: JSON's shapes.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ScriptValue {
    /// `null` or `undefined`.
    #[default]
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number.
    Number(f64),
    /// A string.
    Text(String),
    /// An array.
    List(Vec<ScriptValue>),
    /// An object, keys sorted.
    Map(BTreeMap<String, ScriptValue>),
}

/// What a script may call. Prompt 26 defines the functions; the port only carries them.
pub trait ScriptApi {
    /// Copied data properties, frozen with `doc`; never host handles.
    fn constants(&self) -> BTreeMap<String, ScriptValue> {
        BTreeMap::new()
    }
    /// The only functions exposed as `doc.<name>`. Names are unique identifiers.
    fn functions(&self) -> &[&str] {
        &[]
    }

    /// Exact number of newly created nodes, including descendants, before admitting a call.
    /// Read-only APIs use zero. Mutating APIs must count against their staged document, without
    /// changing it. The runtime reserves this cost before calling `call`.
    fn node_cost(&self, function: &str, args: &[ScriptValue]) -> Result<u32, ScriptError>;

    /// Calls the host function `function`.
    /// Implementations stage changes; only the caller commits after a successful run (Prompt 26).
    /// Calls must be synchronous and bounded: the JS interrupt cannot preempt Rust host code.
    fn call(&self, function: &str, args: &[ScriptValue]) -> Result<ScriptValue, ScriptError>;
}

/// What stops a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptLimits {
    /// Wall-clock time in milliseconds, measured by the runtime.
    pub time_ms: u32,
    /// Calls into the API.
    pub max_ops: u32,
    /// Nodes the script may create.
    pub max_nodes: u32,
    /// Memory in megabytes.
    pub memory_mb: u32,
}

/// What a finished script leaves behind.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScriptOutput {
    /// The explicit return value of the script body, or `Null`.
    pub value: ScriptValue,
    /// What the script logged, in order.
    pub logs: Vec<String>,
}

/// Why a script did not finish. Every escape attempt ends in one of these, never a panic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptError {
    /// The source does not parse.
    Syntax {
        /// What is wrong.
        message: String,
    },
    /// The script threw.
    Runtime {
        /// What it threw.
        message: String,
    },
    /// It ran past `time_ms`.
    Timeout,
    /// It made more than `max_ops` API calls.
    OpLimit,
    /// It created more than `max_nodes` nodes.
    NodeLimit,
    /// It used more than `memory_mb`.
    MemoryLimit,
    /// It reached for something the sandbox does not have (network, files, timers, host objects).
    Forbidden {
        /// What it reached for.
        what: String,
    },
    /// The host refused an API call.
    Api {
        /// Why.
        message: String,
    },
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { message } => write!(f, "The script has a syntax error: {message}"),
            Self::Runtime { message } => write!(f, "The script failed: {message}"),
            Self::Timeout => f.write_str("The script ran too long and was stopped."),
            Self::OpLimit => f.write_str("The script made too many changes and was stopped."),
            Self::NodeLimit => f.write_str("The script made too many layers and was stopped."),
            Self::MemoryLimit => f.write_str("The script used too much memory and was stopped."),
            Self::Forbidden { what } => write!(f, "Scripts cannot use {what}."),
            Self::Api { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ScriptError {}

/// Runs scripts in a sandbox.
pub trait ScriptRuntime {
    /// Runs with source diagnostics. Runtimes with engine locations override this method.
    fn run_diagnostic(
        &self,
        src: &str,
        api: Rc<dyn ScriptApi>,
        limits: ScriptLimits,
    ) -> Result<ScriptOutput, ScriptDiagnostic> {
        self.run(src, api, limits)
            .map_err(|error| ScriptDiagnostic::new(error, src, None))
    }
    /// Runs `src` with `api` within `limits`.
    fn run(
        &self,
        src: &str,
        api: Rc<dyn ScriptApi>,
        limits: ScriptLimits,
    ) -> Result<ScriptOutput, ScriptError>;
}

/// A failure with a one-based source position when the engine can provide one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptDiagnostic {
    pub error: ScriptError,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// At most three source lines, bounded to 512 characters each.
    pub excerpt: String,
}
impl ScriptDiagnostic {
    pub fn new(error: ScriptError, source: &str, position: Option<(u32, u32)>) -> Self {
        let (line, column) = position.map_or((None, None), |(l, c)| (Some(l), Some(c)));
        let index = line.unwrap_or(1).saturating_sub(1) as usize;
        let excerpt = source
            .lines()
            .enumerate()
            .skip(index.saturating_sub(1))
            .take(3)
            .map(|(i, s)| {
                let start = if i == index {
                    column.unwrap_or(1).saturating_sub(1).saturating_sub(256) as usize
                } else {
                    0
                };
                format!(
                    "{} | {}{}",
                    i + 1,
                    if start > 0 { "…" } else { "" },
                    s.chars().skip(start).take(512).collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            error,
            line,
            column,
            excerpt,
        }
    }
}
impl fmt::Display for ScriptDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for ScriptDiagnostic {}
