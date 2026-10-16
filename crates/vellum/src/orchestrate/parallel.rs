//! Split, Side by side, Explore, and helpers an external client asked for (ARCHITECTURE §7.5; DESIGN.md C05, C07).
use vellum_core::guard::LeaseChange;
use vellum_core::ops;
use vellum_core::parallel::{
    Axis, LaneEnd, LaneRecord, Mode, OptionRecord, ParallelEvent, check_lanes, check_variants,
    explore_first_prompt, explore_prompt, helper_prompt, option_label, parallel_state,
    side_by_side_prompt, split, split_prompt,
};
use vellum_core::plans::{Placeholder, Plan, PlanStatus, plan_state};
use vellum_core::ports::{EventBody, TurnEnd, TurnRequest};
use vellum_core::sessions::{history, record_transition, turn_state};
use vellum_core::turns::{TurnInput, TurnState};
use vellum_types::{Actor, AgentId, Document, Node, NodeId, Origin, PageId, Size, Value};

use super::lanes::{LaneOutcome, LaneRun, clear};
use super::{OrchestrationError, Orchestrator, Phase, TurnSpec, no_page, not_now};
use crate::daemon::{Documents, SpawnRequest};
use crate::tools::{Session, ToolError, free_space};

/// One lane the person set up: its agent, what it is called (its hue), and its model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneSpec {
    pub agent: AgentId,
    pub name: String,
    pub model: String,
}

/// Where Side by side and Explore put each result: a page, and the size the request implies.
#[derive(Debug, Clone, PartialEq)]
pub struct Area {
    pub page: PageId,
    pub width: f64,
    pub height: f64,
}

/// How one lane ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneResult {
    pub agent: AgentId,
    pub name: String,
    pub end: LaneEnd,
}

/// How a turn with lanes ended: its state, each lane, and the results to choose between.
#[derive(Debug, Clone, PartialEq)]
pub struct ParallelOutcome {
    pub state: TurnState,
    pub lanes: Vec<LaneResult>,
    pub options: Vec<OptionRecord>,
}

