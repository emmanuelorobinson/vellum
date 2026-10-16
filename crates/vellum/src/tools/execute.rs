//! `execute`: one script, many operations, one transaction (ARCHITECTURE §7.3 Layer 1).
//!
//! The script runs in the `ScriptRuntime` sandbox against the core's `doc` API, on a scratch copy of the document
//! as its holder has it. What it leaves is diffed against what it started from and sent to the document's target
//! as one transaction by this connection's agent, so a live editor and a headless file are written the same way and
//! the target's Guard has the last word. A script that fails, or a transaction the target refuses, applies nothing.
//! Frames the script claimed or released are claimed and released at the target too.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::diff::diff;
use vellum_core::doc_api::{self, EXECUTE_LIMITS, ExecuteContext, ExecuteRequest};
use vellum_core::guard::{GuardState, LeaseChange, WriteError};
use vellum_core::ops::Transaction;
use vellum_core::ports::{IdGen, ScriptDiagnostic, ScriptError, TargetError};
use vellum_types::{Actor, AgentId, NodeId};

use super::batch::{ids_in_use, sentence};
use super::step::Counts;
use super::support::{parse_input, schema_of};
use super::{ErrorKind, Tool, ToolCtx, ToolError, Verb};
use crate::daemon::ToolIds;

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(Execute));
}

/// The longest script taken.
const MAX_SOURCE: usize = 100_000;

struct Execute;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExecuteInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// A JavaScript program using `doc` (read_skill "execute" documents it): doc.page, get, query, insert,
    /// update, copy, replace, move, delete, setVariables, createComponent, makeSlot, instance, findSpace,
    /// layoutProblems, screenshot, export, claim, release. `return` a value to get it back; return
    /// doc.screenshot(id) to see the result.
    code: String,
    /// Run it and report what would change, without changing anything.
    #[serde(default)]
    validate_only: bool,
}

/// New ids from the daemon's generator that the document does not already use.
struct Fresh {
    ids: ToolIds,
    taken: BTreeSet<String>,
}

impl IdGen for Fresh {
    fn node_id(&mut self) -> NodeId {
        loop {
            let id = self.ids.node_id();
            if self.taken.insert(id.as_str().to_owned()) {
                return id;
            }
        }
    }
}

fn counts_json(counts: Counts) -> Value {
    json!({
        "created": counts.created,
        "changed": counts.changed,
        "removed": counts.removed,
        "variables": counts.variables,
    })
}

/// A failed script as the agent sees it: what was wrong, the line, and the lines around it.
fn script_error(diagnostic: &ScriptDiagnostic) -> ToolError {
    let (kind, reason) = match &diagnostic.error {
        ScriptError::Syntax { message } => {
            (ErrorKind::InvalidInput, format!("Syntax error: {message}"))
        }
        ScriptError::Runtime { message } => (
            ErrorKind::InvalidInput,
            format!("The script threw: {message}"),
        ),
        ScriptError::Api { message } => (ErrorKind::InvalidInput, sentence(message)),
        ScriptError::Timeout => (
            ErrorKind::Refused,
            format!(
                "The script ran past {} seconds.",
                EXECUTE_LIMITS.time_ms / 1000
            ),
        ),
        ScriptError::OpLimit => (
            ErrorKind::Refused,
            format!(
                "The script made more than {} calls to doc; split it.",
                EXECUTE_LIMITS.max_ops
            ),
        ),
        ScriptError::NodeLimit => (
            ErrorKind::Refused,
            format!(
                "The script made more than {} layers; split it.",
                EXECUTE_LIMITS.max_nodes
            ),
        ),
        ScriptError::MemoryLimit => (
            ErrorKind::Refused,
            format!("The script used more than {} MB.", EXECUTE_LIMITS.memory_mb),
        ),
        ScriptError::Forbidden { what } => (
            ErrorKind::Refused,
            format!("Scripts cannot use {what}: there is no network, file system or timer."),
        ),
    };
    let mut message = match diagnostic.line {
        Some(line) => format!("Line {line}: {reason} Nothing was changed."),
        None => format!("{reason} Nothing was changed."),
    };
    if !diagnostic.excerpt.is_empty() {
        message.push('\n');
        message.push_str(&diagnostic.excerpt);
    }
    let error = ToolError::new(kind, message).at_path("code");
    match diagnostic.line {
        Some(line) => error.at_line(line, diagnostic.column),
        None => error,
    }
}

/// The frames `agent` holds in `guard` at `now`.
fn held(
    guard: &GuardState,
    agent: &AgentId,
    now: vellum_core::ports::Timestamp,
) -> BTreeSet<NodeId> {
    guard
        .leases
        .live(now)
        .filter(|(_, holder)| *holder == agent)
        .map(|(frame, _)| frame.clone())
        .collect()
}

/// A script's return value for the agent. An image (`doc.screenshot`, `doc.export`) is put where front ends show
/// pictures.
fn returned(value: Value, out: &mut Value) {
    let is_image = value.as_object().is_some_and(|map| {
        map.get("mime").is_some_and(Value::is_string)
            && map.get("base64").is_some_and(Value::is_string)
    });
    if is_image {
        out["image"] = value;
    } else {
        out["value"] = value;
    }
}

