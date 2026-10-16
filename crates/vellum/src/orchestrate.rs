//! Orchestrating an engine turn: planning, plans, placeholders and leases (ARCHITECTURE §7.5; guide §10.1,
//! Prompt 38).
//!
//! A turn starts in `planning`. The engine is offered one more tool than MCP clients get, `propose_plan`. A plan
//! over the threshold ([`PlanSettings`]) moves the turn to `awaiting_plan`: it is logged, the engine is told to end
//! its run, and any write it still tries is refused. A plan under the threshold, or a first write without a plan,
//! means no plan is needed and the run goes on building. The person then presses Build (with the plan as they
//! edited it), asks again (Edit request, a new planning run) or stops.
//!
//! Build places a placeholder for every new frame: an id reserved for the frame, held by the building agent, with
//! planned bounds from `find_space` that keep clear of the page's frames and of every other placeholder. They live
//! in the daemon's documents (session state, never the file), so every agent sees them through `get_app_state`
//! and `find_space` keeps away from them. When the agent creates a frame with a reserved id, the frame is leased to
//! it; when it reports the item done (`report_progress` with `items`), the item ticks off and its placeholder goes.
//! Stopped, failed or finished, the turn's placeholders go with it, and nothing of an abandoned plan is in the file.
//!
//! The orchestrator is single-threaded like the daemon: its futures borrow the documents. A person's Stop may come
//! while a run is in progress (between polls); the run then ends and clears up.
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fmt;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::guard::LeaseChange;
use vellum_core::ops;
use vellum_core::plans::{
    Placeholder, Plan, PlanError, PlanEvent, PlanItem, PlanSettings, PlanState, PlanStatus,
    build_prompt, check, plan_state,
};
use vellum_core::ports::{
    AgentEngine, BridgeTool, EngineError, EventBody, ModelTool, NewEvent, SessionEvent, StoreError,
    Timestamp, ToolBridge, ToolReply, TurnEnd, TurnEvent, TurnRequest,
};
use vellum_core::sessions::{TransitionError, history, record_transition, turn_state};
use vellum_core::turns::{TurnInput, TurnRejection, TurnState, transition};
use vellum_types::{Actor, AgentId, Document, Node, NodeId, Origin, SessionId, TurnId};

use crate::daemon::Documents;
use crate::sessions::SessionLog;
use crate::tools::{
    ErrorKind, Registry, Session, ToolCtx, ToolError, free_space, parse_input, schema_of,
};

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

/// Where a run is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Deciding whether a plan is needed.
    Planning,
    /// A plan is with the person: reads only.
    AwaitingPlan,
    Building,
}

/// Runs engine turns with plans, placeholders and leases.
pub struct Orchestrator<'a> {
    engine: &'a dyn AgentEngine,
    registry: &'a Registry,
    log: &'a SessionLog,
    settings: PlanSettings,
    /// Turns with a run in progress.
    running: RefCell<BTreeSet<TurnId>>,
    /// The latest time read from the documents, for a Stop that comes without them.
    now: Cell<Timestamp>,
}

impl<'a> Orchestrator<'a> {
    pub fn new(engine: &'a dyn AgentEngine, registry: &'a Registry, log: &'a SessionLog) -> Self {
        Self {
            engine,
            registry,
            log,
            settings: PlanSettings::default(),
            running: RefCell::default(),
            now: Cell::new(Timestamp(0)),
        }
    }

