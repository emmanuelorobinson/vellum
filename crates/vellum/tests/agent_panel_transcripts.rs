//! TEST DATA. The Agent panel's browser tests (Prompt 49) replay what the installed engines of Prompts 36–37 make
//! of their recorded transcripts. Each transcript runs through its real decoder, and every event the session log
//! keeps is recorded through `SessionLog`, exactly as the daemon records a turn. The result, with the stream of text
//! and tool events the panel shows while the turn runs, is written to `fixtures/agent-panel/<engine>.json` in the
//! session codec's JSON. Rewrite the files with `VELLUM_UPDATE_SNAPSHOTS=1 cargo test -p vellum --test
//! agent_panel_transcripts`; otherwise this test fails when they differ.
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use vellum::sessions::{SessionLog, pricing};
use vellum_contracts::{
    fakes::{MemorySessions, RecordingBridge},
    suites::model::block_on,
};
use vellum_core::ports::*;
use vellum_core::sessions::{body_to_json, origin_to_json, record_transition};
use vellum_core::turns::TurnInput;
use vellum_types::{Actor, AgentId, DeviceId, Origin, SessionId, TurnId};

const SESSION: &str = "01J00000000000000000000S01";
const TURN: &str = "01J00000000000000000000T01";
const PERSON: &str = "01J00000000000000000000D01";
const START: i64 = 1_792_160_000_000;

