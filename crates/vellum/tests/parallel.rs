//! TEST DATA. Parallel agents (Split, Side by side, Explore), spawn_agents, Stop / Stop this agent / Stop all /
//! Pause all, and Undo this turn (guide Prompt 39; ARCHITECTURE §7.5; Invariants 3, 4, 12). Fake engines driven
//! through the real tool registry, scene and daemon documents, with the session log in memory.
mod common;

use std::future::Future;
use std::pin::{Pin, pin};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use common::{Daemon, design};
use serde_json::{Value, json};
use vellum::daemon::SpawnRequest;
use vellum::orchestrate::{
    Area, LaneSpec, OrchestrationError, Orchestrator, ParallelOutcome, TurnSpec,
};
use vellum::sessions::{SessionLog, pricing};
use vellum::tools::{AgentSettings, Client, Registry, RegistryBridge, Session};
use vellum_contracts::fakes::{FakeEngine, FakeStep, MemorySessions, StalledModel};
use vellum_contracts::suites::model::block_on;
use vellum_core::parallel::{Axis, LaneEnd, Mode, ParallelEvent, parallel_state};
use vellum_core::plans::{PlanStatus, plan_state};
use vellum_core::ports::*;
use vellum_core::turns::{TurnInput, TurnState};
use vellum_types::{Actor, AgentId, DeviceId, PageId, SessionId, TurnId};

const PATH: &str = "design/app.vellum";
const HOME: &str = "pageHome01";

fn ulid(n: u32) -> String {
    format!("01J{n:023}")
}
fn agent(n: u32) -> AgentId {
    AgentId::new(ulid(n)).unwrap()
}

const TEAL: u32 = 2;
const SAFFRON: u32 = 5;
const ROSE: u32 = 6;

/// The names built-in helpers get, in order; the host takes them from the design's agent hues.
fn names() -> Vec<String> {
    ["Saffron", "Rose", "Green"].map(String::from).to_vec()
}

fn lane(n: u32, name: &str, model: &str) -> LaneSpec {
    LaneSpec {
        agent: agent(n),
        name: name.into(),
        model: model.into(),
    }
}

struct Rig {
    daemon: Daemon,
    log: SessionLog,
    registry: Registry,
    session: Session,
    spec: TurnSpec,
    teal: FakeEngine,
    saffron: FakeEngine,
    rose: FakeEngine,
}

fn rig_with(teal: FakeEngine, saffron: FakeEngine, rose: FakeEngine) -> Rig {
    let daemon = Daemon::with_scene(&design(), &[PATH]);
    let log = SessionLog::new(Box::new(MemorySessions::new()), pricing());
    let id = SessionId::new(ulid(1)).unwrap();
    log.store()
        .create(NewSession {
            id: id.clone(),
            doc: DocKey::new("/repo", PATH).unwrap(),
            kind: SessionKind::Engine {
                engine: "fake".into(),
            },
            agent: agent(TEAL),
            title: "TEST DATA: Onboarding".into(),
            created_at: Timestamp(0),
        })
        .unwrap();
    Rig {
        daemon,
        log,
        registry: Registry::standard(),
        session: Session::for_agent("Teal", agent(TEAL), id.clone()),
        spec: TurnSpec {
            session: id,
            turn: TurnId::new(ulid(3)).unwrap(),
            person: Actor::Person {
                device: DeviceId::new(ulid(4)).unwrap(),
            },
            document: PATH.into(),
        },
        teal,
        saffron,
        rose,
    }
}

fn rig(teal: Vec<Vec<FakeStep>>) -> Rig {
    rig_with(
        FakeEngine::new(teal),
        FakeEngine::new(vec![]),
        FakeEngine::new(vec![]),
    )
}

/// The orchestrator over the rig's engines, borrowing only the fields it needs.
macro_rules! orchestrator {
    ($rig:expr) => {
        Orchestrator::new(&$rig.teal, &$rig.registry, &$rig.log)
            .with_settings(vellum_core::plans::PlanSettings {
                min_frames: 2,
                ..Default::default()
            })
            .with_helpers(vec![&$rig.saffron, &$rig.rose], names())
    };
}

impl Rig {
    fn request(&self, prompt: &str) -> TurnRequest {
        TurnRequest::new(self.spec.turn.clone(), "fake-model", prompt)
    }
    fn events(&self) -> Vec<SessionEvent> {
        self.log.store().events(&self.spec.session, None).unwrap()
    }
    fn transitions(&self) -> Vec<(TurnInput, TurnState, bool)> {
        self.events()
            .iter()
            .filter_map(|e| match &e.body {
                EventBody::Turn(t) => Some((t.input, t.to, e.origin.actor.is_person())),
                _ => None,
            })
            .collect()
    }
    fn parallel_events(&self) -> Vec<(ParallelEvent, Actor)> {
        self.events()
            .into_iter()
            .filter_map(|e| match e.body {
                EventBody::Parallel(p) => Some((p, e.origin.actor)),
                _ => None,
            })
            .collect()
    }
    /// Every tool reply in the log, with the agent whose lane called it.
    fn replies(&self) -> Vec<(AgentId, String, Value, bool)> {
        self.events()
            .into_iter()
            .filter_map(|e| match (e.body, e.origin.actor) {
                (EventBody::Messages(m), Actor::Agent { agent }) => Some((agent, m)),
                _ => None,
            })
            .flat_map(|(agent, messages)| {
                messages.into_iter().flat_map(move |m| {
                    let agent = agent.clone();
                    m.parts.into_iter().filter_map(move |part| match part {
                        StoredPart::ToolResult {
                            name,
                            result,
                            failed,
                            ..
                        } => Some((agent.clone(), name, result, failed)),
                        _ => None,
                    })
                })
            })
            .collect()
    }
    fn doc(&mut self) -> vellum_types::Document {
        self.daemon.docs.target(PATH).unwrap().snapshot().unwrap()
    }
    /// Another agent, connected over MCP, calls a tool; the reply comes back whether it failed or not.
    fn external(&mut self, session: &mut Session, tool: &str, input: Value) -> ToolReply {
        let bridge = RegistryBridge::new(&self.registry, &mut self.daemon.docs, session);
        vellum_core::ports::ToolBridge::call(&bridge, tool, input)
    }
}

