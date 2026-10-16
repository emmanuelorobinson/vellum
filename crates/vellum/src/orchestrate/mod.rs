//! Orchestrating engine turns: planning, plans, placeholders and leases (Prompt 38), and parallel lanes, helper
//! agents, Stop, Pause all and Undo this turn (Prompt 39) (ARCHITECTURE §7.5; guide §10.1).
//!
//! A turn starts in `planning`. The engine is offered two more tools than MCP clients get: `propose_plan` and
//! `spawn_agents`. A plan over the threshold ([`PlanSettings`]) moves the turn to `awaiting_plan`: it is logged,
//! the engine is told to end its run, and any write it still tries is refused. A plan under the threshold, or a
//! first write without a plan, means no plan is needed and the run goes on building. The person then presses Build
//! (with the plan as they edited it, on one agent or split across several), asks again (Edit request, a new
//! planning run) or stops.
//!
//! Build places a placeholder for every new frame: an id reserved for the frame, held by the agent building it,
//! with planned bounds from `find_space` that keep clear of the page's frames and of every other placeholder. They
//! live in the daemon's documents (session state, never the file), so every agent sees them through
//! `get_app_state` and `find_space` keeps away from them. When an agent creates a frame with a reserved id, the
//! frame is leased to it; when it reports the item done (`report_progress` with `items`), the item ticks off and
//! its placeholder goes. Stopped, failed or finished, the turn's placeholders go with it, and nothing of an
//! abandoned plan is in the file.
//!
//! A turn may run several **lanes** at once (`lanes`): Split, Side by side, Explore's successive variants and
//! helpers an agent spawns. Each lane is its own engine run by its own agent, with its own leases; all of them
//! carry the turn, so Undo this turn takes all of them back (`control`).
//!
//! The orchestrator is single-threaded like the daemon: its futures borrow the documents. A person's Stop or Pause
//! all may come while a run is in progress (between polls): the engines are interrupted at once, and what the
//! Guard must refuse is applied before the next tool call of any lane reaches the documents.
mod control;
mod lanes;
mod parallel;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;

use schemars::JsonSchema;
use serde::Deserialize;
use vellum_core::guard::AgentControl;
use vellum_core::ops;
use vellum_core::parallel::ParallelError;
use vellum_core::plans::{
    Placeholder, Plan, PlanError, PlanEvent, PlanItem, PlanSettings, PlanState, PlanStatus,
    build_prompt, check, plan_state,
};
use vellum_core::ports::{
    AgentEngine, EngineError, EventBody, NewEvent, SessionEvent, StoreError, Timestamp, TurnEnd,
    TurnRequest,
};
use vellum_core::sessions::{TransitionError, history, record_transition, turn_state};
use vellum_core::turns::{TurnInput, TurnRejection, TurnState, transition};
use vellum_types::{Actor, AgentId, Document, Node, NodeId, Origin, SessionId, TurnId};

use crate::daemon::Documents;
use crate::sessions::SessionLog;
use crate::tools::{Registry, Session, ToolError, free_space};

use lanes::{LaneOutcome, LaneRun};
pub use parallel::{Area, LaneResult, LaneSpec, ParallelOutcome};

/// The tool only engines run by Vellum get: proposing a plan.
pub const PROPOSE_PLAN: &str = "propose_plan";

const PROPOSE_DESCRIPTION: &str = "Before building a request that makes or changes more than one frame, or takes \
    more than about 20 changes, propose a plan: each frame with its page, a one-line intent and, for a new frame, \
    its size. A plan the person reviews first is answered with `proposed`: then end your turn with one sentence \
    on what you plan, and build only when told. A small one is answered with `not_needed`: build it at once.";

/// The turn an orchestrator call is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnSpec {
    /// The session-store session the turn is in (made by the host).
    pub session: SessionId,
    pub turn: TurnId,
    /// The person who asked, presses Build, Edit request and Stop.
    pub person: Actor,
    /// The daemon path of the document the turn works on.
    pub document: String,
}

