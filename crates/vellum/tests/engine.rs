//! TEST DATA. The built-in engine end to end: an OpenAI-compatible provider over HTTP to the stub endpoint, the
//! real tool registry and scene, and the daemon's documents.
mod common;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use common::{Daemon, FakeEditor, design};
use serde_json::{Value, json};
use vellum::daemon::EditorId;
use vellum::tools::{Client, Registry, RegistryBridge, Session};
use vellum_contracts::{fakes::MemorySecrets, suites::model::block_on};
use vellum_core::ports::*;
use vellum_engine_builtin::{BuiltinEngine, PromptPack};
use vellum_testkit::model::{StubEndpoint, StubReply};
use vellum_types::{Actor, Document, TurnId};

const PATH: &str = "design/app.vellum";

fn turn() -> TurnId {
    TurnId::new("01J00000000000000000000E2E").unwrap()
}

fn call(id: &str, name: &str, arguments: Value) -> StubReply {
    StubReply::sse(
        vec![json!({
            "choices":[{"delta":{"tool_calls":[{"index":0,"id":id,"function":{"name":name,"arguments":arguments.to_string()}}]},"finish_reason":"tool_calls"}]
        })],
        true,
    )
}

fn say(text: &str) -> StubReply {
    StubReply::sse(
        vec![
            json!({"choices":[{"delta":{"content":text}}]}),
            json!({"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":1200,"completion_tokens":40}}),
        ],
        true,
    )
}

fn engine(stub: &StubEndpoint, vision: bool) -> Arc<BuiltinEngine> {
    let secrets = Arc::new(MemorySecrets::default());
    let canary = std::env::var("VELLUM_CANARY_API_KEY").ok();
    if let Some(key) = &canary {
        secrets
            .set("security-canary", SecretString::new(key))
            .unwrap();
    }
    let provider = vellum_llm_openai_compat::Provider::new(
        ModelConfig {
            id: "stub".into(),
            endpoint: stub.url("/chat"),
            secret_key: canary.map(|_| "security-canary".into()),
            caps: ModelCaps {
                tools: true,
                vision,
                reasoning: false,
                context_tokens: Some(200_000),
            },
        },
        secrets,
        Arc::new(vellum_model_http::HttpTransport::default()),
    )
    .unwrap();
    Arc::new(BuiltinEngine::new(
        Arc::new(provider),
        PromptPack::builtin(),
    ))
}

fn session() -> Session {
    let mut session = Session::default();
    session.client = Some(Client {
        name: "Teal".into(),
        version: String::new(),
    });
    session.set_turn(Some(turn()));
    session
}

fn request(prompt: &str) -> TurnRequest {
    let mut request = TurnRequest::new(turn(), "fixture-model", prompt);
    request.skills =
        vellum_core::ports::SkillSource::list(&vellum::content::BuiltinSkills).unwrap();
    request
}

fn drain(stream: &mut dyn TurnStream) -> Vec<TurnEvent> {
    let mut events = vec![];
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    events
}

fn page(doc: &Document, id: &str) -> Vec<String> {
    doc.pages
        .iter()
        .find(|p| p.id.as_str() == id)
        .unwrap()
        .children
        .iter()
        .map(|n| n.name().to_owned())
        .collect()
}

