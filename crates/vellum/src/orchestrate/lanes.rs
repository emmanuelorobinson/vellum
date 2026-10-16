//! Lanes: engine runs of one turn at the same time, each by its own agent on its own engine (ARCHITECTURE §7.5).
//!
//! Every lane calls tools through a [`LaneBridge`] over the same documents and plan. The futures of all lanes are
//! polled in turn by [`join`], on the one thread the daemon has, so a tool call of one lane never overlaps
//! another's. A helper started with `spawn_agents` joins the running set as a new lane. Before every tool call,
//! what a person pressed meanwhile (Stop, Pause all) is applied to the Guard, so the call is refused there.
use std::cell::{Cell, RefCell};
use std::future::{Future, poll_fn};
use std::pin::Pin;
use std::task::Poll;

use serde_json::{Value, json};
use vellum_core::guard::{AgentControl, LeaseChange};
use vellum_core::ops;
use vellum_core::parallel::{LaneEnd, LaneRecord, MAX_LANES, Mode, ParallelEvent, helper_prompt};
use vellum_core::plans::{PlanError, PlanEvent, PlanState};
use vellum_core::ports::{
    BridgeTool, EventBody, ModelTool, Timestamp, ToolBridge, ToolReply, TurnEnd, TurnEvent,
    TurnRequest,
};
use vellum_core::sessions::record_transition;
use vellum_core::turns::TurnInput;
use vellum_types::{AgentId, Node, NodeId, Origin};

use super::{
    LiveLane, OrchestrationError, Orchestrator, PROPOSE_DESCRIPTION, PROPOSE_PLAN, Phase,
    ProposeInput, Run, TurnSpec, resolve,
};
use crate::daemon::Documents;
use crate::tools::{
    ErrorKind, SPAWN_AGENTS, SPAWN_DESCRIPTION, Session, SpawnInput, ToolCtx, ToolError,
    parse_input, schema_of,
};

/// A lane about to start.
pub(super) struct LaneRun {
    /// The engine it runs on (0 is the main one).
    pub engine: usize,
    /// Its agent's connection, made with [`Session::for_agent`].
    pub session: Session,
    pub request: TurnRequest,
    pub phase: Phase,
    /// It is in a `Lanes` event, so how it ended is logged too.
    pub listed: bool,
}

/// How a lane ended.
pub(super) struct LaneOutcome {
    /// When it started among the turn's lanes.
    pub order: usize,
    pub agent: AgentId,
    pub name: String,
    pub session: Session,
    pub phase: Phase,
    pub end: TurnEnd,
    /// A person pressed Stop this agent on it.
    pub halted: bool,
    pub listed: bool,
    /// Its last progress message (an Explore variant's label).
    pub report: Option<String>,
    /// The first thing that could not be logged, or the engine refusing to start.
    pub error: Option<OrchestrationError>,
}

/// What every lane of a run shares.
pub(super) struct Shared<'o> {
    pub orch: &'o Orchestrator<'o>,
    pub spec: &'o TurnSpec,
    pub docs: RefCell<&'o mut Documents>,
    /// The turn's plan, ticked by any lane.
    pub plan: RefCell<Option<PlanState>>,
    /// Helpers spawned since the last poll, for [`join`] to start.
    pub spawned: RefCell<Vec<LaneRun>>,
    /// How many lanes the run has had.
    pub lanes: Cell<usize>,
    /// What a helper's request is made from (skills, limits, effort, model).
    pub template: TurnRequest,
}

