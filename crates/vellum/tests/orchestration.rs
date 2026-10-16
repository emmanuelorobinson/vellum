//! TEST DATA. Plans, placeholders and leases in orchestration (guide Prompt 38; ARCHITECTURE §7.5): a fake engine
//! driven through the real tool registry, scene and daemon documents, with the session log in memory.
mod common;

use std::pin::pin;
use std::task::{Context, Poll, Waker};

use common::{Daemon, design, encode};
use serde_json::{Value, json};
use vellum::orchestrate::{OrchestrationError, Orchestrator, TurnSpec};
use vellum::sessions::{SessionLog, pricing};
use vellum::tools::{Client, Registry, RegistryBridge, Session};
use vellum_contracts::fakes::{FakeEngine, FakeStep, MemorySessions};
use vellum_contracts::suites::model::block_on;
use vellum_core::plans::{PlanEvent, PlanSettings, PlanStatus, plan_state};
use vellum_core::ports::*;
use vellum_core::turns::{TurnInput, TurnState};
use vellum_types::{Actor, AgentId, DeviceId, SessionId, TurnId};

const PATH: &str = "design/app.vellum";
const HOME: &str = "pageHome01";

fn ulid(n: u32) -> String {
    format!("01J{n:023}")
}

struct Rig {
    daemon: Daemon,
    log: SessionLog,
    registry: Registry,
    session: Session,
    spec: TurnSpec,
    engine: FakeEngine,
}

fn rig(scripts: Vec<Vec<FakeStep>>) -> Rig {
    let daemon = Daemon::with_scene(&design(), &[PATH]);
    let log = SessionLog::new(Box::new(MemorySessions::new()), pricing());
    let id = SessionId::new(ulid(1)).unwrap();
    let agent = AgentId::new(ulid(2)).unwrap();
    log.store()
        .create(NewSession {
            id: id.clone(),
            doc: DocKey::new("/repo", PATH).unwrap(),
            kind: SessionKind::Engine {
                engine: "fake".into(),
            },
            agent: agent.clone(),
            title: "TEST DATA: Onboarding".into(),
            created_at: Timestamp(0),
        })
        .unwrap();
    let turn = TurnId::new(ulid(3)).unwrap();
    let session = Session::for_agent("Teal", agent, id.clone());
    let spec = TurnSpec {
        session: id,
        turn,
        person: Actor::Person {
            device: DeviceId::new(ulid(4)).unwrap(),
        },
        document: PATH.into(),
    };
    Rig {
        daemon,
        log,
        registry: Registry::standard(),
        session,
        spec,
        engine: FakeEngine::new(scripts),
    }
}