impl Tool for Execute {
    fn name(&self) -> &'static str {
        "execute"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Run a short JavaScript program against the document's `doc` API: loops and variables do in one call \
         what would take many tool calls. The whole script is one change and one undo step; if any call fails, \
         nothing is applied and the error names the line. `validate_only` reports what would change. Limits: 5 \
         seconds, 2,000 calls, 2,000 new layers; no network, files or timers."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ExecuteInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::RanScript
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ExecuteInput {
            document,
            code,
            validate_only,
        } = parse_input(input)?;
        if code.trim().is_empty() {
            return Err(ToolError::invalid("code", "The script is empty."));
        }
        if code.len() > MAX_SOURCE {
            return Err(ToolError::invalid(
                "code",
                format!("A script is at most {MAX_SOURCE} bytes; split the work."),
            ));
        }
        ctx.note_script(&code);
        let Some(runtime) = ctx.docs().scripts() else {
            return Err(ToolError::new(
                ErrorKind::Internal,
                "This build of Vellum has no script sandbox, so execute is not available. Use the typed tools.",
            ));
        };
        let (path, before) = ctx.document(document.as_deref())?;
        let origin = ctx.origin();
        let Actor::Agent { agent } = origin.actor.clone() else {
            unreachable!("a tool's origin is its agent")
        };
        let now = ctx.docs().now();
        // The daemon's own Guard when it holds the file; an editor's Guard decides when the result is applied.
        let mut guard = ctx
            .docs()
            .headless_guard(&path)
            .cloned()
            .unwrap_or_default();
        guard
            .pause
            .check(&agent)
            .map_err(|why| ToolError::new(ErrorKind::Refused, why.to_string()))?;
        let held_before = held(&guard, &agent, now);

        let mut scratch = ctx.docs().scratch(&before);
        let ids = Rc::new(RefCell::new(Fresh {
            ids: ctx.docs().tool_ids(),
            taken: ids_in_use(&before),
        }));
        let output = doc_api::execute(
            &*runtime,
            &mut *scratch,
            ExecuteRequest {
                source: &code,
                origin: origin.clone(),
                validate_only: false,
                limits: EXECUTE_LIMITS,
            },
            ExecuteContext {
                ids,
                services: ctx.docs().services(),
                guard: &mut guard,
                now,
            },
        )
        .map_err(|d| script_error(&d))?;
        let value = doc_api::to_json(&output.value).map_err(|e| {
            ToolError::invalid(
                "code",
                format!("The script returned something that is not data: {e}"),
            )
        })?;
        let held_after = held(&guard, &agent, now);

        let ops = diff(&before, &output.document).map_err(|e| {
            ToolError::new(
                ErrorKind::Internal,
                format!("The script's result could not be compared: {e}"),
            )
        })?;
        let mut out = json!({
            "document": path,
            "committed": false,
            "calls": output.op_count,
        });
        returned(value, &mut out);

        if validate_only {
            let counts = output.changes.as_ref().map(Counts::of).unwrap_or_default();
            if let Some(changes) = &output.changes {
                ctx.noted(&path, &before, &output.document, changes);
            }
            out["changes"] = counts_json(counts);
            out["validateOnly"] = json!(true);
            return Ok(out);
        }

        // Claims first, so the transaction is checked against them; undone if the transaction is refused.
        let claimed: Vec<NodeId> = held_after.difference(&held_before).cloned().collect();
        let released: Vec<NodeId> = held_before.difference(&held_after).cloned().collect();
        let target = ctx.target(&path)?;
        for (n, frame) in claimed.iter().enumerate() {
            if let Err(error) = target.lease(frame, &agent, LeaseChange::Claim) {
                for earlier in &claimed[..n] {
                    let _ = target.lease(earlier, &agent, LeaseChange::Release);
                }
                return Err(error.into());
            }
        }
        let changes = if ops.is_empty() {
            None
        } else {
            match target.transact(Transaction::new(origin, ops)) {
                Ok(applied) => Some(applied.changes),
                Err(error) => {
                    for frame in &claimed {
                        let _ = target.lease(frame, &agent, LeaseChange::Release);
                    }
                    return Err(match error {
                        TargetError::Write(WriteError::Invalid(e)) => ToolError::invalid(
                            "code",
                            format!(
                                "The document changed while the script ran, and its result no longer applies: {e} \
                                 Nothing was changed; run it again."
                            ),
                        ),
                        other => super::refusal(ctx.docs().agents(), other),
                    });
                }
            }
        };
        for frame in &released {
            let _ = target.lease(frame, &agent, LeaseChange::Release);
        }
        let after = target.snapshot()?;
        let counts = match &changes {
            Some(changes) => {
                ctx.noted(&path, &before, &after, changes);
                Counts::of(changes)
            }
            None => Counts::default(),
        };
        out["committed"] = json!(changes.is_some());
        out["changes"] = counts_json(counts);
        if !claimed.is_empty() {
            out["claimed"] = json!(claimed.iter().map(NodeId::as_str).collect::<Vec<_>>());
        }
        Ok(out)
    }
}