    /// Plans with `settings` instead of the default threshold.
    #[must_use]
    pub fn with_settings(mut self, settings: PlanSettings) -> Self {
        self.settings = settings;
        self
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

    fn agent(session: &Session, spec: &TurnSpec) -> Result<(AgentId, Origin), OrchestrationError> {
        let agent = session
            .agent()
            .cloned()
            .ok_or(OrchestrationError::NoAgent)?;
        let origin = Origin::new(Actor::Agent {
            agent: agent.clone(),
        })
        .in_session(spec.session.clone())
        .in_turn(spec.turn.clone());
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

    /// Build: the person approves the waiting plan, or `edited`, their version of it. Every new frame gets a
    /// placeholder, placed with `find_space` clear of the page's frames and every other placeholder. The turn
    /// moves to `running`; [`Self::run_build`] then builds. Nothing is written to the document.
    pub fn build(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        edited: Option<Plan>,
    ) -> Result<Vec<Placeholder>, OrchestrationError> {
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
        let mut placeholders = vec![];
        let mut taken = vec![];
        for item in plan.items.iter().filter(|item| item.is_new()) {
            let size = (item.width.unwrap_or(0.0), item.height.unwrap_or(0.0));
            let (bounds, _) = free_space(
                docs,
                &spec.document,
                &doc,
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
                agent: agent.clone(),
                turn: spec.turn.clone(),
            });
        }
        let at = self.clock(docs);
        record_transition(self.log.store(), &person, TurnInput::Build, at)?;
        self.append(
            &person,
            EventBody::Plan(PlanEvent::Built {
                plan,
                placeholders: placeholders.clone(),
            }),
            at,
        )?;
        docs.reserve(&spec.document, placeholders.clone());
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
            .ok_or(OrchestrationError::Turn(TransitionError::Rejected(
                TurnRejection::NotNow,
            )))?;
        request.prompt = build_prompt(&plan.plan, &plan.placeholders);
        request.history = history(self.log.store(), &events)?;
        self.run(docs, session, spec, request, Phase::Building, Some(plan))
            .await
    }

    /// Stop, pressed by the person. The turn is stopped and its engine interrupted. A plan is abandoned: when a
    /// run is in progress it clears the placeholders as it ends; otherwise pass `docs` and they go now.
    pub fn stop(
        &self,
        docs: Option<&mut Documents>,
        spec: &TurnSpec,
    ) -> Result<TurnState, OrchestrationError> {
        let at = docs
            .as_deref()
            .map_or(self.now.get(), |docs| self.clock(docs));
        let person = Self::person(spec);
        let (state, _) = record_transition(self.log.store(), &person, TurnInput::Stop, at)?;
        self.engine.interrupt(&spec.turn);
        if !self.running.borrow().contains(&spec.turn) {
            self.abandon(docs, spec, &person, at)?;
        }
        Ok(state)
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
            clear(docs, spec);
        }
        Ok(())
    }

    async fn run(
        &self,
        docs: &mut Documents,
        session: &mut Session,
        spec: &TurnSpec,
        request: TurnRequest,
        phase: Phase,
        plan: Option<PlanState>,
    ) -> Result<TurnState, OrchestrationError> {
        let (agent, origin) = Self::agent(session, spec)?;
        session.set_turn(Some(spec.turn.clone()));
        let model = request.model.clone();
        let _running = Running::new(&self.running, spec.turn.clone());
        let bridge = PlanBridge {
            orchestrator: self,
            spec,
            agent: agent.clone(),
            origin: origin.clone(),
            inner: RefCell::new(Inner {
                docs,
                session,
                phase,
                plan,
                error: None,
            }),
        };
        let mut end = TurnEnd::Done;
        {
            let mut stream = self
                .engine
                .run_turn(request, &bridge)
                .map_err(OrchestrationError::Engine)?;
            while let Some(event) = stream.next().await {
                let at = self.clock(bridge.inner.borrow().docs);
                match &event {
                    TurnEvent::Usage(_) | TurnEvent::Messages(_) => {
                        self.log.record_turn_event(&origin, &model, &event, at)?;
                    }
                    TurnEvent::Ended(how) => end = *how,
                    _ => {}
                }
            }
        }
        let Inner {
            docs,
            session,
            phase,
            plan,
            error,
        } = bridge.inner.into_inner();
        session.set_turn(None);
        self.log.record_steps(docs, session)?;
        if let Some(error) = error {
            return Err(error);
        }
        self.finish(docs, spec, &origin, phase, plan.is_some(), end)
    }

    /// Moves the turn to where the run left it and clears what an ended plan leaves standing.
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
                clear(docs, spec);
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
}