#[derive(Debug)]
pub enum OrchestrationError {
    /// The turn cannot do that now, or its move could not be logged.
    Turn(TransitionError),
    Store(StoreError),
    /// The plan as the person edited it is not a plan.
    Plan(PlanError),
    /// The document refused, or the plan names a page or frame it does not have, or Build could not place it.
    Tool(ToolError),
    Engine(EngineError),
    /// The tool session has no agent; make it with [`Session::for_agent`].
    NoAgent,
    /// There is no plan waiting for Build.
    NoPlan,
    /// Too many or too few lanes or variants.
    Parallel(ParallelError),
    /// Each lane runs on its own engine, and there are not enough ([`Orchestrator::with_helpers`]).
    Engines {
        needed: usize,
        have: usize,
    },
    /// No lane of the turn is that agent's, or it has ended.
    NoLane(AgentId),
}
impl fmt::Display for OrchestrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Turn(e) => e.fmt(f),
            Self::Store(e) => e.fmt(f),
            Self::Plan(e) => e.fmt(f),
            Self::Tool(e) => e.fmt(f),
            Self::Engine(e) => e.fmt(f),
            Self::NoAgent => f.write_str("the engine's session has no agent"),
            Self::NoPlan => f.write_str("no plan is waiting for Build"),
            Self::Parallel(e) => e.fmt(f),
            Self::Engines { needed, have } => {
                write!(
                    f,
                    "{needed} agents need {needed} engines; {have} are set up"
                )
            }
            Self::NoLane(agent) => write!(f, "no working lane of this turn is {agent}'s"),
        }
    }
}
impl std::error::Error for OrchestrationError {}
impl From<TransitionError> for OrchestrationError {
    fn from(e: TransitionError) -> Self {
        Self::Turn(e)
    }
}
impl From<StoreError> for OrchestrationError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}
impl From<ToolError> for OrchestrationError {
    fn from(e: ToolError) -> Self {
        Self::Tool(e)
    }
}
impl From<PlanError> for OrchestrationError {
    fn from(e: PlanError) -> Self {
        Self::Plan(e)
    }
}
impl From<ParallelError> for OrchestrationError {
    fn from(e: ParallelError) -> Self {
        Self::Parallel(e)
    }
}

fn not_now() -> OrchestrationError {
    OrchestrationError::Turn(TransitionError::Rejected(TurnRejection::NotNow))
}

/// Where a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Deciding whether a plan is needed.
    Planning,
    /// A plan is with the person: reads only.
    AwaitingPlan,
    Building,
}

/// A lane at work, as Stop finds it.
#[derive(Debug)]
struct LiveLane {
    agent: AgentId,
    /// Which engine runs it (0 is the main one).
    engine: usize,
    /// A person pressed Stop this agent on it.
    halted: bool,
    ended: bool,
}

/// A turn with lanes at work.
#[derive(Debug)]
struct Run {
    spec: TurnSpec,
    lanes: Vec<LiveLane>,
}

/// Runs engine turns with plans, placeholders, leases and lanes.
pub struct Orchestrator<'a> {
    engine: &'a dyn AgentEngine,
    /// More engines for more lanes: lane `n` runs on engine `n` (the main engine is 0).
    helpers: Vec<&'a dyn AgentEngine>,
    /// What helper agents are called, in order (the host's agent hues).
    names: Vec<String>,
    registry: &'a Registry,
    log: &'a SessionLog,
    settings: PlanSettings,
    /// Turns with a run in progress.
    running: RefCell<BTreeMap<TurnId, Run>>,
    /// What a person pressed while the documents were busy with a run: applied before the next tool call.
    pending: RefCell<Vec<AgentControl>>,
    /// The latest time read from the documents, for a Stop that comes without them.
    now: Cell<Timestamp>,
}

impl<'a> Orchestrator<'a> {
    pub fn new(engine: &'a dyn AgentEngine, registry: &'a Registry, log: &'a SessionLog) -> Self {
        Self {
            engine,
            helpers: vec![],
            names: vec![],
            registry,
            log,
            settings: PlanSettings::default(),
            running: RefCell::default(),
            pending: RefCell::default(),
            now: Cell::new(Timestamp(0)),
        }
    }