impl Orchestrator<'_> {
    /// Applies what a person pressed while a run held the documents.
    pub(super) fn flush(&self, docs: &mut Documents) {
        let pending = std::mem::take(&mut *self.pending.borrow_mut());
        for change in &pending {
            docs.control_all(change);
        }
    }

    /// Notes that `agent` works on `engine` in `spec`'s turn.
    pub(super) fn enlist(&self, spec: &TurnSpec, agent: &AgentId, engine: usize) {
        let mut running = self.running.borrow_mut();
        let run = running.entry(spec.turn.clone()).or_insert_with(|| Run {
            spec: spec.clone(),
            lanes: vec![],
        });
        if !run.lanes.iter().any(|lane| &lane.agent == agent) {
            run.lanes.push(LiveLane {
                agent: agent.clone(),
                engine,
                halted: false,
                ended: false,
            });
        }
    }

    /// Notes that `agent`'s lane ended; whether a person had stopped it.
    fn ended(&self, spec: &TurnSpec, agent: &AgentId) -> bool {
        let mut running = self.running.borrow_mut();
        let Some(lane) = running
            .get_mut(&spec.turn)
            .and_then(|run| run.lanes.iter_mut().find(|lane| &lane.agent == agent))
        else {
            return false;
        };
        lane.ended = true;
        lane.halted
    }

    /// Engines no working lane of any turn is on.
    pub(super) fn free_engines(&self) -> Vec<usize> {
        let running = self.running.borrow();
        (0..self.engines())
            .filter(|n| {
                !running
                    .values()
                    .flat_map(|run| &run.lanes)
                    .any(|lane| lane.engine == *n && !lane.ended)
            })
            .collect()
    }

    /// Runs `runs` as lanes of `spec`'s turn until every one (and every helper they start) has ended. Returns how
    /// each ended, in the order they started, and whether the turn had a plan.
    pub(super) async fn run_lanes(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        runs: Vec<LaneRun>,
        plan: Option<PlanState>,
        template: TurnRequest,
    ) -> (Vec<LaneOutcome>, bool) {
        let _running = RunGuard {
            orch: self,
            turn: spec.turn.clone(),
        };
        for run in &runs {
            if let Some(agent) = run.session.agent() {
                self.enlist(spec, agent, run.engine);
            }
        }
        let shared = Shared {
            orch: self,
            spec,
            docs: RefCell::new(docs),
            plan: RefCell::new(plan),
            spawned: RefCell::default(),
            lanes: Cell::new(runs.len()),
            template,
        };
        let outcomes = join(&shared, runs).await;
        let planned = shared.plan.into_inner().is_some();
        (outcomes, planned)
    }
}

/// Takes the turn off the running list however its run ends.
struct RunGuard<'o> {
    orch: &'o Orchestrator<'o>,
    turn: vellum_types::TurnId,
}
impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.orch.running.borrow_mut().remove(&self.turn);
    }
}