impl Orchestrator<'_> {
    fn check_engines(&self, lanes: usize) -> Result<(), OrchestrationError> {
        check_lanes(lanes)?;
        if lanes > self.engines() {
            return Err(OrchestrationError::Engines {
                needed: lanes,
                have: self.engines(),
            });
        }
        Ok(())
    }

    fn snapshot(docs: &mut Documents, spec: &TurnSpec) -> Result<Document, OrchestrationError> {
        Ok(docs
            .target(&spec.document)
            .and_then(|t| t.snapshot())
            .map_err(ToolError::from)?)
    }

    fn lane_session(spec: &TurnSpec, agent: &AgentId, name: &str) -> Session {
        Session::for_agent(name, agent.clone(), spec.session.clone())
    }

    /// Build with Split: the plan's items are divided into `lanes.len()` lanes of about equal size, in plan
    /// order (fewer when the plan has fewer items), and each new frame's placeholder is held by the agent of its
    /// lane. Then [`Self::run_split`] builds.
    pub fn build_split(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        edited: Option<Plan>,
        lanes: &[LaneSpec],
    ) -> Result<Vec<Placeholder>, OrchestrationError> {
        self.check_engines(lanes.len())?;
        let (plan, _, doc) = self.approve(docs, spec, edited)?;
        let ids: Vec<u32> = plan.items.iter().map(|item| item.id).collect();
        let shares = split(&ids, lanes.len());
        let owner = |item: u32| {
            let share = shares
                .iter()
                .position(|share| share.contains(&item))
                .expect("every item is in a share");
            lanes[share].agent.clone()
        };
        let placeholders = self.place(docs, spec, &doc, &plan, owner)?;
        self.built(docs, spec, plan, &placeholders)?;
        let records = lanes
            .iter()
            .zip(&shares)
            .map(|(lane, items)| LaneRecord {
                agent: lane.agent.clone(),
                name: lane.name.clone(),
                model: lane.model.clone(),
                items: items.clone(),
                task: None,
            })
            .collect();
        let at = self.clock(docs);
        self.append(
            &Self::person(spec),
            EventBody::Parallel(ParallelEvent::Lanes {
                mode: Mode::Split,
                lanes: records,
            }),
            at,
        )?;
        for lane in lanes {
            docs.name_agent(lane.agent.clone(), lane.name.clone());
        }
        Ok(placeholders)
    }

    /// Builds a split plan: every lane at once, each told only its items, on its own engine and model. `template`
    /// gives the requests their skills, limits and effort.
    pub async fn run_split(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        template: TurnRequest,
    ) -> Result<ParallelOutcome, OrchestrationError> {
        let events = self.events(spec)?;
        let state = turn_state(&events, &spec.turn).unwrap_or(TurnState::Queued);
        let plan = plan_state(&events, &spec.turn)
            .filter(|plan| plan.status == PlanStatus::Building && state == TurnState::Running)
            .ok_or_else(not_now)?;
        let lanes = parallel_state(&events, &spec.turn)
            .filter(|lanes| lanes.mode == Mode::Split)
            .ok_or_else(not_now)?
            .lanes;
        let past = history(self.log.store(), &events)?;
        let runs = lanes
            .iter()
            .enumerate()
            .map(|(n, lane)| {
                let record = &lane.record;
                let mut request = template.clone();
                request.turn = spec.turn.clone();
                request.model = record.model.clone();
                request.prompt =
                    split_prompt(&plan.plan, &record.items, &plan.placeholders, lanes.len());
                request.history = past.clone();
                LaneRun {
                    engine: n,
                    session: Self::lane_session(spec, &record.agent, &record.name),
                    request,
                    phase: Phase::Building,
                    listed: true,
                }
            })
            .collect();
        let (outcomes, planned) = self.run_lanes(docs, spec, runs, Some(plan), template).await;
        self.finish_lanes(docs, spec, outcomes, planned, vec![])
    }

    /// The person's request with Parallel Side by side: Start, then every lane builds the whole request in its
    /// own area of `area`'s size, as option A, B, ….
    pub async fn side_by_side(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        request: TurnRequest,
        lanes: &[LaneSpec],
        area: Area,
    ) -> Result<ParallelOutcome, OrchestrationError> {
        self.check_engines(lanes.len())?;
        let doc = Self::snapshot(docs, spec)?;
        if !doc.pages.iter().any(|page| page.id == area.page) {
            return Err(no_page(&doc, area.page.as_str()).into());
        }
        self.begin(docs, spec, &request)?;
        let mut placeholders = vec![];
        let mut taken = vec![];
        for (n, lane) in lanes.iter().enumerate() {
            let option = n as u32 + 1;
            let (bounds, _) = free_space(
                docs,
                &spec.document,
                &doc,
                area.page.as_str(),
                (area.width, area.height),
                None,
                &taken,
            )?;
            taken.push(bounds);
            placeholders.push(Placeholder {
                item: option,
                frame: NodeId::new(docs.ulid()).expect("a ULID is an id"),
                name: option_label(option),
                page: area.page.clone(),
                bounds,
                agent: lane.agent.clone(),
                turn: spec.turn.clone(),
            });
        }
        docs.reserve(&spec.document, placeholders.clone());
        let records = lanes
            .iter()
            .map(|lane| LaneRecord {
                agent: lane.agent.clone(),
                name: lane.name.clone(),
                model: lane.model.clone(),
                items: vec![],
                task: None,
            })
            .collect();
        let at = self.clock(docs);
        self.append(
            &Self::person(spec),
            EventBody::Parallel(ParallelEvent::Lanes {
                mode: Mode::SideBySide,
                lanes: records,
            }),
            at,
        )?;
        let runs = lanes
            .iter()
            .zip(&placeholders)
            .enumerate()
            .map(|(n, (lane, placeholder))| {
                let mut lane_request = request.clone();
                lane_request.model = lane.model.clone();
                lane_request.prompt = side_by_side_prompt(
                    &request.prompt,
                    placeholder.item,
                    lanes.len(),
                    placeholder,
                );
                LaneRun {
                    engine: n,
                    session: Self::lane_session(spec, &lane.agent, &lane.name),
                    request: lane_request,
                    phase: Phase::Building,
                    listed: true,
                }
            })
            .collect();
        let (outcomes, _) = self.run_lanes(docs, spec, runs, None, request).await;
        // Every lane's frame is an option, stopped or not: what was built stays.
        let doc = Self::snapshot(docs, spec)?;
        let options: Vec<OptionRecord> = placeholders
            .iter()
            .filter(|p| matches!(ops::find(&doc, &p.frame), Some(Node::Frame(_))))
            .map(|p| OptionRecord {
                option: p.item,
                agent: p.agent.clone(),
                frame: p.frame.clone(),
                label: outcomes
                    .iter()
                    .find(|o| o.agent == p.agent)
                    .and_then(|o| o.report.clone()),
            })
            .collect();
        let at = self.clock(docs);
        for option in &options {
            self.append(
                &Self::origin_of(&option.agent, spec),
                EventBody::Parallel(ParallelEvent::Option(option.clone())),
                at,
            )?;
        }
        self.finish_lanes(docs, spec, outcomes, false, options)
    }

    /// The person's request with Explore: Start, then the first result, then `count − 1` successive variants by
    /// the same agent, each a copy of the first that changes only `axis`. A variant waits for the one before.
    #[allow(clippy::too_many_arguments)]
    pub async fn explore(
        &self,
        docs: &mut Documents,
        session: &mut Session,
        spec: &TurnSpec,
        request: TurnRequest,
        axis: Axis,
        count: usize,
        area: Area,
    ) -> Result<ParallelOutcome, OrchestrationError> {
        check_variants(count)?;
        let (agent, origin) = Self::agent(session, spec)?;
        let name = session.agent_name();
        let doc = Self::snapshot(docs, spec)?;
        if !doc.pages.iter().any(|page| page.id == area.page) {
            return Err(no_page(&doc, area.page.as_str()).into());
        }
        self.begin(docs, spec, &request)?;
        let at = self.clock(docs);
        self.append(
            &Self::person(spec),
            EventBody::Parallel(ParallelEvent::Lanes {
                mode: Mode::Explore(axis),
                lanes: vec![LaneRecord {
                    agent: agent.clone(),
                    name: name.clone(),
                    model: request.model.clone(),
                    items: vec![],
                    task: None,
                }],
            }),
            at,
        )?;
        let mut options: Vec<OptionRecord> = vec![];
        let mut first: Option<(NodeId, (f64, f64))> = None;
        let mut end = TurnEnd::Done;
        for n in 1..=count {
            let doc = Self::snapshot(docs, spec)?;
            let size = first.as_ref().map_or((area.width, area.height), |f| f.1);
            let (bounds, _) = free_space(
                docs,
                &spec.document,
                &doc,
                area.page.as_str(),
                size,
                None,
                &[],
            )?;
            let option = n as u32;
            let placeholder = Placeholder {
                item: option,
                frame: NodeId::new(docs.ulid()).expect("a ULID is an id"),
                name: option_label(option),
                page: area.page.clone(),
                bounds,
                agent: agent.clone(),
                turn: spec.turn.clone(),
            };
            docs.reserve(&spec.document, vec![placeholder.clone()]);
            let mut run = request.clone();
            run.prompt = match &first {
                None => explore_first_prompt(&request.prompt, count, &placeholder),
                Some((frame, _)) => {
                    run.images = vec![];
                    run.history = history(self.log.store(), &self.events(spec)?)?;
                    explore_prompt(&request.prompt, axis, n, count, frame, &placeholder)
                }
            };
            let main = LaneRun {
                engine: 0,
                session: std::mem::take(session),
                request: run,
                phase: Phase::Building,
                listed: false,
            };
            let (mut outcomes, _) = self
                .run_lanes(docs, spec, vec![main], None, request.clone())
                .await;
            let outcome = outcomes.remove(0);
            *session = outcome.session;
            if let Some(error) = outcome.error {
                return Err(error);
            }
            let stopped = turn_state(&self.events(spec)?, &spec.turn) == Some(TurnState::Stopped);
            let doc = Self::snapshot(docs, spec)?;
            let built = frame_size(&doc, &placeholder.frame);
            if stopped || outcome.end != TurnEnd::Done || built.is_none() {
                end = match outcome.end {
                    // Done without its frame: the variant was not built. Like a run that ended early, that fails
                    // the turn (unless a person stopped it).
                    TurnEnd::Done => TurnEnd::Stopped,
                    other => other,
                };
                break;
            }
            // The variant is a frame now: its placeholder and lease go.
            docs.unreserve(&spec.document, |p| p.frame == placeholder.frame);
            if let Ok(target) = docs.target(&spec.document) {
                let _ = target.lease(&placeholder.frame, &agent, LeaseChange::Release);
            }
            let record = OptionRecord {
                option,
                agent: agent.clone(),
                frame: placeholder.frame.clone(),
                label: outcome.report.filter(|_| first.is_some()),
            };
            let at = self.clock(docs);
            self.append(
                &origin,
                EventBody::Parallel(ParallelEvent::Option(record.clone())),
                at,
            )?;
            options.push(record);
            first.get_or_insert((placeholder.frame.clone(), built.expect("checked")));
        }
        let outcome = LaneOutcome::summary(agent, name, Session::default(), end);
        self.finish_lanes(docs, spec, vec![outcome], false, options)
    }

    /// Helpers an external client asked for (`spawn_agents`), started by the host as lanes of the engine session
    /// and turn in `spec`. The client is who started the turn.
    pub async fn run_spawned(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        request: SpawnRequest,
        template: TurnRequest,
    ) -> Result<ParallelOutcome, OrchestrationError> {
        if request.document != spec.document {
            return Err(ToolError::invalid(
                "document",
                format!("The helpers were asked for on {}.", request.document),
            )
            .into());
        }
        let engines = self.free_engines();
        let names: Vec<String> = self
            .names
            .iter()
            .filter(|name| **name != request.by_name)
            .cloned()
            .collect();
        let lanes = request.tasks.len();
        check_lanes(lanes)?;
        let have = engines.len().min(names.len());
        if lanes > have {
            return Err(OrchestrationError::Engines {
                needed: lanes,
                have,
            });
        }
        let by = Origin::new(Actor::Agent {
            agent: request.by.clone(),
        })
        .in_session(spec.session.clone())
        .in_turn(spec.turn.clone());
        let at = self.clock(docs);
        record_transition(self.log.store(), &by, TurnInput::Start, at)?;
        record_transition(self.log.store(), &by, TurnInput::NoPlanNeeded, at)?;
        let mut records = vec![];
        let mut runs = vec![];
        for ((task, engine), name) in request.tasks.iter().zip(engines).zip(names) {
            let agent = AgentId::new(docs.ulid()).expect("a ULID is an id");
            let model = task.model.clone().unwrap_or_else(|| template.model.clone());
            let mut lane = template.clone();
            lane.turn = spec.turn.clone();
            lane.model = model.clone();
            lane.prompt = helper_prompt(&task.task, &request.by_name);
            records.push(LaneRecord {
                agent: agent.clone(),
                name: name.clone(),
                model,
                items: vec![],
                task: Some(task.task.clone()),
            });
            runs.push(LaneRun {
                engine,
                session: Self::lane_session(spec, &agent, &name),
                request: lane,
                phase: Phase::Building,
                listed: true,
            });
        }
        self.append(
            &by,
            EventBody::Parallel(ParallelEvent::Lanes {
                mode: Mode::Spawned,
                lanes: records,
            }),
            at,
        )?;
        let (outcomes, _) = self.run_lanes(docs, spec, runs, None, template).await;
        self.finish_lanes(docs, spec, outcomes, false, vec![])
    }

    /// Logs the person's request and starts the turn without a plan: Side by side and Explore build at once.
    fn begin(
        &self,
        docs: &Documents,
        spec: &TurnSpec,
        request: &TurnRequest,
    ) -> Result<(), OrchestrationError> {
        let at = self.clock(docs);
        let person = Self::person(spec);
        self.prompt(&person, request, at)?;
        record_transition(self.log.store(), &person, TurnInput::Start, at)?;
        record_transition(self.log.store(), &person, TurnInput::NoPlanNeeded, at)?;
        Ok(())
    }

    /// Ends a turn with lanes: logs how each lane ended, then moves the turn to done, failed or (when a person
    /// stopped it) leaves it stopped, and clears what is left of its placeholders. `options` were logged as they
    /// were made.
    fn finish_lanes(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
        mut outcomes: Vec<LaneOutcome>,
        planned: bool,
        options: Vec<OptionRecord>,
    ) -> Result<ParallelOutcome, OrchestrationError> {
        if let Some(error) = outcomes.iter_mut().find_map(|o| o.error.take()) {
            clear(docs, spec);
            return Err(error);
        }
        for outcome in &mut outcomes {
            outcome.listed = true;
        }
        self.end_lanes(docs, spec, &outcomes)?;
        let end = super::lanes::combine(outcomes.iter().map(|o| (o.end, o.halted)));
        let speaker = outcomes.iter().find(|o| !o.halted).unwrap_or(&outcomes[0]);
        let origin = Self::origin_of(&speaker.agent, spec);
        let state = self.finish(docs, spec, &origin, Phase::Building, planned, end)?;
        clear(docs, spec);
        let lanes = outcomes
            .iter()
            .map(|o| LaneResult {
                agent: o.agent.clone(),
                name: o.name.clone(),
                end: if o.halted || state == TurnState::Stopped {
                    LaneEnd::Stopped
                } else {
                    super::lanes::lane_end(o.end)
                },
            })
            .collect();
        Ok(ParallelOutcome {
            state,
            lanes,
            options,
        })
    }
}

/// The fixed size of frame `id` in `doc`, when it is a frame; a hugging or filling side counts its fallback.
fn frame_size(doc: &Document, id: &NodeId) -> Option<(f64, f64)> {
    let Some(Node::Frame(frame)) = ops::find(doc, id) else {
        return None;
    };
    let side = |size: &Size| match size {
        Size::Fixed(Value::Literal(v)) => Some(*v),
        Size::Hug { fallback } | Size::Fill { fallback } => *fallback,
        Size::Fixed(Value::Var(_)) => None,
    };
    Some((
        side(&frame.width).unwrap_or(0.0).max(1.0),
        side(&frame.height).unwrap_or(0.0).max(1.0),
    ))
}

impl LaneOutcome {
    /// A lane's outcome made after the fact (Explore's one agent over its successive runs).
    fn summary(agent: AgentId, name: String, session: Session, end: TurnEnd) -> Self {
        Self {
            order: 0,
            agent,
            name,
            session,
            phase: Phase::Building,
            end,
            halted: false,
            listed: true,
            report: None,
            error: None,
        }
    }
}