    /// Plans with `settings` instead of the default threshold.
    #[must_use]
    pub fn with_settings(mut self, settings: PlanSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Engines for more lanes (one each: an engine runs one lane of a turn) and the names helper agents get, in
    /// order. Built-in agents are named by hue, so `names` come from the design's agent hues.
    #[must_use]
    pub fn with_helpers(mut self, engines: Vec<&'a dyn AgentEngine>, names: Vec<String>) -> Self {
        self.helpers = engines;
        self.names = names;
        self
    }

    fn engine_at(&self, n: usize) -> &'a dyn AgentEngine {
        if n == 0 {
            self.engine
        } else {
            self.helpers[n - 1]
        }
    }

    fn engines(&self) -> usize {
        1 + self.helpers.len()
    }

    fn clock(&self, docs: &Documents) -> Timestamp {
        let now = docs.now();
        self.now.set(now);
        now
    }

    fn person(spec: &TurnSpec) -> Origin {
        Origin::new(spec.person.clone())
            .in_session(spec.session.clone())
            .in_turn(spec.turn.clone())
    }

    fn origin_of(agent: &AgentId, spec: &TurnSpec) -> Origin {
        Origin::new(Actor::Agent {
            agent: agent.clone(),
        })
        .in_session(spec.session.clone())
        .in_turn(spec.turn.clone())
    }

    fn agent(session: &Session, spec: &TurnSpec) -> Result<(AgentId, Origin), OrchestrationError> {
        let agent = session
            .agent()
            .cloned()
            .ok_or(OrchestrationError::NoAgent)?;
        let origin = Self::origin_of(&agent, spec);
        Ok((agent, origin))
    }

    fn append(&self, origin: &Origin, body: EventBody, at: Timestamp) -> Result<(), StoreError> {
        let session = origin.session.clone().ok_or(StoreError::WrongSession)?;
        self.log.store().append(
            &session,
            NewEvent {
                at,
                origin: origin.clone(),
                body,
            },
        )?;
        Ok(())
    }

    fn events(&self, spec: &TurnSpec) -> Result<Vec<SessionEvent>, StoreError> {
        self.log.store().events(&spec.session, None)
    }

    /// The person's request: logs it, starts the turn and runs the engine until it builds, waits for Build, or
    /// ends. `request.turn` must be `spec.turn`.
    pub async fn start(
        &self,
        docs: &mut Documents,
        session: &mut Session,
        spec: &TurnSpec,
        request: TurnRequest,
    ) -> Result<TurnState, OrchestrationError> {
        let (agent, _) = Self::agent(session, spec)?;
        docs.name_agent(agent, session.agent_name());
        let at = self.clock(docs);
        let person = Self::person(spec);
        self.prompt(&person, &request, at)?;
        record_transition(self.log.store(), &person, TurnInput::Start, at)?;
        self.run(docs, session, spec, request, Phase::Planning, None)
            .await
    }

    /// Edit request: the person asks again while a plan waits. The engine plans again with the new words and the
    /// conversation so far; its new proposal replaces the old one.
    pub async fn edit_request(
        &self,
        docs: &mut Documents,
        session: &mut Session,
        spec: &TurnSpec,
        mut request: TurnRequest,
    ) -> Result<TurnState, OrchestrationError> {
        let at = self.clock(docs);
        let person = Self::person(spec);
        record_transition(self.log.store(), &person, TurnInput::EditRequest, at)?;
        self.prompt(&person, &request, at)?;
        request.history = history(self.log.store(), &self.events(spec)?)?;
        self.run(docs, session, spec, request, Phase::Planning, None)
            .await
    }

    fn prompt(
        &self,
        person: &Origin,
        request: &TurnRequest,
        at: Timestamp,
    ) -> Result<(), StoreError> {
        let images = request
            .images
            .iter()
            .map(|(_, data)| {
                let bytes = vellum_core::base64::decode(data)
                    .ok_or_else(|| StoreError::Corrupt("a picture is not base64".into()))?;
                self.log.store().put_image(&bytes)
            })
            .collect::<Result<_, _>>()?;
        self.append(
            person,
            EventBody::Prompt {
                text: request.prompt.clone(),
                context: vec![],
                images,
            },
            at,
        )
    }

