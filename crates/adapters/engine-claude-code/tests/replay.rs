//! TEST DATA. Protocol replays need no installed engine, account or network.
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use vellum_contracts::{
    fakes::RecordingBridge,
    suites::{engine, model::block_on},
};
use vellum_core::ports::*;
use vellum_engine_claude_code::*;

struct Replay {
    lines: Mutex<Vec<String>>,
    status: EngineStatus,
}
impl Launcher for Replay {
    fn inspect(&self) -> Installation {
        Installation {
            version: Some("2.1.301".into()),
            status: self.status,
        }
    }
    fn start(&self, _: Launch, cancel: Cancellation) -> Result<Box<dyn Process>, EngineError> {
        let lines = self.lines.lock().unwrap().clone();
        let stall = lines.is_empty();
        Ok(Box::new(Tape {
            lines: lines.into(),
            cancel,
            stall,
        }))
    }
}
struct Tape {
    lines: VecDeque<String>,
    cancel: Cancellation,
    stall: bool,
}
impl Process for Tape {
    fn next(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>> {
        Box::pin(std::future::poll_fn(|cx| {
            if self.cancel.poll_cancelled(cx).is_ready() {
                return Poll::Ready(Ok(None));
            }
            if let Some(line) = self.lines.pop_front() {
                Poll::Ready(Ok(Some(line)))
            } else if self.stall {
                Poll::Pending
            } else {
                Poll::Ready(Ok(None))
            }
        }))
    }
}
struct Host {
    probe: bool,
}
impl McpHost for Host {
    fn open(&self, _: &[BridgeTool]) -> Result<Box<dyn McpChannel>, EngineError> {
        Ok(Box::new(Channel { probe: self.probe }))
    }
}
struct Channel {
    probe: bool,
}
impl McpChannel for Channel {
    fn config(&self) -> Value {
        json!({"mcpServers":{"vellum":{"command":"vellum","args":["mcp"]}}})
    }
    fn poll_call(&mut self, _: &mut Context<'_>) -> Poll<Option<McpCall>> {
        if std::mem::take(&mut self.probe) {
            Poll::Ready(Some(McpCall {
                name: "probe".into(),
                input: json!({"value":1}),
                respond: Box::new(|_| {}),
            }))
        } else {
            Poll::Pending
        }
    }
}
fn make(text: &str, probe: bool) -> ClaudeEngine {
    ClaudeEngine::new(
        Arc::new(Replay {
            lines: Mutex::new(text.lines().map(str::to_owned).collect()),
            status: EngineStatus::Ready,
        }),
        Arc::new(Host { probe }),
    )
}
fn request() -> TurnRequest {
    TurnRequest::new(
        vellum_types::TurnId::new("01J00000000000000000000T01").unwrap(),
        "sonnet",
        "TEST DATA probe",
    )
}
fn drain(engine: &ClaudeEngine) -> Vec<TurnEvent> {
    let bridge = RecordingBridge::probe();
    let mut stream = engine.run_turn(request(), &bridge).unwrap();
    let mut events = vec![];
    while let Some(e) = block_on(stream.next()) {
        events.push(e);
    }
    events
}
#[test]
fn adapter_passes_shared_engine_contract() {
    engine::agent_engine(&|scenario| {
        Arc::new(make(
            if scenario == engine::Scenario::CallProbe {
                "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"Done\",\"usage\":{\"input_tokens\":2,\"output_tokens\":1}}"
            } else {
                ""
            },
            scenario == engine::Scenario::CallProbe,
        ))
    });
}
#[test]
fn replay_text_tools_and_inclusive_cache_usage_without_duplicate_text() {
    let engine = make(
        include_str!("../../../../fixtures/engines/claude-code/2.1.301/success.synthetic.ndjson"),
        true,
    );
    let events = drain(&engine);
    assert_eq!(
        events
            .iter()
            .filter_map(|e| if let TurnEvent::Text(t) = e {
                Some(t.as_str())
            } else {
                None
            })
            .collect::<String>(),
        "Done"
    );
    assert!(events.iter().any(|e| matches!(e,TurnEvent::Usage(u) if u.input_tokens==Some(32) && u.output_tokens==Some(4) && u.cached_input_tokens==Some(20) && u.cache_write_tokens==Some(10))));
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
}
#[test]
fn authentication_and_usage_limit_are_explicit_and_recheckable() {
    for (line, status) in [
        (
            r#"{"type":"assistant","error":"authentication_failed","message":{"content":[{"type":"text","text":"native diagnostic must not escape"}]}}"#,
            EngineStatus::SignedOut,
        ),
        (
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1792188000}}"#,
            EngineStatus::UsageLimit,
        ),
    ] {
        let engine = make(line, false);
        let events = drain(&engine);
        assert_eq!(engine.availability(), status);
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Ended(TurnEnd::Failed(_)))
        ));
        assert!(!format!("{events:?}").contains("native diagnostic"));
        assert_eq!(engine.recheck().status, EngineStatus::Ready);
    }
}
#[test]
fn eof_malformed_and_unknown_lifecycle_are_not_fabricated_success() {
    for line in [
        r#"{"type":"system","subtype":"future_lifecycle"}"#,
        "bad JSON",
        r#"{"type":"result","subtype":"error_max_turns","is_error":true}"#,
    ] {
        assert!(matches!(
            drain(&make(line, false)).last(),
            Some(TurnEvent::Ended(TurnEnd::Failed(_)))
        ));
    }
}
#[test]
fn launch_uses_only_explicit_mcp_and_no_native_tools_or_api_keys() {
    let launch = Launch::for_turn(&request(), json!({"mcpServers":{}})).unwrap();
    let args = &launch.args;
    for (flag, value) in [
        ("--tools", ""),
        ("--output-format", "stream-json"),
        ("--permission-mode", "dontAsk"),
    ] {
        assert!(args.windows(2).any(|p| p == [flag, value]));
    }
    for flag in [
        "--restricted",
        "--strict-mcp-config",
        "--no-session-persistence",
    ] {
        assert!(args.iter().any(|a| a == flag));
    }
    assert!(
        !args
            .iter()
            .any(|a| a.contains("skip-permissions") || a == "--bare")
    );
    assert!(launch.input["message"]["content"].is_array());
}
#[test]
fn stop_after_tool_started_prevents_the_call_and_discards_queued_events() {
    let engine = make(
        r#"{"type":"result","subtype":"success","is_error":false,"result":"Done"}"#,
        true,
    );
    let bridge = RecordingBridge::probe();
    let req = request();
    let id = req.turn.clone();
    let mut stream = engine.run_turn(req, &bridge).unwrap();
    assert!(matches!(
        block_on(stream.next()),
        Some(TurnEvent::ToolStarted { .. })
    ));
    assert!(
        bridge.calls().is_empty(),
        "tool must not run before its start event is observed"
    );
    engine.interrupt(&id);
    let mut rest = vec![];
    while let Some(e) = block_on(stream.next()) {
        rest.push(e);
    }
    assert!(bridge.calls().is_empty());
    assert_eq!(rest.last(), Some(&TurnEvent::Ended(TurnEnd::Stopped)));
    assert!(
        !rest
            .iter()
            .any(|e| matches!(e, TurnEvent::ToolFinished { .. }))
    );
}
#[test]
fn probe_never_retains_auth_identity_or_treats_unrecognized_output_as_signed_out() {
    let help = "--restricted --tools --mcp-config --strict-mcp-config --no-session-persistence --permission-prompts --include-partial-messages";
    let signed = Installation::from_probe(
        "2.1.301 (Claude Code)",
        help,
        r#"{"loggedIn":true,"email":"person@example.test","accessToken":"TEST DATA secret"}"#,
    );
    assert_eq!(signed.status, EngineStatus::Ready);
    assert!(!format!("{signed:?}").contains("secret"));
    assert_eq!(
        Installation::from_probe("2.1.301", help, "bad JSON").status,
        EngineStatus::Failed
    );
    assert_eq!(
        Installation::from_probe("2.1.301", "old help", r#"{"loggedIn":true}"#).status,
        EngineStatus::Failed
    );
    assert_eq!(
        Installation::from_probe("2.1.301", help, r#"{"loggedIn":false}"#).status,
        EngineStatus::SignedOut
    );
}

#[test]
fn replay_of_real_2_1_301_stream_has_exact_text_and_usage() {
    let events = drain(&make(
        include_str!("../../../../fixtures/engines/claude-code/2.1.301/success.recorded.ndjson"),
        false,
    ));
    assert_eq!(
        events
            .iter()
            .filter_map(|e| if let TurnEvent::Text(t) = e {
                Some(t.as_str())
            } else {
                None
            })
            .collect::<String>(),
        "Fixture complete"
    );
    assert!(events.iter().any(|e|matches!(e,TurnEvent::Usage(u) if u.input_tokens==Some(25846) && u.output_tokens==Some(44) && u.cached_input_tokens==Some(12804) && u.cache_write_tokens==Some(13038))));
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, TurnEvent::ToolStarted { .. })),
        "observed tool records never replay edits; actual calls arrive only through MCP"
    );
}