impl Rig {
    fn request(&self, prompt: &str) -> TurnRequest {
        TurnRequest::new(self.spec.turn.clone(), "fake-model", prompt)
    }
    fn start(&mut self, prompt: &str) -> Result<TurnState, OrchestrationError> {
        let request = self.request(prompt);
        let orchestrator = Orchestrator::new(&self.engine, &self.registry, &self.log);
        block_on(orchestrator.start(
            &mut self.daemon.docs,
            &mut self.session,
            &self.spec,
            request,
        ))
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
    fn plan_events(&self) -> Vec<PlanEvent> {
        self.events()
            .into_iter()
            .filter_map(|e| match e.body {
                EventBody::Plan(p) => Some(p),
                _ => None,
            })
            .collect()
    }
    /// What the tool replied to the `n`th call the engine made, from the messages the session log keeps.
    fn replies(&self) -> Vec<(String, Value, bool)> {
        self.events()
            .iter()
            .filter_map(|e| match &e.body {
                EventBody::Messages(m) => Some(m.clone()),
                _ => None,
            })
            .flatten()
            .flat_map(|m| m.parts)
            .filter_map(|part| match part {
                StoredPart::ToolResult {
                    name,
                    result,
                    failed,
                    ..
                } => Some((name, result, failed)),
                _ => None,
            })
            .collect()
    }
    /// Another agent, connected over MCP, calls a tool.
    fn other(&mut self, session: &mut Session, tool: &str, input: Value) -> Value {
        let bridge = RegistryBridge::new(&self.registry, &mut self.daemon.docs, session);
        let reply = vellum_core::ports::ToolBridge::call(&bridge, tool, input);
        assert!(!reply.failed, "{tool}: {}", reply.value);
        reply.value
    }
    fn file_bytes(&self) -> Vec<u8> {
        self.daemon.fs.read(PATH).unwrap()
    }
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

fn frame(id: &str, rect: &Value) -> Value {
    json!({"parent": HOME, "nodes": [{
        "type": "frame", "id": id, "name": "TEST DATA screen",
        "position": {"type": "absolute", "x": rect["x"], "y": rect["y"]},
        "width": rect["width"], "height": rect["height"],
    }]})
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn json_rect(value: &Value) -> Rect {
    Rect::new(
        value["x"].as_f64().unwrap(),
        value["y"].as_f64().unwrap(),
        value["width"].as_f64().unwrap(),
        value["height"].as_f64().unwrap(),
    )
}

/// The page's existing frames: Hero and Footer.
fn existing() -> [Rect; 2] {
    [
        Rect::new(0.0, 0.0, 390.0, 300.0),
        Rect::new(0.0, 400.0, 390.0, 100.0),
    ]
}

// ---- planning ----------------------------------------------------------------------------------------------------

#[test]
fn a_request_over_the_threshold_waits_with_a_plan_and_changes_nothing() {
    let mut rig = rig(vec![vec![
        call("get_app_state", json!({})),
        call("propose_plan", screens(&["Welcome", "Step 1"])),
        call(
            "insert_nodes",
            frame(
                "frameEarly1",
                &json!({"x": 900, "y": 0, "width": 10, "height": 10}),
            ),
        ),
    ]]);
    let before = rig.file_bytes();
    assert_eq!(
        rig.start("Design the onboarding screens").unwrap(),
        TurnState::AwaitingPlan
    );
    assert_eq!(
        TurnState::AwaitingPlan.dot(),
        vellum_core::turns::SessionDot::NeedsYou
    );

    assert_eq!(
        rig.transitions(),
        [
            (TurnInput::Start, TurnState::Planning, true),
            (TurnInput::PlanNeeded, TurnState::AwaitingPlan, false),
        ]
    );
    let events = rig.plan_events();
    let [PlanEvent::Proposed(plan)] = events.as_slice() else {
        panic!("one proposal: {:?}", rig.plan_events());
    };
    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.items.iter().map(|i| i.id).collect::<Vec<_>>(), [1, 2]);
    assert!(
        plan.items.iter().all(|i| i.page.as_str() == HOME),
        "page names resolve to ids"
    );
    assert_eq!(plan.items[1].intent, "TEST DATA: the Step 1 screen");

    let replies = rig.replies();
    assert_eq!(replies[1].0, "propose_plan");
    assert_eq!(replies[1].1["plan"], "proposed");
    let (tool, refused, failed) = &replies[2];
    assert_eq!(tool, "insert_nodes");
    assert!(*failed, "nothing is built before Build: {refused}");
    assert_eq!(refused["error"]["kind"], "refused");

    assert_eq!(rig.file_bytes(), before);
    let snapshot = rig.daemon.docs.target(PATH).unwrap().snapshot().unwrap();
    assert_eq!(snapshot, design());
    let prompt = rig.events().into_iter().find_map(|e| match e.body {
        EventBody::Prompt { text, .. } => Some((text, e.origin.actor.is_person())),
        _ => None,
    });
    assert_eq!(prompt, Some(("Design the onboarding screens".into(), true)));
}

#[test]
fn a_plan_under_the_threshold_builds_at_once() {
    let mut rig = rig(vec![vec![
        call("propose_plan", screens(&["Welcome"])),
        call(
            "insert_nodes",
            frame(
                "frameSolo01",
                &json!({"x": 900, "y": 0, "width": 390, "height": 844}),
            ),
        ),
    ]]);
    assert_eq!(
        rig.start("Design a welcome screen").unwrap(),
        TurnState::Done
    );
    assert_eq!(
        rig.transitions(),
        [
            (TurnInput::Start, TurnState::Planning, true),
            (TurnInput::NoPlanNeeded, TurnState::Running, false),
            (TurnInput::Done, TurnState::Done, false),
        ]
    );
    assert!(
        rig.plan_events().is_empty(),
        "no plan event under the threshold"
    );
    assert_eq!(rig.replies()[0].1["plan"], "not_needed");
    assert!(!rig.replies()[1].2, "the write went through");
}

#[test]
fn an_engine_that_builds_without_proposing_needs_no_plan() {
    let mut rig = rig(vec![vec![call(
        "insert_nodes",
        frame(
            "frameSolo01",
            &json!({"x": 900, "y": 0, "width": 10, "height": 10}),
        ),
    )]]);
    assert_eq!(rig.start("Add a small frame").unwrap(), TurnState::Done);
    assert_eq!(
        rig.transitions()[1],
        (TurnInput::NoPlanNeeded, TurnState::Running, false)
    );

    // A turn that only reads also ends cleanly.
    let mut quiet = self::rig(vec![vec![call("get_app_state", json!({}))]]);
    assert_eq!(
        quiet.start("What is on the page?").unwrap(),
        TurnState::Done
    );
}

#[test]
fn the_threshold_is_a_setting() {
    let mut rig = rig(vec![vec![call("propose_plan", screens(&["A", "B", "C"]))]]);
    let request = rig.request("Three screens");
    let orchestrator =
        Orchestrator::new(&rig.engine, &rig.registry, &rig.log).with_settings(PlanSettings {
            min_frames: 4,
            ..PlanSettings::default()
        });
    let state =
        block_on(orchestrator.start(&mut rig.daemon.docs, &mut rig.session, &rig.spec, request));
    assert_eq!(state.unwrap(), TurnState::Done);
    assert!(rig.plan_events().is_empty());
}

#[test]
fn a_bad_plan_is_refused_so_the_engine_can_fix_it() {
    let mut no_page = screens(&["A", "B"]);
    no_page["items"][1]["page"] = json!("Nowhere");
    let mut no_size = screens(&["A", "B"]);
    no_size["items"][0]["width"] = Value::Null;
    let mut not_a_frame = screens(&["A", "B"]);
    not_a_frame["items"][0]["frame"] = json!("textTitle1");
    let mut rig = rig(vec![vec![
        call("propose_plan", no_page),
        call("propose_plan", no_size),
        call("propose_plan", not_a_frame),
        call("propose_plan", screens(&["A", "B"])),
    ]]);
    assert_eq!(rig.start("Two screens").unwrap(), TurnState::AwaitingPlan);
    let replies = rig.replies();
    assert!(replies[..3].iter().all(|(_, _, failed)| *failed));
    let message = |n: usize| {
        replies[n].1["error"]["message"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert!(
        message(0).contains("There is no page \"Nowhere\""),
        "{}",
        message(0)
    );
    assert!(message(0).contains("Home"), "{}", message(0));
    assert!(message(1).contains("width and height"), "{}", message(1));
    assert!(message(2).contains("not a frame"), "{}", message(2));
    assert_eq!(rig.plan_events().len(), 1);
}

#[test]
fn edit_request_plans_again() {
    let mut rig = rig(vec![
        vec![call("propose_plan", screens(&["Welcome", "Step 1"]))],
        vec![call(
            "propose_plan",
            screens(&["Welcome", "Step 1", "Step 2"]),
        )],
    ]);
    assert_eq!(
        rig.start("Design onboarding").unwrap(),
        TurnState::AwaitingPlan
    );
    let request = rig.request("Add a third step");
    let orchestrator = Orchestrator::new(&rig.engine, &rig.registry, &rig.log);
    let state = block_on(orchestrator.edit_request(
        &mut rig.daemon.docs,
        &mut rig.session,
        &rig.spec,
        request,
    ));
    drop(orchestrator);
    assert_eq!(state.unwrap(), TurnState::AwaitingPlan);
    assert_eq!(
        rig.transitions()[2..],
        [
            (TurnInput::EditRequest, TurnState::Planning, true),
            (TurnInput::PlanNeeded, TurnState::AwaitingPlan, false),
        ]
    );
    let state = plan_state(&rig.events(), &rig.spec.turn).unwrap();
    assert_eq!(state.plan.items.len(), 3);
    assert_eq!(state.status, PlanStatus::Proposed);

    let requests = rig.engine.requests();
    assert_eq!(requests[1].prompt, "Add a third step");
    assert!(
        !requests[1].history.is_empty(),
        "the second planning run sees the first one"
    );
}

// ---- Build and placeholders --------------------------------------------------------------------------------------

fn planned(rig: &mut Rig) -> Vec<vellum_core::plans::Placeholder> {
    assert_eq!(
        rig.start("Design onboarding").unwrap(),
        TurnState::AwaitingPlan
    );
    let orchestrator = Orchestrator::new(&rig.engine, &rig.registry, &rig.log);
    let placeholders = orchestrator.build(&mut rig.daemon.docs, &rig.spec, None);
    drop(orchestrator);
    placeholders.unwrap()
}

#[test]
fn build_places_placeholders_that_every_agent_sees_and_find_space_avoids() {
    let mut rig = rig(vec![vec![call(
        "propose_plan",
        screens(&["Welcome", "Step 1", "Step 2"]),
    )]]);
    let before = rig.file_bytes();
    let placeholders = planned(&mut rig);
    assert_eq!(placeholders.len(), 3);
    assert_eq!(
        rig.transitions()[2],
        (TurnInput::Build, TurnState::Running, true)
    );

    // Planned bounds come from find_space: clear of the page's frames and of each other.
    for (n, p) in placeholders.iter().enumerate() {
        assert_eq!((p.bounds.width, p.bounds.height), (390.0, 844.0));
        assert_eq!(p.item, n as u32 + 1);
        assert_eq!(p.page.as_str(), HOME);
        assert_eq!(&p.agent, rig.session.agent().unwrap());
        for frame in existing() {
            assert!(!overlaps(&p.bounds, &frame), "{p:?} on a frame");
        }
        for q in &placeholders[n + 1..] {
            assert!(!overlaps(&p.bounds, &q.bounds), "{p:?} on {q:?}");
        }
    }
    let events = rig.plan_events();
    let [
        _,
        PlanEvent::Built {
            placeholders: logged,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("proposal then build: {:?}", rig.plan_events());
    };
    assert_eq!(logged, &placeholders);

    // Another agent sees them, names and all, and find_space keeps clear of them.
    let mut other = Session::default();
    other.client = Some(Client {
        name: "Claude Code".into(),
        version: String::new(),
    });
    let state = rig.other(&mut other, "get_app_state", json!({"document": PATH}));
    let seen = state["placeholders"].as_array().unwrap();
    assert_eq!(seen.len(), 3);
    assert_eq!(seen[0]["frame"], placeholders[0].frame.as_str());
    assert_eq!(seen[0]["agent"], placeholders[0].agent.as_str());
    assert_eq!(seen[0]["name"], "Welcome");
    assert_eq!(seen[0]["agentName"], "Teal");
    assert_eq!(seen[0]["page"], HOME);
    assert_eq!(seen[0]["item"], 1);
    assert_eq!(seen[0]["filling"], false);
    assert_eq!(json_rect(&seen[0]["bounds"]), placeholders[0].bounds);

    let space = rig.other(
        &mut other,
        "find_space",
        json!({"document": PATH, "width": 390, "height": 844}),
    );
    let free = json_rect(&space["rect"]);
    for p in &placeholders {
        assert!(
            !overlaps(&free, &p.bounds),
            "find_space chose a planned spot: {free:?}"
        );
    }

    // Placeholders are session state: the file and the document are as they were.
    assert_eq!(rig.file_bytes(), before);
    rig.daemon.docs.save(PATH).unwrap();
    assert_eq!(rig.file_bytes(), before);
    assert_eq!(
        rig.daemon.docs.target(PATH).unwrap().snapshot().unwrap(),
        design()
    );
}

#[test]
fn the_person_can_edit_the_plan_before_build() {
    let mut rig = rig(vec![vec![call(
        "propose_plan",
        screens(&["Welcome", "Step 1", "Step 2"]),
    )]]);
    assert_eq!(
        rig.start("Design onboarding").unwrap(),
        TurnState::AwaitingPlan
    );
    let mut edited = plan_state(&rig.events(), &rig.spec.turn).unwrap().plan;
    edited.items.remove(1);
    edited.items[1].name = "Last step".into();
    let orchestrator = Orchestrator::new(&rig.engine, &rig.registry, &rig.log);

    // An edit that breaks the plan is refused and changes nothing: the plan still waits.
    let mut broken = edited.clone();
    broken.items[0].width = None;
    let error = orchestrator.build(&mut rig.daemon.docs, &rig.spec, Some(broken));
    assert!(
        matches!(error, Err(OrchestrationError::Plan(_))),
        "{error:?}"
    );
    let mut elsewhere = edited.clone();
    elsewhere.items[0].page = vellum_types::PageId::new("pageGone01").unwrap();
    let error = orchestrator.build(&mut rig.daemon.docs, &rig.spec, Some(elsewhere));
    assert!(
        matches!(error, Err(OrchestrationError::Tool(_))),
        "{error:?}"
    );
    assert!(rig.daemon.docs.placeholders(PATH).is_empty());
    assert_eq!(
        plan_state(&rig.events(), &rig.spec.turn).unwrap().status,
        PlanStatus::Proposed
    );

    let placeholders = orchestrator
        .build(&mut rig.daemon.docs, &rig.spec, Some(edited.clone()))
        .unwrap();
    assert_eq!(
        placeholders.iter().map(|p| p.item).collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(placeholders[1].name, "Last step");
    let state = plan_state(&rig.events(), &rig.spec.turn).unwrap();
    assert_eq!(state.plan, edited);
}

#[test]
fn build_is_refused_unless_a_plan_waits() {
    let mut rig = rig(vec![vec![call(
        "insert_nodes",
        frame(
            "frameSolo01",
            &json!({"x": 900, "y": 0, "width": 10, "height": 10}),
        ),
    )]]);
    assert_eq!(rig.start("Add a frame").unwrap(), TurnState::Done);
    let orchestrator = Orchestrator::new(&rig.engine, &rig.registry, &rig.log);
    let error = orchestrator.build(&mut rig.daemon.docs, &rig.spec, None);
    assert!(
        matches!(error, Err(OrchestrationError::Turn(_))),
        "{error:?}"
    );
}

#[test]
fn report_progress_ticks_items_and_frames_take_their_placeholders_place() {
    let mut rig = rig(vec![vec![call(
        "propose_plan",
        screens(&["Welcome", "Step 1"]),
    )]]);
    let placeholders = planned(&mut rig);
    let first = &placeholders[0];
    let rect = json!({"x": first.bounds.x, "y": first.bounds.y, "width": 390, "height": 844});
    rig.engine.queue(vec![
        call("insert_nodes", frame(first.frame.as_str(), &rect)),
        call("get_app_state", json!({})),
        call(
            "report_progress",
            json!({"message": "TEST DATA: Welcome is done.", "items": [1], "done": 1, "total": 2}),
        ),
        call(
            "report_progress",
            json!({"message": "TEST DATA: wrong item.", "items": [9]}),
        ),
        call("get_app_state", json!({})),
    ]);
    let request = rig.request("ignored: the build prompt replaces it");
    let orchestrator = Orchestrator::new(&rig.engine, &rig.registry, &rig.log);
    let state = block_on(orchestrator.run_build(
        &mut rig.daemon.docs,
        &mut rig.session,
        &rig.spec,
        request,
    ));
    drop(orchestrator);
    assert_eq!(state.unwrap(), TurnState::Done);

    let build = &rig.engine.requests()[1];
    assert!(
        build.prompt.contains(first.frame.as_str()),
        "{}",
        build.prompt
    );
    assert!(build.prompt.contains("report_progress"), "{}", build.prompt);

    let replies = rig.replies();
    let during: Vec<_> = replies
        .iter()
        .filter(|(tool, ..)| tool == "get_app_state")
        .collect();
    // Once its frame exists the placeholder is being filled, and the frame is leased to the building agent.
    let state = &during[0].1;
    assert_eq!(state["placeholders"][0]["filling"], true);
    assert_eq!(state["placeholders"][1]["filling"], false);
    assert!(
        state["leases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["frame"] == first.frame.as_str() && l["agent"] == first.agent.as_str()),
        "{state}"
    );
    let ticked = replies
        .iter()
        .find(|(tool, ..)| tool == "report_progress")
        .unwrap();
    assert!(!ticked.2);
    assert_eq!(ticked.1["plan"], json!({"done": 1, "total": 2}));
    let wrong = replies
        .iter()
        .filter(|(tool, ..)| tool == "report_progress")
        .nth(1)
        .unwrap();
    assert!(wrong.2, "item 9 is not in the plan");
    assert_eq!(wrong.1["error"]["message"], "The plan has no item 9.");
    // After the tick only item 2's placeholder stands.
    let after = &during[1].1["placeholders"];
    assert_eq!(after.as_array().unwrap().len(), 1);
    assert_eq!(after[0]["item"], 2);

    // The log has the tick and the finish; the plan state replays to one of two done.
    let events = rig.plan_events();
    assert_eq!(events[2], PlanEvent::Ticked(vec![1]));
    assert_eq!(events[3], PlanEvent::Finished);
    let replayed = plan_state(&rig.events(), &rig.spec.turn).unwrap();
    assert_eq!(replayed.status, PlanStatus::Finished);
    assert_eq!(replayed.progress(), (1, 2));
    assert!(
        rig.daemon.docs.placeholders(PATH).is_empty(),
        "the turn is over"
    );
    assert_eq!(rig.transitions().last().unwrap().1, TurnState::Done);
}

// ---- abandoned plans ---------------------------------------------------------------------------------------------

#[test]
fn stopping_a_waiting_plan_leaves_nothing() {
    let mut rig = rig(vec![vec![call(
        "propose_plan",
        screens(&["Welcome", "Step 1"]),
    )]]);
    let before = rig.file_bytes();
    assert_eq!(
        rig.start("Design onboarding").unwrap(),
        TurnState::AwaitingPlan
    );
    let orchestrator = Orchestrator::new(&rig.engine, &rig.registry, &rig.log);
    assert_eq!(
        orchestrator
            .stop(Some(&mut rig.daemon.docs), &rig.spec)
            .unwrap(),
        TurnState::Stopped
    );
    drop(orchestrator);
    assert_eq!(rig.plan_events().last(), Some(&PlanEvent::Abandoned));
    assert_eq!(
        rig.transitions().last(),
        Some(&(TurnInput::Stop, TurnState::Stopped, true))
    );
    assert_eq!(rig.file_bytes(), before);
}

#[test]
fn stopping_after_build_takes_the_placeholders_away_and_leaves_the_file_alone() {
    let mut rig = rig(vec![vec![call(
        "propose_plan",
        screens(&["Welcome", "Step 1"]),
    )]]);
    let before = rig.file_bytes();
    let placeholders = planned(&mut rig);
    assert_eq!(rig.daemon.docs.placeholders(PATH).len(), 2);
    rig.engine.queue(vec![FakeStep::Work]);

    let request = rig.request("build");
    {
        let orchestrator = Orchestrator::new(&rig.engine, &rig.registry, &rig.log);
        let mut run = pin!(orchestrator.run_build(
            &mut rig.daemon.docs,
            &mut rig.session,
            &rig.spec,
            request,
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(
            run.as_mut().poll(&mut cx).is_pending(),
            "the agent is working"
        );
        // A person presses Stop while the build runs; the run ends and clears up.
        assert_eq!(
            orchestrator.stop(None, &rig.spec).unwrap(),
            TurnState::Stopped
        );
        let Poll::Ready(state) = run.as_mut().poll(&mut cx) else {
            panic!("Stop ends the run");
        };
        assert_eq!(state.unwrap(), TurnState::Stopped);
    }

    assert!(rig.daemon.docs.placeholders(PATH).is_empty());
    assert_eq!(rig.plan_events().last(), Some(&PlanEvent::Abandoned));
    let view = rig.daemon.docs.view(PATH).unwrap();
    assert!(view.state.leases.is_empty());
    rig.daemon.docs.save(PATH).unwrap();
    assert_eq!(
        rig.file_bytes(),
        before,
        "an abandoned plan leaves nothing in the file"
    );
    let doc = rig.daemon.docs.target(PATH).unwrap().snapshot().unwrap();
    for p in &placeholders {
        assert!(vellum_core::ops::find(&doc, &p.frame).is_none());
    }
    assert_eq!(encode(&doc), encode(&design()));
}

#[test]
fn a_build_that_fails_abandons_its_plan() {
    let mut rig = rig(vec![vec![call(
        "propose_plan",
        screens(&["Welcome", "Step 1"]),
    )]]);
    planned(&mut rig);
    rig.engine
        .queue(vec![FakeStep::Emit(TurnEvent::Ended(TurnEnd::Failed(
            EngineError::RoundLimit,
        )))]);
    let request = rig.request("build");
    let orchestrator = Orchestrator::new(&rig.engine, &rig.registry, &rig.log);
    let state = block_on(orchestrator.run_build(
        &mut rig.daemon.docs,
        &mut rig.session,
        &rig.spec,
        request,
    ));
    drop(orchestrator);
    assert_eq!(state.unwrap(), TurnState::Failed);
    assert_eq!(rig.plan_events().last(), Some(&PlanEvent::Abandoned));
    assert!(rig.daemon.docs.placeholders(PATH).is_empty());
}