/// Removes the turn's placeholders and lets go of the frames it was filling.
fn clear(docs: &mut Documents, spec: &TurnSpec) {
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

/// Marks a turn as running while a run is in progress, however the future ends.
struct Running<'r> {
    set: &'r RefCell<BTreeSet<TurnId>>,
    turn: TurnId,
}
impl<'r> Running<'r> {
    fn new(set: &'r RefCell<BTreeSet<TurnId>>, turn: TurnId) -> Self {
        set.borrow_mut().insert(turn.clone());
        Self { set, turn }
    }
}
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.set.borrow_mut().remove(&self.turn);
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

struct Inner<'d> {
    docs: &'d mut Documents,
    session: &'d mut Session,
    phase: Phase,
    plan: Option<PlanState>,
    /// The first failure to log something; the run reports it when it ends.
    error: Option<OrchestrationError>,
}

/// The registry as a planning engine sees it: `propose_plan` added, writes held back while a plan waits, ticks
/// and placeholders kept up to date.
struct PlanBridge<'o, 'd> {
    orchestrator: &'o Orchestrator<'o>,
    spec: &'d TurnSpec,
    agent: AgentId,
    origin: Origin,
    inner: RefCell<Inner<'d>>,
}

fn failed(error: ToolError) -> ToolReply {
    ToolReply {
        value: error.to_json(),
        failed: true,
    }
}

impl PlanBridge<'_, '_> {
    fn log(
        &self,
        inner: &mut Inner<'_>,
        write: impl FnOnce(Timestamp) -> Result<(), OrchestrationError>,
    ) -> bool {
        let at = self.orchestrator.clock(inner.docs);
        match write(at) {
            Ok(()) => true,
            Err(error) => {
                inner.error.get_or_insert(error);
                false
            }
        }
    }

    fn transition(&self, inner: &mut Inner<'_>, input: TurnInput) -> bool {
        let store = self.orchestrator.log.store();
        self.log(inner, |at| {
            record_transition(store, &self.origin, input, at)
                .map(|_| ())
                .map_err(OrchestrationError::from)
        })
    }

