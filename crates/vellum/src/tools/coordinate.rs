//! Coordination: `claim_frame`, `release_frame`, `report_progress` (ARCHITECTURE §7.3, §7.5).
//!
//! A lease lives where the Guard does (the editor holding the file, or the daemon's own engine), so a claim made
//! here holds against every other agent's writes, whichever client they come from. A person is never blocked.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::guard::LeaseChange;

use super::batch::node_id;
use super::support::{parse_input, schema_of};
use super::{Tool, ToolCtx, ToolError, Verb};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(Lease(LeaseChange::Claim)));
    tools.push(Box::new(Lease(LeaseChange::Release)));
    tools.push(Box::new(ReportProgress));
}

/// How long a claim lasts without activity, as the lease state machine sets it (guide §10.2).
const LEASE_SECONDS: u32 = 60;

struct Lease(LeaseChange);

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LeaseInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The frame, by id.
    frame: String,
}

impl Tool for Lease {
    fn name(&self) -> &'static str {
        match self.0 {
            LeaseChange::Claim => "claim_frame",
            LeaseChange::Release => "release_frame",
        }
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        match self.0 {
            LeaseChange::Claim => {
                "Claim a frame before working in it, so other agents cannot change it while you do. The claim \
                 lapses after 60 seconds without a change from you, and a person can always edit or stop it. \
                 Claim fails when another agent holds the frame; pick another one or wait."
            }
            LeaseChange::Release => {
                "Let go of a frame you claimed, when you are done with it, so other agents can work there."
            }
        }
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<LeaseInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        match self.0 {
            LeaseChange::Claim => Verb::Claimed,
            LeaseChange::Release => Verb::Released,
        }
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let LeaseInput { document, frame } = parse_input(input)?;
        let id = node_id(&frame, "frame")?;
        let (path, doc) = ctx.document(document.as_deref())?;
        ctx.note_target(&doc, &frame);
        let origin = ctx.origin();
        let vellum_types::Actor::Agent { agent } = origin.actor else {
            unreachable!("a tool's origin is its agent")
        };
        ctx.target(&path)?.lease(&id, &agent, self.0)?;
        Ok(match self.0 {
            LeaseChange::Claim => {
                json!({"document": path, "frame": frame, "claimed": true, "idleSeconds": LEASE_SECONDS})
            }
            LeaseChange::Release => json!({"document": path, "frame": frame, "released": true}),
        })
    }
}

struct ReportProgress;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProgressInput {
    /// One or two plain sentences on what is done and what is next. Shown to the person as it is.
    message: String,
    /// How many items of the plan are done.
    done: Option<u32>,
    /// How many items the plan has.
    total: Option<u32>,
}

/// The longest progress message kept.
const MESSAGE_LIMIT: usize = 500;

impl Tool for ReportProgress {
    fn name(&self) -> &'static str {
        "report_progress"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Tell the person how the work is going: what is done and what comes next, and how many plan items are \
         done. It changes nothing in the document. Call it between parts of a long piece of work."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ProgressInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Progress
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ProgressInput {
            message,
            done,
            total,
        } = parse_input(input)?;
        let message = message.trim();
        if message.is_empty() {
            return Err(ToolError::invalid(
                "message",
                "Say what is done and what is next.",
            ));
        }
        if message.chars().count() > MESSAGE_LIMIT {
            return Err(ToolError::invalid(
                "message",
                format!("Keep it under {MESSAGE_LIMIT} characters."),
            ));
        }
        let progress = match (done, total) {
            (None, None) => None,
            (Some(done), Some(total)) if done <= total => Some((done, total)),
            (Some(_), Some(_)) => {
                return Err(ToolError::invalid("done", "done is at most total."));
            }
            (Some(_), None) => return Err(ToolError::invalid("total", "Give total with done.")),
            (None, Some(_)) => return Err(ToolError::invalid("done", "Give done with total.")),
        };
        ctx.note_progress(message, progress);
        Ok(json!({"recorded": true}))
    }
}
