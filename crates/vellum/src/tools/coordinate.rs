//! Coordination: `claim_frame`, `release_frame`, `report_progress`, and `spawn_agents` for external clients when
//! the setting allows it (ARCHITECTURE §7.3, §7.5).
//!
//! A lease lives where the Guard does (the editor holding the file, or the daemon's own engine), so a claim made
//! here holds against every other agent's writes, whichever client they come from. A person is never blocked.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::guard::LeaseChange;

use super::batch::node_id;
use super::support::{parse_input, schema_of};
use super::{AgentSettings, ErrorKind, Tool, ToolCtx, ToolError, Verb};
use crate::daemon::{SpawnRequest, SpawnTask};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>, settings: &AgentSettings) {
    tools.push(Box::new(Lease(LeaseChange::Claim)));
    tools.push(Box::new(Lease(LeaseChange::Release)));
    tools.push(Box::new(ReportProgress));
    if settings.spawn_agents_external {
        tools.push(Box::new(SpawnAgents));
    }
}

/// The tool that starts helper agents. Engines Vellum runs always have it; external clients only when
/// [`AgentSettings::spawn_agents_external`] is on (ARCHITECTURE §7.4, §11 `[agents]`).
pub const SPAWN_AGENTS: &str = "spawn_agents";

pub(crate) const SPAWN_DESCRIPTION: &str = "Start helper agents for separate parts of the work, each with one     explicit task (an icon set, the empty states, a second page). They work at the same time as you, in this turn,     each claiming its own frames, and report through report_progress. Give each a task it can finish without you,     and leave their frames alone.";

/// The longest task, in characters.
const TASK_LIMIT: usize = 1_000;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpawnInput {
    /// The document. Defaults to the one opened last (for an engine Vellum runs, its turn's document).
    pub document: Option<String>,
    /// One helper per task.
    pub tasks: Vec<SpawnTaskInput>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpawnTaskInput {
    /// What the helper does, said so it can finish it alone.
    pub task: String,
    /// The model it runs on. Defaults to yours.
    pub model: Option<String>,
}

impl SpawnInput {
    /// At least one task, none empty or too long, and at most one helper per other lane; for an engine's turn,
    /// only its own document (`turn_document`).
    pub(crate) fn check(&self, turn_document: Option<&str>) -> Result<(), ToolError> {
        let most = vellum_core::parallel::MAX_LANES - 1;
        if self.tasks.is_empty() || self.tasks.len() > most {
            return Err(ToolError::invalid(
                "tasks",
                format!("Give 1 to {most} tasks, one per helper."),
            ));
        }
        for (n, task) in self.tasks.iter().enumerate() {
            let length = task.task.trim().chars().count();
            if length == 0 || length > TASK_LIMIT {
                return Err(ToolError::invalid(
                    format!("tasks[{n}].task"),
                    format!("Say the task in 1 to {TASK_LIMIT} characters."),
                ));
            }
            if task.model.as_deref().is_some_and(|m| m.trim().is_empty()) {
                return Err(ToolError::invalid(
                    format!("tasks[{n}].model"),
                    "Name a model, or leave it out.",
                ));
            }
        }
        if let (Some(asked), Some(turn)) = (self.document.as_deref(), turn_document)
            && asked != turn
        {
            return Err(ToolError::invalid(
                "document",
                format!("Helpers work on this turn's document, {turn}."),
            ));
        }
        Ok(())
    }
}

/// `spawn_agents` for external clients: the request is queued for the host that runs helper agents.
struct SpawnAgents;

impl Tool for SpawnAgents {
    fn name(&self) -> &'static str {
        SPAWN_AGENTS
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        SPAWN_DESCRIPTION
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<SpawnInput>()
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
        let input: SpawnInput = parse_input(input)?;
        input.check(None)?;
        let (path, _) = ctx.document(input.document.as_deref())?;
        let origin = ctx.origin();
        let vellum_types::Actor::Agent { agent } = origin.actor else {
            unreachable!("a tool's origin is its agent")
        };
        let request = SpawnRequest {
            by: agent,
            by_name: ctx.session().agent_name(),
            document: path.clone(),
            tasks: input
                .tasks
                .into_iter()
                .map(|t| SpawnTask {
                    task: t.task,
                    model: t.model,
                })
                .collect(),
        };
        let count = request.tasks.len();
        ctx.docs().request_spawn(request).map_err(|_| {
            ToolError::new(
                ErrorKind::Refused,
                "Nothing here runs helper agents yet: open the document in the Vellum editor, or do the work \
                 yourself.",
            )
        })?;
        Ok(json!({"document": path, "queued": count,
                  "message": "The helpers start shortly and report through report_progress. Leave their frames alone."}))
    }
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
    /// The numbers of the plan items finished since the last report. Their placeholders give way to the frames.
    items: Option<Vec<u32>>,
}

/// The longest progress message kept.
const MESSAGE_LIMIT: usize = 500;

impl Tool for ReportProgress {
    fn name(&self) -> &'static str {
        "report_progress"
    }
    fn version(&self) -> u32 {
        2
    }
    fn description(&self) -> &'static str {
        "Tell the person how the work is going: what is done and what comes next, and how many plan items are \
         done. When building an approved plan, name the items you just finished in `items` so they tick off. It \
         changes nothing in the document. Call it between parts of a long piece of work."
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
            items,
        } = parse_input(input)?;
        if items.as_ref().is_some_and(Vec::is_empty) {
            return Err(ToolError::invalid(
                "items",
                "Name at least one item, or leave items out.",
            ));
        }
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
        ctx.note_progress(message, progress, items.unwrap_or_default());
        Ok(json!({"recorded": true}))
    }
}