#[test]
#[ignore = "model-http: requires loopback sockets; cargo test -p vellum --test engine -- --ignored"]
fn a_single_frame_request_is_built_checked_and_reported() {
    let stub = StubEndpoint::start(vec![
        call("call1", "read_skill", json!({"path": "format"})),
        call(
            "call2",
            "insert_nodes",
            json!({"parent": "pageOther1", "nodes": [{"type": "frame", "name": "Pricing", "width": 360, "height": 480,
                "layout": "column", "gap": 16, "padding": 24, "fills": [{"type": "solid", "color": "#FFFFFF"}],
                "children": [{"type": "text", "name": "Plan", "width": "hug", "height": "hug",
                    "content": [{"text": "Pro", "font": "Inter", "size": 24}]}]}]}),
        ),
        say("Added the Pricing frame."),
        call("call3", "screenshot", json!({"node": "pageOther1"})),
        say("Checked the screenshot: the Pricing frame is in place."),
    ])
    .unwrap();
    let engine = engine(&stub, true);
    let mut daemon = Daemon::with_scene(&design(), &[PATH]);
    let registry = Registry::standard();
    let mut session = session();
    {
        let mut ctx = vellum::tools::ToolCtx::new(&mut daemon.docs, &mut session);
        registry
            .call(&mut ctx, "open_document", json!({"path": PATH}))
            .unwrap();
    }
    session.take_steps();
    let events = {
        let bridge = RegistryBridge::new(&registry, &mut daemon.docs, &mut session);
        let mut stream = engine
            .run_turn(request("Add a pricing card on the Other page"), &bridge)
            .unwrap();
        drain(&mut *stream)
    };
    assert_eq!(
        events.last(),
        Some(&TurnEvent::Ended(TurnEnd::Done)),
        "{events:?}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == TurnEvent::SelfCheck)
            .count(),
        1
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TurnEvent::Usage(u) if u.input_tokens == Some(1200)))
    );
    let doc = daemon.docs.target(PATH).unwrap().snapshot().unwrap();
    assert_eq!(page(&doc, "pageOther1"), ["Pricing"]);
    assert_eq!(stub.remaining(), 0);

    // What the model saw.
    let requests = stub.requests();
    let first = &requests[0].body;
    let system = first["messages"][0]["content"].as_str().unwrap();
    assert!(system.starts_with(PromptPack::builtin().system_prefix()));
    assert!(
        !system.contains("format.md"),
        "skill metadata stays outside system instructions"
    );
    assert!(
        first["messages"][1]["content"]
            .to_string()
            .contains("format.md"),
        "the skills are listed as task data"
    );
    let offered: Vec<_> = first["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["function"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(offered.len(), registry.tools().count());
    assert!(offered.contains(&"screenshot") && offered.contains(&"execute"));
    // The self-check, then the picture after the screenshot's tool message.
    let nudged = requests[3].body["messages"].as_array().unwrap();
    assert_eq!(
        nudged.last().unwrap()["content"][0]["text"],
        PromptPack::builtin().self_check()
    );
    let after = requests[4].body["messages"].as_array().unwrap();
    let n = after.len();
    assert_eq!(after[n - 2]["role"], "tool");
    assert!(
        after[n - 2]["content"]
            .as_str()
            .unwrap()
            .contains("[attached below as a picture]")
    );
    assert_eq!(after[n - 1]["role"], "user");
    assert!(
        after[n - 1]["content"][1]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,iVBOR"),
        "the screenshot reaches the model as a picture"
    );
    // Each call was an ordinary step in the session.
    let steps: Vec<_> = session.steps().iter().map(|s| s.tool.clone()).collect();
    assert_eq!(steps, ["read_skill", "insert_nodes", "screenshot"]);
}

#[test]
#[ignore = "model-http, native-renderer, ephemeral canary: run pnpm security:canary"]
fn scripted_session_canary() {
    let artifacts = std::env::var("VELLUM_CANARY_ARTIFACTS").expect("use the canary runner");
    let key = std::env::var("VELLUM_CANARY_API_KEY").expect("ephemeral test credential");
    // This fixture has no external fonts/resources: the resulting share is complete on its own.
    let fixture: Document = serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA canary","createdWith":"tests"},"pages":[{"id":"pageOther1","name":"Page","children":[]}]})).unwrap();
    let stub = StubEndpoint::start(vec![
        call("draw", "insert_nodes", json!({"parent":"pageOther1","nodes":[{"type":"frame","name":"Frame","width":160,"height":100,"fills":[{"type":"solid","color":"#123456"}]}]})),
        call("check", "screenshot", json!({"node":"pageOther1"})),
        // A hostile provider echo must also be scrubbed before session/log persistence.
        say(&key),
    ]).unwrap();
    let mut daemon = Daemon::with_scene(&fixture, &[PATH]);
    let registry = Registry::standard();
    let mut session = session();
    registry
        .call(
            &mut vellum::tools::ToolCtx::new(&mut daemon.docs, &mut session),
            "open_document",
            json!({"path":PATH}),
        )
        .unwrap();
    let engine = engine(&stub, true);
    let events = {
        let bridge = RegistryBridge::new(&registry, &mut daemon.docs, &mut session);
        drain(
            &mut *engine
                .run_turn(request("Draw and check a frame"), &bridge)
                .unwrap(),
        )
    };
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    assert!(
        !format!("{events:?}").contains(&key),
        "credential echo reached the session"
    );
    assert_eq!(stub.remaining(), 0);
    persist_canary_artifacts(&artifacts, &mut daemon, &mut session, &events, &stub);
}