/// Hands the decoder one recorded line at a time, then ends.
struct Tape(VecDeque<String>);
impl Tape {
    fn next_line(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>> {
        let line = self.0.pop_front();
        Box::pin(async move { Ok(line) })
    }
}
impl vellum_engine_claude_code::Process for Tape {
    fn next(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>> {
        self.next_line()
    }
}
impl vellum_engine_codex::Process for Tape {
    fn next(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>> {
        self.next_line()
    }
    fn send(&mut self, _: Value) -> EngineFuture<'_, Result<(), EngineError>> {
        Box::pin(async { Ok(()) })
    }
}

/// No canvas tools are offered: a recorded transcript's tool calls arrive through MCP, never replayed as edits.
struct NoTools;
impl McpChannel for NoTools {
    fn config(&self) -> Value {
        json!({"mcpServers":{"vellum":{"command":"vellum","args":["mcp"]}}})
    }
    fn poll_call(&mut self, _: &mut Context<'_>) -> Poll<Option<McpCall>> {
        Poll::Pending
    }
}
impl McpHost for NoTools {
    fn open(&self, _: &[BridgeTool]) -> Result<Box<dyn McpChannel>, EngineError> {
        Ok(Box::new(NoTools))
    }
}

struct Claude(Mutex<Vec<String>>);
impl vellum_engine_claude_code::Launcher for Claude {
    fn inspect(&self) -> vellum_engine_claude_code::Installation {
        vellum_engine_claude_code::Installation {
            version: Some("2.1.301".into()),
            status: EngineStatus::Ready,
        }
    }
    fn start(
        &self,
        _: vellum_engine_claude_code::Launch,
        _: Cancellation,
    ) -> Result<Box<dyn vellum_engine_claude_code::Process>, EngineError> {
        Ok(Box::new(Tape(self.0.lock().unwrap().clone().into())))
    }
}

struct Codex(Vec<String>);
impl vellum_engine_codex::Launcher for Codex {
    fn inspect(&self) -> vellum_engine_codex::Installation {
        vellum_engine_codex::Installation {
            version: Some("0.158.0".into()),
            status: EngineStatus::Ready,
            mode: Some(vellum_engine_codex::Mode::Exec),
        }
    }
    fn start(
        &self,
        _: vellum_engine_codex::Launch,
        _: Cancellation,
    ) -> Result<Box<dyn vellum_engine_codex::Process>, EngineError> {
        Ok(Box::new(Tape(self.0.clone().into())))
    }
}

fn lines(transcript: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(transcript);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn run(engine: &dyn AgentEngine, model: &str, prompt: &str) -> Vec<TurnEvent> {
    let request = TurnRequest::new(TurnId::new(TURN).unwrap(), model, prompt);
    let bridge = RecordingBridge::probe();
    let mut stream = engine.run_turn(request, &bridge).unwrap();
    let mut events = vec![];
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    events
}

/// The turn as the daemon records it: the person's prompt, Start and NoPlanNeeded, then what the engine reported.
fn record(
    engine: &str,
    agent: &str,
    model: &str,
    prompt: &str,
    transcript: &str,
    turn: &[TurnEvent],
) -> Value {
    let session = SessionId::new(SESSION).unwrap();
    let log = SessionLog::new(Box::new(MemorySessions::new()), pricing());
    log.store()
        .create(NewSession {
            id: session.clone(),
            doc: DocKey::new("/TEST DATA/juniper", "juniper.vellum").unwrap(),
            kind: SessionKind::Engine {
                engine: engine.into(),
            },
            agent: AgentId::new(agent).unwrap(),
            title: "Onboarding screens".into(),
            created_at: Timestamp(START),
        })
        .unwrap();
    let in_turn = |actor| {
        Origin::new(actor)
            .in_session(session.clone())
            .in_turn(TurnId::new(TURN).unwrap())
    };
    let person = in_turn(Actor::Person {
        device: DeviceId::new(PERSON).unwrap(),
    });
    let agent_origin = in_turn(Actor::Agent {
        agent: AgentId::new(agent).unwrap(),
    });
    let mut at = START;
    let mut tick = || {
        at += 1_000;
        Timestamp(at)
    };
    log.store()
        .append(
            &session,
            NewEvent {
                at: tick(),
                origin: person.clone(),
                body: EventBody::Prompt {
                    text: prompt.into(),
                    context: vec!["DESIGN.md".into()],
                    images: vec![],
                },
            },
        )
        .unwrap();
    record_transition(log.store(), &person, TurnInput::Start, tick()).unwrap();
    record_transition(log.store(), &agent_origin, TurnInput::NoPlanNeeded, tick()).unwrap();
    let mut live = vec![];
    for event in turn {
        match event {
            TurnEvent::Text(text) => live.push(json!({"type": "text", "text": text})),
            TurnEvent::ToolStarted {
                call,
                name,
                arguments,
            } => live.push(
                json!({"type": "toolStarted", "call": call, "name": name, "arguments": arguments}),
            ),
            TurnEvent::ToolFinished {
                call, name, failed, ..
            } => live.push(
                json!({"type": "toolFinished", "call": call, "name": name, "failed": failed}),
            ),
            _ => {}
        }
        log.record_turn_event(&agent_origin, model, event, tick())
            .unwrap();
    }
    let events: Vec<Value> = log
        .store()
        .events(&session, None)
        .unwrap()
        .iter()
        .map(|e| {
            json!({
                "seq": e.seq.0,
                "at": e.at.0,
                "origin": origin_to_json(&e.origin),
                "body": body_to_json(&e.body),
            })
        })
        .collect();
    json!({
        "_note": "TEST DATA. Generated by crates/vellum/tests/agent_panel_transcripts.rs; never edit by hand.",
        "engine": engine,
        "transcript": transcript,
        "session": {
            "id": SESSION,
            "kind": {"type": "engine", "engine": engine},
            "agent": agent,
            "title": "Onboarding screens",
            "createdAt": START,
        },
        "live": live,
        "events": events,
    })
}

fn check(name: &str, actual: &Value) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/agent-panel")
        .join(format!("{name}.json"));
    let text = serde_json::to_string_pretty(actual).unwrap() + "\n";
    if std::env::var_os("VELLUM_UPDATE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, &text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&file).unwrap_or_else(|_| {
        panic!(
            "{} is missing; run with VELLUM_UPDATE_SNAPSHOTS=1",
            file.display()
        )
    });
    assert!(
        text == expected,
        "{} differs from what the engine makes of its transcript; review and rewrite it",
        file.display()
    );
}

const PROMPT: &str = "Design the onboarding screens in @DESIGN.md using Starter Minimal.";

#[test]
fn claude_code_transcript_as_the_agent_panel_sees_it() {
    let transcript = "fixtures/engines/claude-code/2.1.301/success.recorded.ndjson";
    let engine = vellum_engine_claude_code::ClaudeEngine::new(
        Arc::new(Claude(Mutex::new(lines(transcript)))),
        Arc::new(NoTools),
    );
    let turn = run(&engine, "sonnet", PROMPT);
    assert_eq!(turn.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    let fixture = record(
        "claude-code",
        "01J00000000000000000000A01",
        "sonnet",
        PROMPT,
        transcript,
        &turn,
    );
    check("claude-code", &fixture);
}

#[test]
fn codex_transcript_as_the_agent_panel_sees_it() {
    let transcript = "fixtures/engines/codex/0.158.0/exec-success.recorded.ndjson";
    let engine = vellum_engine_codex::CodexEngine::new(
        Arc::new(Codex(lines(transcript))),
        Arc::new(NoTools),
    );
    let turn = run(&engine, "fixture-model", PROMPT);
    assert_eq!(turn.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    let fixture = record(
        "codex",
        "01J00000000000000000000A02",
        "fixture-model",
        PROMPT,
        transcript,
        &turn,
    );
    check("codex", &fixture);
}