/// Polls every lane, and every helper they start, until all have ended.
async fn join(shared: &Shared<'_>, runs: Vec<LaneRun>) -> Vec<LaneOutcome> {
    type Lane<'s> = Pin<Box<dyn Future<Output = LaneOutcome> + 's>>;
    let mut order = 0;
    let mut lanes: Vec<Lane<'_>> = vec![];
    for run in runs {
        lanes.push(Box::pin(lane(shared, run, order)));
        order += 1;
    }
    let mut outcomes = vec![];
    poll_fn(|cx| {
        loop {
            let mut n = 0;
            while n < lanes.len() {
                match lanes[n].as_mut().poll(cx) {
                    Poll::Ready(outcome) => {
                        outcomes.push(outcome);
                        drop(lanes.remove(n));
                    }
                    Poll::Pending => n += 1,
                }
            }
            let spawned: Vec<LaneRun> = shared.spawned.borrow_mut().drain(..).collect();
            if spawned.is_empty() {
                break;
            }
            for run in spawned {
                lanes.push(Box::pin(lane(shared, run, order)));
                order += 1;
            }
        }
        if lanes.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    outcomes.sort_by_key(|outcome| outcome.order);
    outcomes
}

/// One lane: its engine's run through its own bridge, its usage and messages logged as its agent's.
async fn lane(shared: &Shared<'_>, run: LaneRun, order: usize) -> LaneOutcome {
    let orch = shared.orch;
    let spec = shared.spec;
    let LaneRun {
        engine,
        mut session,
        request,
        phase,
        listed,
    } = run;
    let agent = session
        .agent()
        .cloned()
        .expect("a lane's session has its agent");
    let name = session.agent_name();
    let origin = Orchestrator::origin_of(&agent, spec);
    orch.enlist(spec, &agent, engine);
    {
        let mut docs = shared.docs.borrow_mut();
        orch.flush(&mut docs);
        // The lane's tools work on its turn's document unless told otherwise.
        if docs.target(&spec.document).is_ok() {
            docs.set_active(&spec.document);
        }
        docs.name_agent(agent.clone(), name.clone());
        // A new turn lifts the agent's stop from the last one; Pause all stays.
        docs.control_all(&AgentControl::Resume(agent.clone()));
        orch.clock(&docs);
    }
    session.set_turn(Some(spec.turn.clone()));
    // Its steps on the turn's document go to the turn's session, beside its messages and usage.
    session.log_path_as(spec.document.clone(), spec.session.clone());
    let model = request.model.clone();
    let bridge = LaneBridge {
        shared,
        agent: agent.clone(),
        name: name.clone(),
        model: model.clone(),
        origin: origin.clone(),
        state: RefCell::new(LaneState {
            session,
            phase,
            error: None,
            report: None,
        }),
    };
    let mut end = TurnEnd::Done;
    let mut error = None;
    match orch.engine_at(engine).run_turn(request, &bridge) {
        Err(e) => {
            end = TurnEnd::Failed(e);
            error = Some(OrchestrationError::Engine(e));
        }
        Ok(mut stream) => {
            while let Some(event) = stream.next().await {
                let at = orch.clock(&shared.docs.borrow());
                match &event {
                    TurnEvent::Usage(_) | TurnEvent::Messages(_) => {
                        if let Err(e) = orch.log.record_turn_event(&origin, &model, &event, at) {
                            error.get_or_insert(e.into());
                        }
                    }
                    TurnEvent::Ended(how) => end = *how,
                    _ => {}
                }
            }
        }
    }
    let LaneState {
        mut session,
        phase,
        error: logging,
        report,
    } = bridge.state.into_inner();
    session.set_turn(None);
    {
        let mut docs = shared.docs.borrow_mut();
        // A Stop pressed while it ran reaches the Guard even if it made no call since.
        orch.flush(&mut docs);
        if let Err(e) = orch.log.record_steps(&mut docs, &mut session) {
            error.get_or_insert(e.into());
        }
        release(&mut docs, spec, &agent);
    }
    let halted = orch.ended(spec, &agent);
    LaneOutcome {
        order,
        agent,
        name,
        session,
        phase,
        end,
        halted,
        listed,
        report,
        error: logging.or(error),
    }
}

/// How lanes that ended this way end the turn: done when every lane a person did not stop is done.
pub(super) fn combine(ends: impl Iterator<Item = (TurnEnd, bool)>) -> TurnEnd {
    let mut combined = TurnEnd::Done;
    for (end, halted) in ends {
        match (end, halted) {
            (_, true) | (TurnEnd::Done, _) => {}
            (TurnEnd::Failed(e), _) => return TurnEnd::Failed(e),
            (TurnEnd::Stopped, _) => combined = TurnEnd::Stopped,
        }
    }
    combined
}

/// How a lane that a person did not stop ended. A run that ends without finishing failed.
pub(super) fn lane_end(end: TurnEnd) -> LaneEnd {
    match end {
        TurnEnd::Done => LaneEnd::Done,
        TurnEnd::Failed(_) | TurnEnd::Stopped => LaneEnd::Failed,
    }
}

/// Lets go of every frame `agent` still holds in the turn's document: its lane is over.
fn release(docs: &mut Documents, spec: &TurnSpec, agent: &AgentId) {
    let Ok(view) = docs.view(&spec.document) else {
        return;
    };
    let Ok(target) = docs.target(&spec.document) else {
        return;
    };
    for lease in view.state.leases.iter().filter(|l| &l.agent == agent) {
        let _ = target.lease(&lease.frame, agent, LeaseChange::Release);
    }
}

/// Removes the turn's placeholders and lets go of the frames it was filling.
pub(super) fn clear(docs: &mut Documents, spec: &TurnSpec) {
    let gone = docs.unreserve(&spec.document, |p| p.turn == spec.turn);
    for planned in gone.iter().filter(|p| p.filling) {
        if let Ok(target) = docs.target(&spec.document) {
            // It may have let go already, or the lease lapsed.
            let _ = target.lease(
                &planned.placeholder.frame,
                &planned.placeholder.agent,
                LeaseChange::Release,
            );
        }
    }
}

struct LaneState {
    session: Session,
    phase: Phase,
    /// The first failure to log something; the run reports it when it ends.
    error: Option<OrchestrationError>,
    report: Option<String>,
}

/// The registry as one lane's engine sees it: `propose_plan` and `spawn_agents` added, writes held back while a
/// plan waits, ticks and placeholders kept up to date.
struct LaneBridge<'s, 'o> {
    shared: &'s Shared<'o>,
    agent: AgentId,
    name: String,
    model: String,
    origin: Origin,
    state: RefCell<LaneState>,
}

fn failed(error: ToolError) -> ToolReply {
    ToolReply {
        value: error.to_json(),
        failed: true,
    }
}

impl LaneBridge<'_, '_> {
    fn log(
        &self,
        docs: &Documents,
        state: &mut LaneState,
        write: impl FnOnce(Timestamp) -> Result<(), OrchestrationError>,
    ) -> bool {
        let at = self.shared.orch.clock(docs);
        match write(at) {
            Ok(()) => true,
            Err(error) => {
                state.error.get_or_insert(error);
                false
            }
        }
    }

    fn append(&self, docs: &Documents, state: &mut LaneState, body: EventBody) -> bool {
        self.log(docs, state, |at| {
            self.shared
                .orch
                .append(&self.origin, body, at)
                .map_err(OrchestrationError::from)
        })
    }

    fn transition(&self, docs: &Documents, state: &mut LaneState, input: TurnInput) -> bool {
        let store = self.shared.orch.log.store();
        self.log(docs, state, |at| {
            record_transition(store, &self.origin, input, at)
                .map(|_| ())
                .map_err(OrchestrationError::from)
        })
    }

    /// A first write while planning means the engine judged the request small: no plan is needed.
    fn building(&self, docs: &Documents, state: &mut LaneState) -> Result<(), ToolReply> {
        match state.phase {
            Phase::AwaitingPlan => Err(failed(ToolError::new(
                ErrorKind::Refused,
                "The plan is waiting for the person. End your turn now; nothing is built until they press Build.",
            ))),
            Phase::Planning => {
                if !self.transition(docs, state, TurnInput::NoPlanNeeded) {
                    return Err(failed(ToolError::new(
                        ErrorKind::Internal,
                        "The turn could not move on.",
                    )));
                }
                state.phase = Phase::Building;
                Ok(())
            }
            Phase::Building => Ok(()),
        }
    }

    fn propose(&self, docs: &mut Documents, state: &mut LaneState, input: Value) -> ToolReply {
        match state.phase {
            Phase::Planning => {}
            Phase::AwaitingPlan => {
                return failed(ToolError::new(
                    ErrorKind::Refused,
                    "A plan is already waiting for the person. End your turn now.",
                ));
            }
            Phase::Building => {
                return failed(ToolError::new(
                    ErrorKind::Refused,
                    "The plan is settled; build it now.",
                ));
            }
        }
        let input: ProposeInput = match parse_input(input) {
            Ok(input) => input,
            Err(error) => return failed(error),
        };
        let doc = match docs
            .target(&self.shared.spec.document)
            .and_then(|target| target.snapshot())
        {
            Ok(doc) => doc,
            Err(error) => return failed(error.into()),
        };
        let plan = match resolve(&doc, input) {
            Ok(plan) => plan,
            Err(error) => return failed(error),
        };
        if !self.shared.orch.settings.needs_plan(&plan) {
            if !self.transition(docs, state, TurnInput::NoPlanNeeded) {
                return failed(ToolError::new(
                    ErrorKind::Internal,
                    "The turn could not move on.",
                ));
            }
            state.phase = Phase::Building;
            return ToolReply {
                value: json!({"plan": "not_needed", "message": "This is small enough to build without a plan. Build it now."}),
                failed: false,
            };
        }
        let items = plan.items.len();
        let logged = self.transition(docs, state, TurnInput::PlanNeeded)
            && self.append(
                docs,
                state,
                EventBody::Plan(PlanEvent::Proposed(plan.clone())),
            );
        if !logged {
            return failed(ToolError::new(
                ErrorKind::Internal,
                "The plan could not be recorded.",
            ));
        }
        state.phase = Phase::AwaitingPlan;
        *self.shared.plan.borrow_mut() = Some(PlanState::proposed(plan));
        ToolReply {
            value: json!({
                "plan": "proposed",
                "items": items,
                "message": "The plan is with the person. End your turn now with one sentence on what you plan; \
                            you will be told when to build.",
            }),
            failed: false,
        }
    }

    /// `spawn_agents`: helpers start as new lanes of this turn, each on a free engine with the next free name.
    fn spawn(&self, docs: &mut Documents, state: &mut LaneState, input: Value) -> ToolReply {
        let input: SpawnInput = match parse_input(input) {
            Ok(input) => input,
            Err(error) => return failed(error),
        };
        if let Err(error) = input.check(Some(&self.shared.spec.document)) {
            return failed(error);
        }
        if let Err(reply) = self.building(docs, state) {
            return reply;
        }
        let orch = self.shared.orch;
        let spec = self.shared.spec;
        let engines = orch.free_engines();
        let room = MAX_LANES.saturating_sub(self.shared.lanes.get());
        let used: Vec<String> = self.shared.orch.running.borrow()[&spec.turn]
            .lanes
            .iter()
            .filter_map(|lane| docs.agents().get(&lane.agent).cloned())
            .collect();
        let names: Vec<&String> = orch
            .names
            .iter()
            .filter(|name| !used.contains(name) && **name != self.name)
            .collect();
        let free = engines.len().min(room).min(names.len());
        if input.tasks.len() > free {
            return failed(ToolError::new(
                ErrorKind::Refused,
                format!(
                    "{} more agents cannot start now: {free} can. Do the rest yourself, or ask for fewer.",
                    input.tasks.len()
                ),
            ));
        }
        let mut runs = vec![];
        let mut records = vec![];
        for ((task, engine), name) in input.tasks.iter().zip(engines).zip(names) {
            let agent = AgentId::new(docs.ulid()).expect("a ULID is an id");
            let model = task.model.clone().unwrap_or_else(|| self.model.clone());
            let mut request = self.shared.template.clone();
            request.turn = spec.turn.clone();
            request.model = model.clone();
            request.prompt = helper_prompt(&task.task, &self.name);
            request.images = vec![];
            request.history = vec![];
            orch.enlist(spec, &agent, engine);
            records.push(LaneRecord {
                agent: agent.clone(),
                name: name.clone(),
                model,
                items: vec![],
                task: Some(task.task.clone()),
            });
            runs.push(LaneRun {
                engine,
                session: Session::for_agent(name.clone(), agent, spec.session.clone()),
                request,
                phase: Phase::Building,
                listed: true,
            });
        }
        if !self.append(
            docs,
            state,
            EventBody::Parallel(ParallelEvent::Lanes {
                mode: Mode::Spawned,
                lanes: records.clone(),
            }),
        ) {
            return failed(ToolError::new(
                ErrorKind::Internal,
                "The helpers could not be recorded.",
            ));
        }
        self.shared.lanes.set(self.shared.lanes.get() + runs.len());
        self.shared.spawned.borrow_mut().extend(runs);
        ToolReply {
            value: json!({
                "spawned": records.iter().map(|r| json!({
                    "agent": r.agent.as_str(), "name": r.name, "model": r.model, "task": r.task,
                })).collect::<Vec<_>>(),
                "message": "They work alongside you in this turn and report through report_progress. Leave their \
                            frames alone.",
            }),
            failed: false,
        }
    }

    /// Placeholders of this lane's agent whose frame now exists are being filled: the frame is leased to it.
    fn fill(&self, docs: &mut Documents) {
        let path = &self.shared.spec.document;
        let waiting: Vec<NodeId> = docs
            .placeholders(path)
            .iter()
            .filter(|p| {
                !p.filling
                    && p.placeholder.turn == self.shared.spec.turn
                    && p.placeholder.agent == self.agent
            })
            .map(|p| p.placeholder.frame.clone())
            .collect();
        if waiting.is_empty() {
            return;
        }
        let Ok(doc) = docs.target(path).and_then(|t| t.snapshot()) else {
            return;
        };
        for frame in waiting {
            if !matches!(ops::find(&doc, &frame), Some(Node::Frame(_))) {
                continue;
            }
            let claimed = docs
                .target(path)
                .and_then(|t| t.lease(&frame, &self.agent, LeaseChange::Claim));
            if claimed.is_ok() {
                docs.filling(path, &frame);
            }
        }
    }

    /// Checks a progress report's `items` against the plan before the tool runs.
    fn ticks(&self, input: &Value) -> Result<Option<Vec<u32>>, ToolError> {
        let Some(items) = input.get("items").and_then(Value::as_array) else {
            return Ok(None);
        };
        let items: Vec<u32> = items
            .iter()
            .filter_map(|n| n.as_u64().and_then(|n| u32::try_from(n).ok()))
            .collect();
        let plan = self.shared.plan.borrow();
        let Some(plan) = plan.as_ref() else {
            return Err(ToolError::invalid(
                "items",
                "This turn has no plan; leave out items.",
            ));
        };
        if let Some(unknown) = items.iter().find(|id| plan.plan.item(**id).is_none()) {
            return Err(ToolError::invalid(
                "items",
                PlanError::UnknownItem(*unknown).to_string(),
            ));
        }
        Ok(Some(items))
    }
}

impl ToolBridge for LaneBridge<'_, '_> {
    fn tools(&self) -> Vec<BridgeTool> {
        let schema =
            |schema: schemars::Schema| serde_json::to_value(schema).expect("a schema is JSON");
        let mut tools: Vec<BridgeTool> = self
            .shared
            .orch
            .registry
            .tools()
            .filter(|tool| tool.name() != SPAWN_AGENTS)
            .map(|tool| BridgeTool {
                tool: ModelTool {
                    name: tool.name().to_owned(),
                    description: tool.description().to_owned(),
                    parameters: schema(tool.input_schema()),
                },
                read_only: tool.read_only(),
            })
            .collect();
        tools.push(BridgeTool {
            tool: ModelTool {
                name: PROPOSE_PLAN.to_owned(),
                description: PROPOSE_DESCRIPTION.to_owned(),
                parameters: schema(schema_of::<ProposeInput>()),
            },
            read_only: false,
        });
        tools.push(BridgeTool {
            tool: ModelTool {
                name: SPAWN_AGENTS.to_owned(),
                description: SPAWN_DESCRIPTION.to_owned(),
                parameters: schema(schema_of::<SpawnInput>()),
            },
            read_only: false,
        });
        tools
    }

    fn call(&self, name: &str, input: Value) -> ToolReply {
        let mut docs = self.shared.docs.borrow_mut();
        let docs: &mut Documents = &mut docs;
        // What a person pressed since the last call reaches the Guard first.
        self.shared.orch.flush(docs);
        let mut guard = self.state.borrow_mut();
        let state = &mut *guard;
        if name == PROPOSE_PLAN {
            return self.propose(docs, state, input);
        }
        if name == SPAWN_AGENTS {
            return self.spawn(docs, state, input);
        }
        let writes = self
            .shared
            .orch
            .registry
            .get(name)
            .is_some_and(|tool| !tool.read_only());
        if writes && let Err(reply) = self.building(docs, state) {
            return reply;
        }
        let ticks = if name == "report_progress" {
            match self.ticks(&input) {
                Ok(ticks) => ticks,
                Err(error) => return failed(error),
            }
        } else {
            None
        };
        let message = input
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let result = {
            let mut ctx = ToolCtx::new(docs, &mut state.session);
            self.shared.orch.registry.call(&mut ctx, name, input)
        };
        let mut reply = match result {
            Ok(value) => ToolReply {
                value,
                failed: false,
            },
            Err(error) => failed(error),
        };
        if !reply.failed {
            if name == "report_progress" {
                state.report = message;
            }
            let ticked = ticks.and_then(|items| {
                let mut plan = self.shared.plan.borrow_mut();
                let plan = plan.as_mut()?;
                let new = plan.tick(&items).unwrap_or_default();
                Some((new, plan.progress()))
            });
            if let Some((new, (done, total))) = ticked {
                if !new.is_empty() {
                    let spec = self.shared.spec;
                    docs.unreserve(&spec.document, |p| {
                        p.turn == spec.turn && new.contains(&p.item)
                    });
                    self.append(docs, state, EventBody::Plan(PlanEvent::Ticked(new)));
                }
                reply.value["plan"] = json!({"done": done, "total": total});
            }
            if writes {
                self.fill(docs);
            }
        }
        if let Err(error) = self.shared.orch.log.record_steps(docs, &mut state.session) {
            state.error.get_or_insert(error.into());
        }
        reply
    }
}