/// TEST DATA. Real model transport, SQLite messages/cache, recorded permissions, export and share bundle.
/// No real keychain, public model, cloud account or upload is involved.
fn persist_canary_artifacts(
    artifacts: &str,
    daemon: &mut Daemon,
    session: &mut Session,
    events: &[TurnEvent],
    stub: &StubEndpoint,
) {
    use std::rc::Rc;
    use vellum_contracts::fakes::{ManualClock, ScriptedPrompt};
    use vellum_core::permissions::{Broker, PermissionSettings};
    use vellum_types::{AgentId, DeviceId, Origin, SessionId};
    let root = std::path::Path::new(artifacts).join("session");
    std::fs::create_dir_all(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let store = Rc::new(
        vellum_sessions_sqlite::SqliteSessions::open(root.join("sessions.db"), root.join("cache"))
            .unwrap(),
    );
    let id = SessionId::new(format!("01J{:023}", 69)).unwrap();
    let agent = AgentId::new(format!("01J{:023}", 70)).unwrap();
    store
        .create(NewSession {
            id: id.clone(),
            doc: DocKey::new(root.to_string_lossy(), "app.vellum").unwrap(),
            kind: SessionKind::Engine {
                engine: "builtin".into(),
            },
            agent: agent.clone(),
            title: "TEST DATA canary".into(),
            created_at: Timestamp(0),
        })
        .unwrap();
    let origin = Origin::new(Actor::Agent { agent }).in_session(id.clone());
    for event in events {
        if let TurnEvent::Messages(messages) = event {
            let messages = vellum_core::sessions::store_messages(&*store, messages).unwrap();
            store
                .append(
                    &id,
                    NewEvent {
                        at: Timestamp(1),
                        origin: origin.clone(),
                        body: EventBody::Messages(messages),
                    },
                )
                .unwrap();
        }
    }
    for step in session.steps() {
        store
            .append(
                &id,
                NewEvent {
                    at: Timestamp(1),
                    origin: origin.clone(),
                    body: EventBody::Step(vellum::sessions::step_record(step, None)),
                },
            )
            .unwrap();
    }
    let person = Actor::Person {
        device: DeviceId::new(format!("01J{:023}", 71)).unwrap(),
    };
    let broker = Broker::new(
        PermissionSettings::default(),
        store.clone(),
        Rc::new(ManualClock::new(1)),
        Some(Rc::new(ScriptedPrompt::always(Answer::AllowOnce))),
        person.clone(),
    );
    let exported = Registry::standard()
        .call(
            &mut vellum::tools::ToolCtx::new(&mut daemon.docs, session),
            "export",
            json!({"nodes":["pageOther1"],"format":"svg"}),
        )
        .unwrap();
    let path = root.join("export.json");
    let action = vellum::permissions::write_action(&path).unwrap();
    let permit = block_on(broker.request(PermissionRequest {
        origin: origin.clone(),
        action,
        engine: Some("builtin".into()),
        engine_verdict: None,
    }))
    .unwrap();
    vellum::permissions::write_file(permit, &path, &serde_json::to_vec(&exported).unwrap())
        .unwrap();
    let denied = Broker::new(
        PermissionSettings::default(),
        store.clone(),
        Rc::new(ManualClock::new(2)),
        Some(Rc::new(ScriptedPrompt::always(Answer::Deny))),
        person,
    );
    assert!(
        block_on(denied.request(PermissionRequest {
            origin,
            action: Action::Network {
                url: "https://invalid.example/collect".into()
            },
            engine: Some("builtin".into()),
            engine_verdict: None
        }))
        .is_err()
    );
    let doc = daemon.docs.target(PATH).unwrap().snapshot().unwrap();
    std::fs::write(
        root.join("app.vellum"),
        vellum_codec_json::encode(&doc).unwrap(),
    )
    .unwrap();
    let mut bundle = vellum::daemon::share::collect(daemon.fs.clone(), PATH, &doc).unwrap();
    vellum::daemon::share::render(&mut bundle, &doc).unwrap();
    std::fs::write(root.join("share.json"), bundle.bytes().unwrap()).unwrap();
    std::fs::write(
        root.join("session.log"),
        format!("{events:?}\n{:?}", stub.requests()),
    )
    .unwrap();
    assert!(
        store
            .events(&id, None)
            .unwrap()
            .iter()
            .any(|e| matches!(e.body, EventBody::Permission(_)))
    );
}

#[test]
#[ignore = "model-http: requires loopback sockets; cargo test -p vellum --test engine -- --ignored"]
fn stop_interrupts_a_slow_model_request_within_three_seconds() {
    let mut slow = say("This reply never finishes arriving.");
    slow.delay = Duration::from_secs(30);
    let stub = StubEndpoint::start(vec![slow]).unwrap();
    let engine = engine(&stub, false);
    let mut daemon = Daemon::with_scene(&design(), &[PATH]);
    let before = daemon.docs.target(PATH).unwrap().snapshot().unwrap();
    let registry = Registry::standard();
    let mut session = session();
    let stopper = {
        let engine = engine.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            let at = Instant::now();
            engine.interrupt(&turn());
            at
        })
    };
    let events = {
        let bridge = RegistryBridge::new(&registry, &mut daemon.docs, &mut session);
        let mut stream = engine.run_turn(request("Build a card"), &bridge).unwrap();
        drain(&mut *stream)
    };
    let asked = stopper.join().unwrap();
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Stopped)));
    assert!(asked.elapsed() < Duration::from_secs(3));
    // The request itself was dropped: the endpoint sees the connection close.
    while stub.closed() == 0 {
        assert!(
            asked.elapsed() < Duration::from_secs(3),
            "the request was not interrupted"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(session.steps().is_empty());
    assert_eq!(
        daemon.docs.target(PATH).unwrap().snapshot().unwrap(),
        before
    );
}

#[test]
#[ignore = "model-http: requires loopback sockets; cargo test -p vellum --test engine -- --ignored"]
fn the_turns_writes_carry_its_turn_and_agent() {
    let stub = StubEndpoint::start(vec![
        call(
            "call1",
            "insert_nodes",
            json!({"parent": "pageOther1", "nodes": [{"type": "frame", "name": "Card", "width": 320, "height": 200}]}),
        ),
        say("Added the card."),
        // The self-check: the editor draws the page.
        call("call2", "screenshot", json!({"node": "pageOther1"})),
        say("Checked: the card is there."),
    ])
    .unwrap();
    let engine = engine(&stub, false);
    let mut daemon = Daemon::new(&design(), &[PATH]);
    let editor = FakeEditor::new(&design());
    daemon
        .docs
        .open_editor(PATH, EditorId("tab-1".into()), editor.link())
        .unwrap();
    let registry = Registry::standard();
    let mut session = session();
    let events = {
        let bridge = RegistryBridge::new(&registry, &mut daemon.docs, &mut session);
        let mut stream = engine.run_turn(request("Add a card"), &bridge).unwrap();
        drain(&mut *stream)
    };
    assert_eq!(
        events.last(),
        Some(&TurnEvent::Ended(TurnEnd::Done)),
        "{events:?}"
    );
    assert!(events.contains(&TurnEvent::SelfCheck));
    let origins = editor.0.borrow().origins.clone();
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].turn, Some(turn()));
    assert_eq!(origins[0].session.as_ref(), session.id());
    assert!(matches!(&origins[0].actor, Actor::Agent { agent } if Some(agent) == session.agent()));
    assert_eq!(
        daemon
            .docs
            .agents()
            .get(session.agent().unwrap())
            .map(String::as_str),
        Some("Teal")
    );
}