    /// What Build starts from: the plan waiting (or `edited`, checked against the document), the agent that
    /// proposed it, and the document. Changes nothing.
    fn approve(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        edited: Option<Plan>,
    ) -> Result<(Plan, AgentId, Document), OrchestrationError> {
        let events = self.events(spec)?;
        let person = Self::person(spec);
        let state = turn_state(&events, &spec.turn).unwrap_or(TurnState::Queued);
        transition(state, TurnInput::Build, &person.actor)
            .map_err(|why| OrchestrationError::Turn(TransitionError::Rejected(why)))?;
        let waiting = plan_state(&events, &spec.turn)
            .filter(|plan| plan.status == PlanStatus::Proposed)
            .ok_or(OrchestrationError::NoPlan)?;
        // The agent that proposed it builds it.
        let agent = events
            .iter()
            .rev()
            .find_map(|event| match (&event.body, &event.origin.actor) {
                (EventBody::Plan(PlanEvent::Proposed(_)), Actor::Agent { agent })
                    if event.origin.turn.as_ref() == Some(&spec.turn) =>
                {
                    Some(agent.clone())
                }
                _ => None,
            })
            .ok_or(OrchestrationError::NoPlan)?;
        let doc = docs
            .target(&spec.document)
            .map_err(ToolError::from)?
            .snapshot()
            .map_err(ToolError::from)?;
        let plan = match edited {
            Some(plan) => {
                check(&plan)?;
                for item in &plan.items {
                    check_against(&doc, item)?;
                }
                plan
            }
            None => waiting.plan,
        };
        Ok((plan, agent, doc))
    }

    /// One placeholder per new frame of `plan`, held by `owner(item)`, each clear of the page's frames and of the
    /// ones placed before it. Reserves nothing yet.
    fn place(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        doc: &Document,
        plan: &Plan,
        owner: impl Fn(u32) -> AgentId,
    ) -> Result<Vec<Placeholder>, OrchestrationError> {
        let mut placeholders = vec![];
        let mut taken = vec![];
        for item in plan.items.iter().filter(|item| item.is_new()) {
            let size = (item.width.unwrap_or(0.0), item.height.unwrap_or(0.0));
            let (bounds, _) = free_space(
                docs,
                &spec.document,
                doc,
                item.page.as_str(),
                size,
                None,
                &taken,
            )?;
            taken.push(bounds);
            placeholders.push(Placeholder {
                item: item.id,
                frame: NodeId::new(docs.ulid()).expect("a ULID is an id"),
                name: item.name.clone(),
                page: item.page.clone(),
                bounds,
                agent: owner(item.id),
                turn: spec.turn.clone(),
            });
        }
        Ok(placeholders)
    }

    /// Logs Build with `plan` and its placeholders, and reserves them.
    fn built(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        plan: Plan,
        placeholders: &[Placeholder],
    ) -> Result<(), OrchestrationError> {
        let at = self.clock(docs);
        let person = Self::person(spec);
        record_transition(self.log.store(), &person, TurnInput::Build, at)?;
        self.append(
            &person,
            EventBody::Plan(PlanEvent::Built {
                plan,
                placeholders: placeholders.to_vec(),
            }),
            at,
        )?;
        docs.reserve(&spec.document, placeholders.to_vec());
        Ok(())
    }

    /// Build: the person approves the waiting plan, or `edited`, their version of it. Every new frame gets a
    /// placeholder, placed with `find_space` clear of the page's frames and every other placeholder. The turn
    /// moves to `running`; [`Self::run_build`] then builds. Nothing is written to the document.
    pub fn build(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        edited: Option<Plan>,
    ) -> Result<Vec<Placeholder>, OrchestrationError> {
        let (plan, agent, doc) = self.approve(docs, spec, edited)?;
        let placeholders = self.place(docs, spec, &doc, &plan, |_| agent.clone())?;
        self.built(docs, spec, plan, &placeholders)?;
        Ok(placeholders)
    }