fn claude_code() -> Session {
    let mut session = Session::default();
    session.client = Some(Client {
        name: "Claude Code".into(),
        version: String::new(),
    });
    session
}

fn call(name: &str, input: Value) -> FakeStep {
    FakeStep::Call {
        name: name.into(),
        input,
    }
}

fn screens(names: &[&str]) -> Value {
    json!({
        "title": "TEST DATA: Onboarding",
        "items": names.iter().map(|name| json!({
            "name": name, "page": "Home", "intent": format!("TEST DATA: the {name} screen"),
            "width": 390, "height": 844,
        })).collect::<Vec<_>>(),
    })
}

fn frame_at(id: &str, x: f64, y: f64, width: f64, height: f64) -> Value {
    json!({"parent": HOME, "nodes": [{
        "type": "frame", "id": id, "name": "TEST DATA screen",
        "position": {"type": "absolute", "x": x, "y": y},
        "width": width, "height": height,
    }]})
}

fn insert(p: &vellum_core::plans::Placeholder) -> FakeStep {
    call(
        "insert_nodes",
        frame_at(
            p.frame.as_str(),
            p.bounds.x,
            p.bounds.y,
            p.bounds.width,
            p.bounds.height,
        ),
    )
}

/// The first frame the prompt reserves: its id and bounds ("new frame `id` … at x 1, y 2, 3 × 4").
fn reserved(prompt: &str) -> (String, [f64; 4]) {
    let start = prompt.find("new frame `").expect("a reserved frame") + "new frame `".len();
    let id = prompt[start..].split('`').next().unwrap().to_owned();
    let at = prompt[start..].find("at x ").unwrap() + start + "at x ".len();
    let numbers: Vec<f64> = prompt[at..]
        .split([',', ' ', '×', 'y', ':', '\n'])
        .filter(|s| !s.is_empty())
        .take(4)
        .map(|s| s.parse().unwrap())
        .collect();
    (id, [numbers[0], numbers[1], numbers[2], numbers[3]])
}

fn build_reserved(prompt: &str) -> Value {
    let (id, [x, y, w, h]) = reserved(prompt);
    frame_at(&id, x, y, w, h)
}

/// Builds a smaller first result than planned, so variants are sized from what was built.
fn build_reserved_small(prompt: &str) -> Value {
    let (id, [x, y, ..]) = reserved(prompt);
    frame_at(&id, x, y, 300.0, 200.0)
}

fn from_prompt(name: &str, input: fn(&str) -> Value) -> FakeStep {
    FakeStep::FromPrompt {
        name: name.into(),
        input,
    }
}

fn progress(message: &str) -> FakeStep {
    call("report_progress", json!({"message": message}))
}

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// Planned, then Build split across Teal and Saffron.
fn split_two(rig: &mut Rig, items: &[&str]) -> Vec<vellum_core::plans::Placeholder> {
    rig.teal.queue(vec![call("propose_plan", screens(items))]);
    let request = rig.request("TEST DATA: Design onboarding");
    let orchestrator = orchestrator!(rig);
    let state =
        block_on(orchestrator.start(&mut rig.daemon.docs, &mut rig.session, &rig.spec, request));
    assert_eq!(state.unwrap(), TurnState::AwaitingPlan);
    orchestrator
        .build_split(
            &mut rig.daemon.docs,
            &rig.spec,
            None,
            &[
                lane(TEAL, "Teal", "fake-model"),
                lane(SAFFRON, "Saffron", "other-model"),
            ],
        )
        .unwrap()
}

// ---- Split ---------------------------------------------------------------------------------------------------