#[test]
fn unrecognized_native_errors_do_not_echo_diagnostics() {
    let events = drain(&make(
        r#"{"type":"assistant","error":"future_error_kind","message":{"content":[{"type":"text","text":"TEST DATA private-token"}]}}"#,
        false,
    ));
    assert!(!format!("{events:?}").contains("private-token"));
    assert!(matches!(
        events.last(),
        Some(TurnEvent::Ended(TurnEnd::Failed(_)))
    ));
}

#[test]
fn large_tool_results_are_marked_and_bounded_in_messages() {
    let engine = make(
        r#"{"type":"result","subtype":"success","is_error":false,"result":"Done"}"#,
        true,
    );
    let bridge = RecordingBridge::new().with("probe", true, |_| ToolReply {
        value: json!({"large":"x".repeat(10000)}),
        failed: false,
    });
    let mut req = request();
    req.limits.max_tool_result_chars = 32;
    let mut stream = engine.run_turn(req, &bridge).unwrap();
    let mut events = vec![];
    while let Some(e) = block_on(stream.next()) {
        events.push(e);
    }
    assert!(events.iter().any(|e|matches!(e,TurnEvent::ToolFinished {truncated:Some(t),..} if t.total_chars>10000 && t.shown_chars==32)));
    let messages = events
        .iter()
        .find_map(|e| {
            if let TurnEvent::Messages(m) = e {
                Some(m)
            } else {
                None
            }
        })
        .unwrap();
    assert!(format!("{messages:?}").len() < 2000);
}