    /// Runs the engine on the approved plan: its prompt lists every item and placeholder (replacing
    /// `request.prompt`), and its history is the session so far (replacing `request.history`).
    pub async fn run_build(
        &self,
        docs: &mut Documents,
        session: &mut Session,
        spec: &TurnSpec,
        mut request: TurnRequest,
    ) -> Result<TurnState, OrchestrationError> {
        let events = self.events(spec)?;
        let state = turn_state(&events, &spec.turn).unwrap_or(TurnState::Queued);
        let plan = plan_state(&events, &spec.turn)
            .filter(|plan| plan.status == PlanStatus::Building && state == TurnState::Running)
            .ok_or_else(not_now)?;
        request.prompt = build_prompt(&plan.plan, &plan.placeholders);
        request.history = history(self.log.store(), &events)?;
        self.run(docs, session, spec, request, Phase::Building, Some(plan))
            .await
    }

    /// Logs that the turn's plan is abandoned (if it has one still standing) and takes its placeholders away.
    fn abandon(
        &self,
        docs: Option<&mut Documents>,
        spec: &TurnSpec,
        origin: &Origin,
        at: Timestamp,
    ) -> Result<(), StoreError> {
        let standing = plan_state(&self.events(spec)?, &spec.turn)
            .is_some_and(|plan| matches!(plan.status, PlanStatus::Proposed | PlanStatus::Building));
        if standing {
            self.append(origin, EventBody::Plan(PlanEvent::Abandoned), at)?;
        }
        if let Some(docs) = docs {
            lanes::clear(docs, spec);
        }
        Ok(())
    }

    /// One engine run of the turn on the main engine, with any helpers it spawns.
    async fn run(
        &self,
        docs: &mut Documents,
        session: &mut Session,
        spec: &TurnSpec,
        request: TurnRequest,
        phase: Phase,
        plan: Option<PlanState>,
    ) -> Result<TurnState, OrchestrationError> {
        Self::agent(session, spec)?;
        let template = request.clone();
        let main = LaneRun {
            engine: 0,
            session: std::mem::take(session),
            request,
            phase,
            listed: false,
        };
        let (mut outcomes, planned) = self.run_lanes(docs, spec, vec![main], plan, template).await;
        let main = outcomes.remove(0);
        *session = main.session;
        if let Some(error) = main
            .error
            .into_iter()
            .chain(outcomes.iter_mut().filter_map(|o| o.error.take()))
            .next()
        {
            return Err(error);
        }
        let end = lanes::combine(
            std::iter::once((main.end, false)).chain(outcomes.iter().map(|o| (o.end, o.halted))),
        );
        let origin = Self::origin_of(&main.agent, spec);
        self.end_lanes(docs, spec, &outcomes)?;
        self.finish(docs, spec, &origin, main.phase, planned, end)
    }

    /// Moves the turn to where its lanes left it and clears what an ended plan leaves standing.
    fn finish(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        agent: &Origin,
        phase: Phase,
        planned: bool,
        end: TurnEnd,
    ) -> Result<TurnState, OrchestrationError> {
        let at = self.clock(docs);
        let store = self.log.store();
        let state = turn_state(&self.events(spec)?, &spec.turn).unwrap_or(TurnState::Queued);
        if state == TurnState::Stopped {
            self.abandon(Some(docs), spec, &Self::person(spec), at)?;
            return Ok(state);
        }
        if phase == Phase::AwaitingPlan {
            // The plan waits for the person whatever the run did after proposing it.
            return Ok(state);
        }
        if phase == Phase::Planning {
            record_transition(store, agent, TurnInput::NoPlanNeeded, at)?;
        }
        let (state, _) = match end {
            TurnEnd::Done => {
                let done = record_transition(store, agent, TurnInput::Done, at)?;
                if planned {
                    self.append(agent, EventBody::Plan(PlanEvent::Finished), at)?;
                }
                lanes::clear(docs, spec);
                done
            }
            // A run that ended without finishing (and was not stopped by a person) failed.
            TurnEnd::Failed(_) | TurnEnd::Stopped => {
                let failed = record_transition(store, agent, TurnInput::Error, at)?;
                self.abandon(Some(docs), spec, agent, at)?;
                failed
            }
        };
        Ok(state)
    }