#[test]
fn split_divides_plan_items_into_lanes_each_with_its_own_run_leases_and_hue() {
    let mut rig = rig(vec![]);
    let placeholders = split_two(&mut rig, &["Welcome", "Step 1", "Step 2", "Step 3"]);
    let owner = |p: &vellum_core::plans::Placeholder| p.agent.clone();
    assert_eq!(
        placeholders.iter().map(owner).collect::<Vec<_>>(),
        [agent(TEAL), agent(TEAL), agent(SAFFRON), agent(SAFFRON)],
        "items 1–2 are Teal's, 3–4 Saffron's"
    );
    for (n, p) in placeholders.iter().enumerate() {
        for q in &placeholders[n + 1..] {
            assert!(!overlaps(&p.bounds, &q.bounds), "{p:?} on {q:?}");
        }
    }
    let [(lanes, by)] = rig.parallel_events().try_into().expect("one Lanes event");
    assert!(by.is_person(), "the person pressed Build");
    let ParallelEvent::Lanes {
        mode: Mode::Split,
        lanes,
    } = lanes
    else {
        panic!("{lanes:?}")
    };
    assert_eq!(lanes[0].items, [1, 2]);
    assert_eq!(lanes[1].items, [3, 4]);
    assert_eq!(
        (lanes[1].name.as_str(), lanes[1].model.as_str()),
        ("Saffron", "other-model")
    );

    let (p1, p2, p3, p4) = (
        &placeholders[0],
        &placeholders[1],
        &placeholders[2],
        &placeholders[3],
    );
    rig.teal.queue(vec![
        insert(p1),
        FakeStep::Wait,
        insert(p2),
        call(
            "report_progress",
            json!({"message": "TEST DATA: Welcome and Step 1.", "items": [1, 2]}),
        ),
    ]);
    rig.saffron.queue(vec![
        insert(p3),
        // Teal holds its frame: Saffron's write there is refused, named by hue.
        call(
            "update_nodes",
            json!({"updates": [{"id": p1.frame.as_str(), "set": {"name": "TEST DATA: not yours"}}]}),
        ),
        call("get_app_state", json!({})),
        insert(p4),
        call(
            "report_progress",
            json!({"message": "TEST DATA: Steps 2 and 3.", "items": [3, 4]}),
        ),
    ]);
    let template = rig.request("ignored: each lane gets its share of the plan");
    let orchestrator = orchestrator!(rig);
    {
        let mut run = pin!(orchestrator.run_split(&mut rig.daemon.docs, &rig.spec, template));
        assert!(poll(run.as_mut()).is_pending(), "Teal waits");
        assert_eq!(
            rig.saffron.requests().len(),
            1,
            "Saffron ran while Teal was still working: the lanes run at the same time"
        );
        rig.teal.release();
        let Poll::Ready(outcome) = poll(run.as_mut()) else {
            panic!("both lanes are done")
        };
        let outcome: ParallelOutcome = outcome.unwrap();
        assert_eq!(outcome.state, TurnState::Done);
        assert_eq!(
            outcome
                .lanes
                .iter()
                .map(|l| (l.name.as_str(), l.end))
                .collect::<Vec<_>>(),
            [("Teal", LaneEnd::Done), ("Saffron", LaneEnd::Done)]
        );
    }
    drop(orchestrator);

    // Each lane was told its own items, on its own model.
    let teal_build = &rig.teal.requests()[1];
    assert!(
        teal_build.prompt.contains(p1.frame.as_str())
            && teal_build.prompt.contains(p2.frame.as_str())
    );
    assert!(
        !teal_build.prompt.contains(p3.frame.as_str()),
        "{}",
        teal_build.prompt
    );
    let saffron_build = &rig.saffron.requests()[0];
    assert!(saffron_build.prompt.contains(p4.frame.as_str()));
    assert!(!saffron_build.prompt.contains(p1.frame.as_str()));
    assert_eq!(saffron_build.model, "other-model");
    assert_eq!(saffron_build.turn, rig.spec.turn, "one turn, several lanes");

    let replies = rig.replies();
    let refused = replies
        .iter()
        .find(|(_, tool, ..)| tool == "update_nodes")
        .unwrap();
    assert_eq!(refused.0, agent(SAFFRON));
    assert!(refused.3, "{}", refused.2);
    assert_eq!(refused.2["error"]["kind"], "refused");
    assert!(
        refused.2["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Frame leased by Teal"),
        "{}",
        refused.2
    );
    let state = &replies
        .iter()
        .find(|(_, tool, ..)| tool == "get_app_state")
        .unwrap()
        .2;
    let leases = state["leases"].as_array().unwrap();
    for (frame, by) in [(p1, TEAL), (p3, SAFFRON)] {
        assert!(
            leases
                .iter()
                .any(|l| l["frame"] == frame.frame.as_str() && l["agent"] == ulid(by)),
            "each lane leases its own frames: {state}"
        );
    }
    let saffron_placeholder = state["placeholders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["frame"] == p4.frame.as_str())
        .unwrap();
    assert_eq!(saffron_placeholder["agentName"], "Saffron");

    let plan = plan_state(&rig.events(), &rig.spec.turn).unwrap();
    assert_eq!(plan.status, PlanStatus::Finished);
    assert_eq!(plan.progress(), (4, 4), "ticks from both lanes count");
    let lanes = parallel_state(&rig.events(), &rig.spec.turn).unwrap();
    assert!(lanes.lanes.iter().all(|l| l.end == Some(LaneEnd::Done)));
    assert_eq!(
        rig.transitions().last(),
        Some(&(TurnInput::Done, TurnState::Done, false))
    );
    assert!(rig.daemon.docs.placeholders(PATH).is_empty());
    assert!(rig.daemon.docs.view(PATH).unwrap().state.leases.is_empty());
}

#[test]
fn a_split_needs_an_engine_per_lane_and_at_most_six_lanes() {
    let mut rig = rig(vec![]);
    rig.teal
        .queue(vec![call("propose_plan", screens(&["A", "B", "C", "D"]))]);
    let request = rig.request("TEST DATA: four screens");
    let orchestrator = orchestrator!(rig);
    block_on(orchestrator.start(&mut rig.daemon.docs, &mut rig.session, &rig.spec, request))
        .unwrap();
    let four: Vec<LaneSpec> = (0..4)
        .map(|n| lane(20 + n, "TEST DATA lane", "fake-model"))
        .collect();
    let error = orchestrator.build_split(&mut rig.daemon.docs, &rig.spec, None, &four);
    assert!(
        matches!(
            error,
            Err(OrchestrationError::Engines { needed: 4, have: 3 })
        ),
        "{error:?}"
    );
    let seven: Vec<LaneSpec> = (0..7)
        .map(|n| lane(20 + n, "TEST DATA lane", "fake-model"))
        .collect();
    let error = orchestrator.build_split(&mut rig.daemon.docs, &rig.spec, None, &seven);
    assert!(
        matches!(error, Err(OrchestrationError::Parallel(_))),
        "{error:?}"
    );
    // Nothing was built: the plan still waits.
    assert!(rig.daemon.docs.placeholders(PATH).is_empty());
    assert_eq!(
        plan_state(&rig.events(), &rig.spec.turn).unwrap().status,
        PlanStatus::Proposed
    );
}

// ---- Side by side ----------------------------------------------------------------------------------------------

#[test]
fn side_by_side_gives_each_lane_the_same_brief_its_own_area_and_its_own_model() {
    let mut rig = rig(vec![]);
    for engine in [&rig.teal, &rig.saffron, &rig.rose] {
        engine.queue(vec![
            from_prompt("insert_nodes", build_reserved),
            call("get_app_state", json!({})),
        ]);
    }
    let lanes = [
        lane(TEAL, "Teal", "fake-model"),
        lane(SAFFRON, "Saffron", "fake-model"),
        lane(ROSE, "Rose", "other-model"),
    ];
    let request = rig.request("TEST DATA: a paywall");
    let orchestrator = orchestrator!(rig);
    let outcome = block_on(orchestrator.side_by_side(
        &mut rig.daemon.docs,
        &rig.spec,
        request,
        &lanes,
        Area {
            page: PageId::new(HOME).unwrap(),
            width: 390.0,
            height: 844.0,
        },
    ))
    .unwrap();
    drop(orchestrator);
    assert_eq!(outcome.state, TurnState::Done);

    let requests: Vec<TurnRequest> = [&rig.teal, &rig.saffron, &rig.rose]
        .iter()
        .map(|e| e.requests().remove(0))
        .collect();
    assert!(
        requests
            .iter()
            .all(|r| r.prompt.starts_with("TEST DATA: a paywall"))
    );
    assert_eq!(
        requests
            .iter()
            .map(|r| r.model.as_str())
            .collect::<Vec<_>>(),
        ["fake-model", "fake-model", "other-model"]
    );
    let frames: Vec<String> = requests.iter().map(|r| reserved(&r.prompt).0).collect();
    assert_eq!(
        frames
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3,
        "each lane has its own frame"
    );
    assert!(requests[1].prompt.contains("option \"B\""));

    // The options are the three frames, A B C, each its agent's.
    let options: Vec<(u32, String, AgentId)> = outcome
        .options
        .iter()
        .map(|o| (o.option, o.frame.to_string(), o.agent.clone()))
        .collect();
    assert_eq!(
        options,
        [
            (1, frames[0].clone(), agent(TEAL)),
            (2, frames[1].clone(), agent(SAFFRON)),
            (3, frames[2].clone(), agent(ROSE)),
        ]
    );
    let doc = rig.doc();
    let bounds: Vec<Rect> = requests
        .iter()
        .map(|r| {
            let [x, y, w, h] = reserved(&r.prompt).1;
            Rect::new(x, y, w, h)
        })
        .collect();
    for (n, b) in bounds.iter().enumerate() {
        assert!(
            vellum_core::ops::find(
                &doc,
                &vellum_types::NodeId::new(frames[n].as_str()).unwrap()
            )
            .is_some()
        );
        for c in &bounds[n + 1..] {
            assert!(!overlaps(b, c), "separate areas: {b:?} {c:?}");
        }
    }
    assert_eq!(
        rig.transitions(),
        [
            (TurnInput::Start, TurnState::Planning, true),
            (TurnInput::NoPlanNeeded, TurnState::Running, true),
            (TurnInput::Done, TurnState::Done, false),
        ]
    );
    let state = parallel_state(&rig.events(), &rig.spec.turn).unwrap();
    assert_eq!(state.mode, Mode::SideBySide);
    assert_eq!(state.lanes[2].record.model, "other-model");
    assert_eq!(state.options.len(), 3);
    assert!(rig.daemon.docs.placeholders(PATH).is_empty());
}

// ---- Explore ---------------------------------------------------------------------------------------------------

#[test]
fn explore_makes_successive_variants_that_vary_only_one_axis() {
    let mut rig = rig(vec![
        vec![from_prompt("insert_nodes", build_reserved_small)],
        vec![
            from_prompt("insert_nodes", build_reserved),
            progress("TEST DATA: warmer palette"),
        ],
        vec![
            from_prompt("insert_nodes", build_reserved),
            progress("TEST DATA: serif headings"),
        ],
    ]);
    let request = rig.request("TEST DATA: a paywall");
    let orchestrator = orchestrator!(rig);
    let outcome = block_on(orchestrator.explore(
        &mut rig.daemon.docs,
        &mut rig.session,
        &rig.spec,
        request,
        Axis::Style,
        3,
        Area {
            page: PageId::new(HOME).unwrap(),
            width: 390.0,
            height: 844.0,
        },
    ))
    .unwrap();
    drop(orchestrator);
    assert_eq!(outcome.state, TurnState::Done);

    let requests = rig.teal.requests();
    assert_eq!(requests.len(), 3, "one run per result, one after another");
    let first = reserved(&requests[0].prompt).0;
    assert!(requests[0].prompt.starts_with("TEST DATA: a paywall"));
    for (n, request) in requests.iter().enumerate().skip(1) {
        assert!(
            request.prompt.contains(&format!("Variant {} of 3", n + 1)),
            "{}",
            request.prompt
        );
        assert!(
            request.prompt.contains("only its style"),
            "{}",
            request.prompt
        );
        assert!(
            request.prompt.contains(&first),
            "variants copy the first result"
        );
        assert!(
            request.prompt.contains("300 × 200"),
            "sized like what was built: {}",
            request.prompt
        );
        assert!(!request.history.is_empty(), "each run sees the ones before");
    }
    assert_eq!(
        outcome
            .options
            .iter()
            .map(|o| (o.option, o.label.as_deref()))
            .collect::<Vec<_>>(),
        [
            (1, None),
            (2, Some("TEST DATA: warmer palette")),
            (3, Some("TEST DATA: serif headings")),
        ]
    );
    assert_eq!(outcome.options[0].frame.as_str(), first);
    let doc = rig.doc();
    for option in &outcome.options {
        assert!(vellum_core::ops::find(&doc, &option.frame).is_some());
    }
    let state = parallel_state(&rig.events(), &rig.spec.turn).unwrap();
    assert_eq!(state.mode, Mode::Explore(Axis::Style));
    assert_eq!(state.lanes.len(), 1, "one agent explores");
    assert_eq!(state.lanes[0].end, Some(LaneEnd::Done));
    assert_eq!(
        rig.transitions().last(),
        Some(&(TurnInput::Done, TurnState::Done, false))
    );
    assert!(rig.daemon.docs.placeholders(PATH).is_empty());

    // Explore makes 2 to 6.
    let request = rig.request("TEST DATA: again");
    let orchestrator = orchestrator!(rig);
    let error = block_on(orchestrator.explore(
        &mut rig.daemon.docs,
        &mut rig.session,
        &rig.spec,
        request,
        Axis::Layout,
        7,
        Area {
            page: PageId::new(HOME).unwrap(),
            width: 390.0,
            height: 844.0,
        },
    ));
    assert!(
        matches!(error, Err(OrchestrationError::Parallel(_))),
        "{error:?}"
    );
}

// ---- spawn_agents ----------------------------------------------------------------------------------------------

#[test]
fn spawn_agents_starts_helper_lanes_for_built_in_engines() {
    let spawn = |tasks: Value| call("spawn_agents", json!({ "tasks": tasks }));
    let mut rig = rig(vec![vec![
        spawn(json!([
            {"task": "TEST DATA: the icon set"},
            {"task": "TEST DATA: the empty states", "model": "other-model"},
        ])),
        // Every helper engine is busy now.
        spawn(json!([{"task": "TEST DATA: one more"}])),
        call(
            "insert_nodes",
            frame_at("frameMain1", 2000.0, 0.0, 390.0, 844.0),
        ),
    ]]);
    rig.saffron.queue(vec![
        call(
            "insert_nodes",
            frame_at("frameIcon1", 3000.0, 0.0, 100.0, 100.0),
        ),
        progress("TEST DATA: icons done"),
    ]);
    rig.rose.queue(vec![
        call(
            "insert_nodes",
            frame_at("frameEmpty", 4000.0, 0.0, 100.0, 100.0),
        ),
        progress("TEST DATA: empty states done"),
    ]);
    let request = rig.request("TEST DATA: Design the settings screen");
    let orchestrator = orchestrator!(rig);
    let state =
        block_on(orchestrator.start(&mut rig.daemon.docs, &mut rig.session, &rig.spec, request));
    drop(orchestrator);
    assert_eq!(state.unwrap(), TurnState::Done);

    let replies = rig.replies();
    let spawned = &replies
        .iter()
        .find(|(_, tool, ..)| tool == "spawn_agents")
        .unwrap()
        .2;
    assert_eq!(
        spawned["spawned"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Saffron", "Rose"]
    );
    let busy = replies
        .iter()
        .filter(|(_, tool, ..)| tool == "spawn_agents")
        .nth(1)
        .unwrap();
    assert!(busy.3, "no engine is free for a third helper: {}", busy.2);

    let saffron = &rig.saffron.requests()[0];
    assert!(
        saffron.prompt.contains("Teal started you"),
        "{}",
        saffron.prompt
    );
    assert!(saffron.prompt.contains("TEST DATA: the icon set"));
    assert_eq!(
        saffron.model, "fake-model",
        "a helper runs on its parent's model"
    );
    assert_eq!(rig.rose.requests()[0].model, "other-model");

    let events = rig.parallel_events();
    let (ParallelEvent::Lanes { mode, lanes }, by) = &events[0] else {
        panic!("{events:?}")
    };
    assert_eq!(*mode, Mode::Spawned);
    assert_eq!(
        by,
        &Actor::Agent { agent: agent(TEAL) },
        "Teal started them"
    );
    assert_eq!(lanes[0].task.as_deref(), Some("TEST DATA: the icon set"));
    let state = parallel_state(&rig.events(), &rig.spec.turn).unwrap();
    assert!(state.lanes.iter().all(|l| l.end == Some(LaneEnd::Done)));
    // The helpers' changes are theirs, in the same turn: they reported through report_progress.
    assert!(
        replies
            .iter()
            .any(|(by, tool, ..)| *by == lanes[1].agent && tool == "report_progress")
    );
    let doc = rig.doc();
    for id in ["frameMain1", "frameIcon1", "frameEmpty"] {
        assert!(
            vellum_core::ops::find(&doc, &vellum_types::NodeId::new(id).unwrap()).is_some(),
            "{id}: {:?}",
            replies
                .iter()
                .filter(|(_, tool, ..)| tool == "insert_nodes")
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn external_clients_get_spawn_agents_only_when_the_setting_allows_it() {
    let mut rig = rig(vec![]);
    let input = json!({"document": PATH, "tasks": [{"task": "TEST DATA: the icon set"}]});

    // Off by default: not offered, and not callable.
    assert!(rig.registry.get("spawn_agents").is_none());
    let mut external = claude_code();
    let reply = rig.external(&mut external, "spawn_agents", input.clone());
    assert!(reply.failed);
    assert_eq!(reply.value["error"]["kind"], "not_found");

    rig.registry = Registry::standard_with(&AgentSettings {
        spawn_agents_external: true,
    });
    assert!(rig.registry.get("spawn_agents").is_some());
    // Allowed, but nothing here runs helpers yet: refused, saying so.
    let reply = rig.external(&mut external, "spawn_agents", input.clone());
    assert!(reply.failed);
    assert_eq!(reply.value["error"]["kind"], "refused", "{}", reply.value);

    rig.daemon.docs.accept_spawns();
    let reply = rig.external(&mut external, "spawn_agents", input);
    assert!(!reply.failed, "{}", reply.value);
    let [request]: [SpawnRequest; 1] = rig.daemon.docs.take_spawns().try_into().unwrap();
    assert_eq!(&request.by, external.agent().unwrap());
    assert_eq!(request.by_name, "Claude Code");
    assert_eq!(request.document, PATH);

    // The host starts them as lanes of an engine session it made, on its free engines (the main one first).
    rig.teal.queue(vec![
        call(
            "insert_nodes",
            frame_at("frameIcon1", 3000.0, 0.0, 100.0, 100.0),
        ),
        progress("TEST DATA: icons done"),
    ]);
    let template = rig.request("ignored: each helper is told its task");
    let orchestrator = orchestrator!(rig);
    let outcome =
        block_on(orchestrator.run_spawned(&mut rig.daemon.docs, &rig.spec, request, template))
            .unwrap();
    drop(orchestrator);
    assert_eq!(outcome.state, TurnState::Done);
    assert_eq!(outcome.lanes.len(), 1);
    assert_eq!(outcome.lanes[0].name, "Saffron");
    assert!(
        rig.teal.requests()[0]
            .prompt
            .contains("Claude Code started you")
    );
    let state = parallel_state(&rig.events(), &rig.spec.turn).unwrap();
    assert_eq!(state.mode, Mode::Spawned);
}

// ---- Stop, Stop this agent, Stop all, Pause all (Invariants 4 and 12) ------------------------------------------

#[test]
fn stop_ends_every_lane_within_the_budgets_and_keeps_what_was_built() {
    let mut rig = rig(vec![]);
    let placeholders = split_two(&mut rig, &["Welcome", "Step 1"]);
    rig.teal
        .queue(vec![insert(&placeholders[0]), FakeStep::Work]);
    rig.saffron
        .queue(vec![insert(&placeholders[1]), FakeStep::Work]);
    let template = rig.request("build");
    let orchestrator = orchestrator!(rig);
    {
        let mut run = pin!(orchestrator.run_split(&mut rig.daemon.docs, &rig.spec, template));
        assert!(poll(run.as_mut()).is_pending(), "both agents are working");
        let pressed = Instant::now();
        assert_eq!(
            orchestrator.stop(None, &rig.spec).unwrap(),
            TurnState::Stopped
        );
        let Poll::Ready(outcome) = poll(run.as_mut()) else {
            panic!("Stop ends every lane")
        };
        assert!(pressed.elapsed() < Duration::from_secs(1), "Invariant 12");
        let outcome = outcome.unwrap();
        assert_eq!(outcome.state, TurnState::Stopped);
        assert!(outcome.lanes.iter().all(|l| l.end == LaneEnd::Stopped));
    }
    drop(orchestrator);
    assert_eq!(
        rig.transitions().last(),
        Some(&(TurnInput::Stop, TurnState::Stopped, true))
    );
    // Stopping keeps everything already built (and offers Undo this turn).
    let doc = rig.doc();
    for p in &placeholders {
        assert!(vellum_core::ops::find(&doc, &p.frame).is_some());
    }
    assert!(rig.daemon.docs.placeholders(PATH).is_empty());
    let view = rig.daemon.docs.view(PATH).unwrap();
    assert!(view.state.leases.is_empty());
    assert_eq!(
        view.state.paused.stopped,
        [agent(TEAL), agent(SAFFRON)],
        "the Guard refuses both agents until their next turn"
    );
    let state = parallel_state(&rig.events(), &rig.spec.turn).unwrap();
    assert!(state.lanes.iter().all(|l| l.end == Some(LaneEnd::Stopped)));
}

#[test]
fn stop_this_agent_stops_one_lane_and_the_guard_refuses_its_next_write() {
    // Teal ignores Stop: the Guard is what keeps it from writing.
    let mut rig = rig_with(
        FakeEngine::new(vec![]).deaf(),
        FakeEngine::new(vec![]),
        FakeEngine::new(vec![]),
    );
    let placeholders = split_two(&mut rig, &["Welcome", "Step 1"]);
    rig.teal.queue(vec![
        insert(&placeholders[0]),
        FakeStep::Wait,
        call(
            "update_nodes",
            json!({"updates": [{"id": placeholders[0].frame.as_str(), "set": {"name": "TEST DATA: after Stop"}}]}),
        ),
    ]);
    rig.saffron.queue(vec![
        insert(&placeholders[1]),
        FakeStep::Wait,
        call(
            "report_progress",
            json!({"message": "TEST DATA: Step 1.", "items": [2]}),
        ),
    ]);
    let template = rig.request("build");
    let orchestrator = orchestrator!(rig);
    {
        let mut run = pin!(orchestrator.run_split(&mut rig.daemon.docs, &rig.spec, template));
        assert!(poll(run.as_mut()).is_pending());
        assert_eq!(
            orchestrator
                .stop_agent(None, &rig.spec, &agent(TEAL))
                .unwrap(),
            TurnState::Running,
            "Saffron is still working"
        );
        rig.teal.release();
        rig.saffron.release();
        let Poll::Ready(outcome) = poll(run.as_mut()) else {
            panic!("done")
        };
        let outcome = outcome.unwrap();
        assert_eq!(outcome.state, TurnState::Done);
        assert_eq!(
            outcome.lanes.iter().map(|l| l.end).collect::<Vec<_>>(),
            [LaneEnd::Stopped, LaneEnd::Done]
        );
    }
    drop(orchestrator);
    let replies = rig.replies();
    let late = replies
        .iter()
        .find(|(_, tool, ..)| tool == "update_nodes")
        .unwrap();
    assert!(late.3, "Teal's write after Stop is refused");
    assert_eq!(late.2["error"]["message"], "Stopped by you");
    let rejected = rig.events().into_iter().any(|e| {
        matches!(&e.body, EventBody::Step(s) if s.tool == "update_nodes" && s.outcome == StepOutcome::Rejected)
    });
    assert!(
        rejected,
        "the rejection is a step in the session: {:?}",
        rig.events()
            .into_iter()
            .filter_map(|e| match e.body {
                EventBody::Step(s) => Some((s.tool, s.outcome)),
                _ => None,
            })
            .collect::<Vec<_>>()
    );
    let state = parallel_state(&rig.events(), &rig.spec.turn).unwrap();
    assert_eq!(state.lanes[0].end, Some(LaneEnd::Stopped));
    assert_eq!(state.lanes[1].end, Some(LaneEnd::Done));
    let ended_by_person = rig.parallel_events().into_iter().any(|(e, by)| {
        matches!(e, ParallelEvent::LaneEnded { agent: ref a, end: LaneEnd::Stopped } if *a == agent(TEAL))
            && by.is_person()
    });
    assert!(ended_by_person);
    assert_eq!(
        rig.transitions().last(),
        Some(&(TurnInput::Done, TurnState::Done, false))
    );
}

#[test]
fn stop_all_stops_every_agent_of_the_turn_and_leaves_external_agents_alone() {
    let mut rig = rig(vec![]);
    let placeholders = split_two(&mut rig, &["Welcome", "Step 1"]);
    rig.teal
        .queue(vec![insert(&placeholders[0]), FakeStep::Work]);
    rig.saffron
        .queue(vec![insert(&placeholders[1]), FakeStep::Work]);
    let template = rig.request("build");
    let orchestrator = orchestrator!(rig);
    {
        let mut run = pin!(orchestrator.run_split(&mut rig.daemon.docs, &rig.spec, template));
        assert!(poll(run.as_mut()).is_pending());
        let stopped = orchestrator.stop_all(None).unwrap();
        assert_eq!(stopped, [(rig.spec.turn.clone(), TurnState::Stopped)]);
        let Poll::Ready(outcome) = poll(run.as_mut()) else {
            panic!("Stop all ends the run")
        };
        assert_eq!(outcome.unwrap().state, TurnState::Stopped);
    }
    drop(orchestrator);
    let mut external = claude_code();
    let reply = rig.external(
        &mut external,
        "insert_nodes",
        json!({"document": PATH, "parent": HOME, "nodes": [{"type": "frame", "name": "TEST DATA external",
               "position": {"type": "absolute", "x": 5000, "y": 0}, "width": 10, "height": 10}]}),
    );
    assert!(!reply.failed, "Stop all is not Pause all: {}", reply.value);
}

#[test]
fn pause_all_rejects_every_agents_next_write_at_once_and_interrupts_the_engines() {
    let mut rig = rig_with(
        FakeEngine::new(vec![]).deaf(),
        FakeEngine::new(vec![]),
        FakeEngine::new(vec![]),
    );
    let placeholders = split_two(&mut rig, &["Welcome", "Step 1"]);
    rig.teal.queue(vec![
        insert(&placeholders[0]),
        FakeStep::Wait,
        call(
            "update_nodes",
            json!({"updates": [{"id": placeholders[0].frame.as_str(), "set": {"name": "TEST DATA: after Pause all"}}]}),
        ),
    ]);
    rig.saffron
        .queue(vec![insert(&placeholders[1]), FakeStep::Work]);
    let template = rig.request("build");
    let orchestrator = orchestrator!(rig);
    let rejected_after;
    {
        let mut run = pin!(orchestrator.run_split(&mut rig.daemon.docs, &rig.spec, template));
        assert!(poll(run.as_mut()).is_pending());
        let pressed = Instant::now();
        let stopped = orchestrator.pause_all(None).unwrap();
        assert_eq!(stopped, [rig.spec.turn.clone()], "Pause all stops the turn");
        rig.teal.release();
        let Poll::Ready(outcome) = poll(run.as_mut()) else {
            panic!("the engines were interrupted")
        };
        rejected_after = pressed.elapsed();
        assert!(rejected_after < Duration::from_secs(3), "Invariant 12");
        assert_eq!(outcome.unwrap().state, TurnState::Stopped);
    }
    drop(orchestrator);
    assert!(
        rejected_after < Duration::from_millis(100),
        "Invariant 4: {rejected_after:?}"
    );
    let replies = rig.replies();
    let late = replies
        .iter()
        .find(|(_, tool, ..)| tool == "update_nodes")
        .unwrap();
    assert!(late.3);
    assert_eq!(late.2["error"]["message"], "Agents paused");
    assert_eq!(
        rig.transitions().last(),
        Some(&(TurnInput::Stop, TurnState::Stopped, true))
    );

    // Every agent: an external one is paused too, and so is a file opened later.
    let mut external = claude_code();
    let frame = json!({"document": PATH, "parent": HOME, "nodes": [{"type": "frame", "name": "TEST DATA external",
                       "position": {"type": "absolute", "x": 5000, "y": 0}, "width": 10, "height": 10}]});
    let reply = rig.external(&mut external, "insert_nodes", frame.clone());
    assert!(reply.failed);
    assert_eq!(reply.value["error"]["message"], "Agents paused");
    assert!(rig.daemon.docs.view(PATH).unwrap().state.paused.all);

    let orchestrator = orchestrator!(rig);
    orchestrator.resume_all(&mut rig.daemon.docs);
    drop(orchestrator);
    let reply = rig.external(&mut external, "insert_nodes", frame);
    assert!(!reply.failed, "{}", reply.value);
}

#[test]
fn pause_all_with_nothing_running_pauses_external_agents_at_once() {
    let mut rig = rig(vec![]);
    let orchestrator = orchestrator!(rig);
    assert!(
        orchestrator
            .pause_all(Some(&mut rig.daemon.docs))
            .unwrap()
            .is_empty()
    );
    drop(orchestrator);
    let mut external = claude_code();
    let reply = rig.external(
        &mut external,
        "insert_nodes",
        json!({"document": PATH, "parent": HOME, "nodes": [{"type": "frame", "name": "TEST DATA external",
               "position": {"type": "absolute", "x": 5000, "y": 0}, "width": 10, "height": 10}]}),
    );
    assert!(reply.failed);
    assert_eq!(reply.value["error"]["message"], "Agents paused");
}

/// Invariant 12 against the built-in engine: a model that never answers (and one that ignores cancellation)
/// is interrupted within 3 s of Stop.
#[test]
fn stop_interrupts_the_built_in_engine_on_a_stalled_model_within_three_seconds() {
    for deaf in [false, true] {
        let builtin = vellum_engine_builtin::BuiltinEngine::new(
            std::sync::Arc::new(StalledModel { deaf }),
            vellum_engine_builtin::PromptPack::builtin(),
        );
        let mut rig = rig(vec![]);
        let request = rig.request("TEST DATA: a slow request");
        let orchestrator = Orchestrator::new(&builtin, &rig.registry, &rig.log);
        let mut run =
            pin!(orchestrator.start(&mut rig.daemon.docs, &mut rig.session, &rig.spec, request,));
        assert!(poll(run.as_mut()).is_pending(), "the model is thinking");
        let pressed = Instant::now();
        assert_eq!(
            orchestrator.stop(None, &rig.spec).unwrap(),
            TurnState::Stopped
        );
        let state = block_on(run);
        assert!(
            pressed.elapsed() < Duration::from_secs(3),
            "deaf {deaf}: {:?}",
            pressed.elapsed()
        );
        assert_eq!(state.unwrap(), TurnState::Stopped);
    }
}

// ---- Undo this turn (Invariant 3) ------------------------------------------------------------------------------

#[test]
fn undo_this_turn_takes_back_every_lane_and_keeps_everyone_elses_changes() {
    let mut rig = rig(vec![]);
    let placeholders = split_two(&mut rig, &["Welcome", "Step 1"]);
    rig.teal.queue(vec![insert(&placeholders[0])]);
    rig.saffron.queue(vec![insert(&placeholders[1])]);
    let before = rig.doc();
    let template = rig.request("build");
    let orchestrator = orchestrator!(rig);
    // Not while it runs.
    assert!(matches!(
        orchestrator.undo_turn(&mut rig.daemon.docs, &rig.spec),
        Err(OrchestrationError::Turn(_))
    ));
    let outcome =
        block_on(orchestrator.run_split(&mut rig.daemon.docs, &rig.spec, template)).unwrap();
    drop(orchestrator);
    assert_eq!(outcome.state, TurnState::Done);

    // Others work after the turn: an external agent adds a frame, the person renames Hero.
    let mut external = claude_code();
    let reply = rig.external(
        &mut external,
        "insert_nodes",
        json!({"document": PATH, "parent": HOME, "nodes": [{"type": "frame", "id": "frameExtrn1", "name": "TEST DATA external",
               "position": {"type": "absolute", "x": 5000, "y": 0}, "width": 10, "height": 10}]}),
    );
    assert!(!reply.failed, "{}", reply.value);
    let person = vellum_types::Origin::new(rig.spec.person.clone());
    rig.daemon
        .docs
        .target(PATH)
        .unwrap()
        .transact(vellum_core::ops::Transaction::new(
            person,
            vec![vellum_core::ops::Op::Update {
                id: vellum_types::NodeId::new("frameHero1").unwrap(),
                patch: serde_json::from_value(json!({"name": "TEST DATA: Hero, renamed"})).unwrap(),
            }],
        ))
        .unwrap();

    let orchestrator = orchestrator!(rig);
    let undone = orchestrator
        .undo_turn(&mut rig.daemon.docs, &rig.spec)
        .unwrap();
    assert!(matches!(undone, TurnUndone::Undone(_)), "{undone:?}");
    let again = orchestrator.undo_turn(&mut rig.daemon.docs, &rig.spec);
    drop(orchestrator);
    assert_eq!(
        again.unwrap(),
        TurnUndone::Nothing,
        "nothing is left to undo"
    );
    let doc = rig.doc();
    for p in &placeholders {
        assert!(
            vellum_core::ops::find(&doc, &p.frame).is_none(),
            "both lanes' frames are gone"
        );
    }
    assert!(
        vellum_core::ops::find(&doc, &vellum_types::NodeId::new("frameExtrn1").unwrap()).is_some()
    );
    assert_eq!(
        vellum_core::ops::find(&doc, &vellum_types::NodeId::new("frameHero1").unwrap())
            .unwrap()
            .name(),
        "TEST DATA: Hero, renamed"
    );
    assert_ne!(doc, before, "the others' changes stay");
    let undo = rig.events().into_iter().find_map(|e| match e.body {
        EventBody::Undo { changed, kept } => Some((changed, kept, e.origin)),
        _ => None,
    });
    let (changed, kept, origin) = undo.expect("Undone by you is in the session");
    assert_eq!((changed, kept), (2, 0));
    assert!(origin.actor.is_person());
    assert_eq!(origin.turn.as_ref(), Some(&rig.spec.turn));
}

#[test]
fn undo_this_turn_after_someone_built_on_it_is_a_new_revert_that_keeps_their_work() {
    let mut rig = rig(vec![]);
    let placeholders = split_two(&mut rig, &["Welcome", "Step 1"]);
    rig.teal.queue(vec![insert(&placeholders[0])]);
    rig.saffron.queue(vec![insert(&placeholders[1])]);
    let template = rig.request("build");
    let orchestrator = orchestrator!(rig);
    block_on(orchestrator.run_split(&mut rig.daemon.docs, &rig.spec, template)).unwrap();
    drop(orchestrator);
    // The person renames Saffron's frame: they built on the turn.
    let person = vellum_types::Origin::new(rig.spec.person.clone());
    rig.daemon
        .docs
        .target(PATH)
        .unwrap()
        .transact(vellum_core::ops::Transaction::new(
            person,
            vec![vellum_core::ops::Op::Update {
                id: placeholders[1].frame.clone(),
                patch: serde_json::from_value(json!({"name": "TEST DATA: mine now"})).unwrap(),
            }],
        ))
        .unwrap();
    let orchestrator = orchestrator!(rig);
    let undone = orchestrator
        .undo_turn(&mut rig.daemon.docs, &rig.spec)
        .unwrap();
    drop(orchestrator);
    let TurnUndone::Reverted { kept, .. } = undone else {
        panic!("{undone:?}")
    };
    assert!(!kept.is_empty());
    let doc = rig.doc();
    assert!(vellum_core::ops::find(&doc, &placeholders[0].frame).is_none());
    assert_eq!(
        vellum_core::ops::find(&doc, &placeholders[1].frame).map(|n| n.name().to_owned()),
        Some("TEST DATA: mine now".to_owned()),
        "the person's change is never dropped"
    );
}