    fn propose(&self, inner: &mut Inner<'_>, input: Value) -> ToolReply {
        match inner.phase {
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
        let doc = match inner
            .docs
            .target(&self.spec.document)
            .and_then(|target| target.snapshot())
        {
            Ok(doc) => doc,
            Err(error) => return failed(error.into()),
        };
        let plan = match resolve(&doc, input) {
            Ok(plan) => plan,
            Err(error) => return failed(error),
        };
        if !self.orchestrator.settings.needs_plan(&plan) {
            if !self.transition(inner, TurnInput::NoPlanNeeded) {
                return failed(ToolError::new(
                    ErrorKind::Internal,
                    "The turn could not move on.",
                ));
            }
            inner.phase = Phase::Building;
            return ToolReply {
                value: json!({"plan": "not_needed", "message": "This is small enough to build without a plan. Build it now."}),
                failed: false,
            };
        }
        let items = plan.items.len();
        let logged = self.transition(inner, TurnInput::PlanNeeded)
            && self.log(inner, |at| {
                self.orchestrator
                    .append(
                        &self.origin,
                        EventBody::Plan(PlanEvent::Proposed(plan.clone())),
                        at,
                    )
                    .map_err(OrchestrationError::from)
            });
        if !logged {
            return failed(ToolError::new(
                ErrorKind::Internal,
                "The plan could not be recorded.",
            ));
        }
        inner.phase = Phase::AwaitingPlan;
        inner.plan = Some(PlanState::proposed(plan));
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

    /// Placeholders whose frame now exists are being filled: the frame is leased to the agent building it.
    fn fill(&self, inner: &mut Inner<'_>) {
        let path = &self.spec.document;
        let waiting: Vec<NodeId> = inner
            .docs
            .placeholders(path)
            .iter()
            .filter(|p| !p.filling && p.placeholder.turn == self.spec.turn)
            .map(|p| p.placeholder.frame.clone())
            .collect();
        if waiting.is_empty() {
            return;
        }
        let Ok(doc) = inner.docs.target(path).and_then(|t| t.snapshot()) else {
            return;
        };
        for frame in waiting {
            if !matches!(ops::find(&doc, &frame), Some(Node::Frame(_))) {
                continue;
            }
            let claimed = inner
                .docs
                .target(path)
                .and_then(|t| t.lease(&frame, &self.agent, LeaseChange::Claim));
            if claimed.is_ok() {
                inner.docs.filling(path, &frame);
            }
        }
    }

    /// Checks a progress report's `items` against the plan before the tool runs.
    fn ticks(&self, inner: &Inner<'_>, input: &Value) -> Result<Option<Vec<u32>>, ToolError> {
        let Some(items) = input.get("items").and_then(Value::as_array) else {
            return Ok(None);
        };
        let items: Vec<u32> = items
            .iter()
            .filter_map(|n| n.as_u64().and_then(|n| u32::try_from(n).ok()))
            .collect();
        let Some(plan) = &inner.plan else {
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

impl ToolBridge for PlanBridge<'_, '_> {
    fn tools(&self) -> Vec<BridgeTool> {
        let mut tools: Vec<BridgeTool> = self
            .orchestrator
            .registry
            .tools()
            .map(|tool| BridgeTool {
                tool: ModelTool {
                    name: tool.name().to_owned(),
                    description: tool.description().to_owned(),
                    parameters: serde_json::to_value(tool.input_schema())
                        .expect("a schema is JSON"),
                },
                read_only: tool.read_only(),
            })
            .collect();
        tools.push(BridgeTool {
            tool: ModelTool {
                name: PROPOSE_PLAN.to_owned(),
                description: PROPOSE_DESCRIPTION.to_owned(),
                parameters: serde_json::to_value(schema_of::<ProposeInput>())
                    .expect("a schema is JSON"),
            },
            read_only: false,
        });
        tools
    }

    fn call(&self, name: &str, input: Value) -> ToolReply {
        let mut guard = self.inner.borrow_mut();
        let inner = &mut *guard;
        if name == PROPOSE_PLAN {
            return self.propose(inner, input);
        }
        let writes = self
            .orchestrator
            .registry
            .get(name)
            .is_some_and(|tool| !tool.read_only());
        match inner.phase {
            Phase::AwaitingPlan if writes => {
                return failed(ToolError::new(
                    ErrorKind::Refused,
                    "The plan is waiting for the person. End your turn now; nothing is built until they press \
                     Build.",
                ));
            }
            Phase::Planning if writes => {
                // Writing without a plan: the engine judged the request small.
                if !self.transition(inner, TurnInput::NoPlanNeeded) {
                    return failed(ToolError::new(
                        ErrorKind::Internal,
                        "The turn could not move on.",
                    ));
                }
                inner.phase = Phase::Building;
            }
            _ => {}
        }
        let ticks = if name == "report_progress" {
            match self.ticks(inner, &input) {
                Ok(ticks) => ticks,
                Err(error) => return failed(error),
            }
        } else {
            None
        };
        let result = {
            let mut ctx = ToolCtx::new(inner.docs, inner.session);
            self.orchestrator.registry.call(&mut ctx, name, input)
        };
        let mut reply = match result {
            Ok(value) => ToolReply {
                value,
                failed: false,
            },
            Err(error) => failed(error),
        };
        if !reply.failed {
            if let (Some(items), Some(plan)) = (ticks, inner.plan.as_mut()) {
                let new = plan.tick(&items).unwrap_or_default();
                let (done, total) = plan.progress();
                if !new.is_empty() {
                    inner.docs.unreserve(&self.spec.document, |p| {
                        p.turn == self.spec.turn && new.contains(&p.item)
                    });
                    self.log(inner, |at| {
                        self.orchestrator
                            .append(
                                &self.origin,
                                EventBody::Plan(PlanEvent::Ticked(new.clone())),
                                at,
                            )
                            .map_err(OrchestrationError::from)
                    });
                }
                reply.value["plan"] = json!({"done": done, "total": total});
            }
            if writes {
                self.fill(inner);
            }
        }
        if let Err(error) = self
            .orchestrator
            .log
            .record_steps(inner.docs, inner.session)
        {
            inner.error.get_or_insert(error.into());
        }
        reply
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