    /// Logs how each listed lane ended, unless a person already did (Stop this agent).
    fn end_lanes(
        &self,
        docs: &Documents,
        spec: &TurnSpec,
        outcomes: &[LaneOutcome],
    ) -> Result<(), StoreError> {
        let at = self.clock(docs);
        let stopped = turn_state(&self.events(spec)?, &spec.turn) == Some(TurnState::Stopped);
        for outcome in outcomes.iter().filter(|o| o.listed && !o.halted) {
            let (end, origin) = if stopped {
                (vellum_core::parallel::LaneEnd::Stopped, Self::person(spec))
            } else {
                (
                    lanes::lane_end(outcome.end),
                    Self::origin_of(&outcome.agent, spec),
                )
            };
            self.append(
                &origin,
                EventBody::Parallel(vellum_core::parallel::ParallelEvent::LaneEnded {
                    agent: outcome.agent.clone(),
                    end,
                }),
                at,
            )?;
        }
        Ok(())
    }
}

/// `item` names a page and (when it changes one) a frame that `doc` has.
fn check_against(doc: &Document, item: &PlanItem) -> Result<(), ToolError> {
    if !doc.pages.iter().any(|page| page.id == item.page) {
        return Err(no_page(doc, item.page.as_str()));
    }
    if let Some(frame) = &item.frame {
        match ops::find(doc, frame) {
            Some(Node::Frame(_)) => {}
            Some(_) => {
                return Err(ToolError::invalid(
                    "frame",
                    format!("{frame} is not a frame; plan items are frames."),
                ));
            }
            None => {
                return Err(ToolError::not_found(format!("There is no frame {frame}.")));
            }
        }
    }
    Ok(())
}

fn no_page(doc: &Document, named: &str) -> ToolError {
    ToolError::not_found(format!(
        "There is no page {named:?}. Plans place frames on existing pages: {}.",
        doc.pages
            .iter()
            .map(|p| format!("{} ({})", p.name, p.id))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProposeInput {
    /// A short title: what the plan makes ("Onboarding · 4 screens").
    title: String,
    /// The frames to create or change, in the order you will build them.
    items: Vec<ProposedItem>,
    /// About how many changes the work takes, when you can tell.
    changes: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProposedItem {
    /// The frame's name.
    name: String,
    /// The page it goes on, by id or by name.
    page: String,
    /// What it is for, in one line.
    intent: String,
    /// The width of a new frame.
    width: Option<f64>,
    /// The height of a new frame.
    height: Option<f64>,
    /// The id of the existing frame this item changes. Leave it out for a new frame.
    frame: Option<String>,
}

/// The plan an engine proposed, with pages resolved against the document.
fn resolve(doc: &Document, input: ProposeInput) -> Result<Plan, ToolError> {
    let mut items = vec![];
    for (n, proposed) in input.items.into_iter().enumerate() {
        let at = |field: &str| format!("items[{n}].{field}");
        let mut named = doc
            .pages
            .iter()
            .filter(|p| p.id.as_str() == proposed.page || p.name == proposed.page);
        let page = match (named.next(), named.next()) {
            (Some(page), None) => page.id.clone(),
            (Some(_), Some(_)) => {
                return Err(ToolError::invalid(
                    at("page"),
                    format!(
                        "More than one page is called {:?}; name it by id.",
                        proposed.page
                    ),
                ));
            }
            (None, _) => return Err(no_page(doc, &proposed.page).at_path(at("page"))),
        };
        let frame = proposed
            .frame
            .map(|frame| {
                NodeId::new(frame.as_str()).map_err(|e| {
                    ToolError::invalid(at("frame"), format!("{frame:?} is not a layer id: {e}."))
                })
            })
            .transpose()?;
        let item = PlanItem {
            id: n as u32 + 1,
            name: proposed.name,
            page,
            intent: proposed.intent,
            width: proposed.width,
            height: proposed.height,
            frame,
        };
        check_against(doc, &item).map_err(|e| e.at_path(at("frame")))?;
        items.push(item);
    }
    let plan = Plan {
        title: input.title,
        items,
        changes: input.changes,
    };
    check(&plan).map_err(|e| ToolError::invalid("items", e.to_string()))?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::schema_of;
    use serde_json::Value;

    #[test]
    fn propose_plan_is_offered_with_a_strict_schema() {
        let schema = serde_json::to_value(schema_of::<ProposeInput>()).unwrap();
        assert_eq!(schema["additionalProperties"], false);
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(required, ["title", "items"]);
        assert!(PROPOSE_DESCRIPTION.contains("20 changes"));
    }
}
