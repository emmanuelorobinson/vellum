//! TEST DATA. Recorded protocol replays run without an installed engine, account or network.
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
use vellum_engine_codex::*;
struct Replay {
    lines: Vec<String>,
    mode: Mode,
    sent: Arc<Mutex<Vec<Value>>>,
}
impl Launcher for Replay {
    fn inspect(&self) -> Installation {
        Installation {
            version: Some("0.158.0".into()),
            status: EngineStatus::Ready,
            mode: Some(self.mode),
        }
    }
    fn start(&self, _: Launch, cancel: Cancellation) -> Result<Box<dyn Process>, EngineError> {
        Ok(Box::new(Tape {
            lines: self.lines.clone().into(),
            cancel,
            sent: self.sent.clone(),
            stall: self.lines.is_empty(),
        }))
    }
}
struct Tape {
    lines: VecDeque<String>,
    cancel: Cancellation,
    sent: Arc<Mutex<Vec<Value>>>,
    stall: bool,
}
impl Process for Tape {
    fn next(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>> {
        Box::pin(std::future::poll_fn(|cx| {
            if self.cancel.poll_cancelled(cx).is_ready() {
                Poll::Ready(Ok(None))
            } else if let Some(l) = self.lines.pop_front() {
                Poll::Ready(Ok(Some(l)))
            } else if self.stall {
                Poll::Pending
            } else {
                Poll::Ready(Ok(None))
            }
        }))
    }
    fn send(&mut self, v: Value) -> EngineFuture<'_, Result<(), EngineError>> {
        self.sent.lock().unwrap().push(v);
        Box::pin(async { Ok(()) })
    }
}
struct Host {
    probe: bool,
}
impl McpHost for Host {
    fn open(&self, _: &[BridgeTool]) -> Result<Box<dyn McpChannel>, EngineError> {
        Ok(Box::new(Host { probe: self.probe }))
    }
}
impl McpChannel for Host {
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
fn make(text: &str, mode: Mode, probe: bool) -> (CodexEngine, Arc<Mutex<Vec<Value>>>) {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if probe && mode == Mode::AppServer {
        lines.insert(4.min(lines.len()),json!({"id":"fixture-call","method":"item/tool/call","params":{"threadId":"TEST-thread","turnId":"TEST-turn","callId":"call-probe","tool":"probe","arguments":{"value":1}}}).to_string());
    }
    let sent = Arc::new(Mutex::new(vec![]));
    (
        CodexEngine::new(
            Arc::new(Replay {
                lines,
                mode,
                sent: sent.clone(),
            }),
            Arc::new(Host {
                probe: probe && mode == Mode::Exec,
            }),
        ),
        sent,
    )
}
fn request() -> TurnRequest {
    TurnRequest::new(
        vellum_types::TurnId::new("01J00000000000000000000T01").unwrap(),
        "fixture-model",
        "TEST DATA probe",
    )
}
fn drain(e: &CodexEngine) -> Vec<TurnEvent> {
    let b = RecordingBridge::probe();
    let mut s = e.run_turn(request(), &b).unwrap();
    let mut out = vec![];
    while let Some(e) = block_on(s.next()) {
        out.push(e)
    }
    out
}
const APP: &str =
    include_str!("../../../../fixtures/engines/codex/0.158.0/app-server.synthetic.ndjson");
const EXEC: &str = include_str!("../../../../fixtures/engines/codex/0.158.0/exec.synthetic.ndjson");
#[test]
fn both_transports_pass_the_shared_contract() {
    for mode in [Mode::AppServer, Mode::Exec] {
        engine::agent_engine(&|s| {
            Arc::new(
                make(
                    if s == engine::Scenario::CallProbe {
                        if mode == Mode::AppServer { APP } else { EXEC }
                    } else {
                        ""
                    },
                    mode,
                    s == engine::Scenario::CallProbe,
                )
                .0,
            )
        });
    }
}
#[test]
fn app_server_handshake_is_sequential_and_text_usage_are_not_duplicated() {
    let (e, sent) = make(APP, Mode::AppServer, false);
    let ev = drain(&e);
    assert_eq!(ev.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    assert_eq!(
        ev.iter()
            .filter_map(|e| if let TurnEvent::Text(t) = e {
                Some(t.as_str())
            } else {
                None
            })
            .collect::<String>(),
        "Done"
    );
    let usage: Vec<_> = ev
        .iter()
        .filter_map(|e| {
            if let TurnEvent::Usage(u) = e {
                Some(u)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].input_tokens, Some(30));
    assert_eq!(usage[0].cached_input_tokens, Some(20));
    assert_eq!(usage[0].output_tokens, Some(5));
    let sent = sent.lock().unwrap();
    assert_eq!(
        sent.iter()
            .filter_map(|v| v["method"].as_str())
            .collect::<Vec<_>>(),
        vec![
            "initialize",
            "initialized",
            "config/read",
            "thread/start",
            "turn/start"
        ]
    );
    assert_eq!(sent[3]["params"]["ephemeral"], true);
    assert_eq!(sent[3]["params"]["environments"], json!([]));
    assert_eq!(
        sent[3]["params"]["config"]["mcp_servers"]["other"]["enabled"],
        false
    );
    assert_eq!(sent[4]["params"]["threadId"], "TEST-thread");
}
#[test]
fn exec_replay_and_terminal_errors_are_honest() {
    assert_eq!(
        drain(&make(EXEC, Mode::Exec, false).0).last(),
        Some(&TurnEvent::Ended(TurnEnd::Done))
    );
    for mode in [Mode::AppServer, Mode::Exec] {
        for text in [
            "{bad",
            r#"{"type":"future.event"}"#,
            r#"{"method":"turn/completed","params":{"threadId":"other","turn":{"id":"other","status":"completed"}}}"#,
        ] {
            assert!(matches!(
                drain(&make(text, mode, false).0).last(),
                Some(TurnEvent::Ended(TurnEnd::Failed(_)))
            ));
        }
    }
}
#[test]
fn structured_auth_and_limits_do_not_echo_diagnostics() {
    for (kind, status) in [
        ("unauthorized", EngineStatus::SignedOut),
        ("usageLimitExceeded", EngineStatus::UsageLimit),
    ] {
        let text = format!(
            "{}\n{}",
            APP.lines().take(4).collect::<Vec<_>>().join("\n"),
            json!({"method":"turn/completed","params":{"threadId":"TEST-thread","turn":{"id":"TEST-turn","status":"failed","error":{"codexErrorInfo":kind,"message":"TEST DATA secret diagnostic"}}}})
        );
        let (e, _) = make(&text, Mode::AppServer, false);
        let events = drain(&e);
        assert_eq!(e.availability(), status);
        assert!(!format!("{events:?}").contains("secret diagnostic"));
        assert_eq!(e.recheck().status, EngineStatus::Ready);
    }
}
#[test]
fn stop_at_start_boundary_prevents_the_actual_bridge_call() {
    let (e, _) = make(APP, Mode::AppServer, true);
    let b = RecordingBridge::probe();
    let r = request();
    let id = r.turn.clone();
    let mut s = e.run_turn(r, &b).unwrap();
    assert!(matches!(
        block_on(s.next()),
        Some(TurnEvent::ToolStarted { .. })
    ));
    e.interrupt(&id);
    while block_on(s.next()).is_some() {}
    assert!(b.calls().is_empty());
}
#[test]
fn probes_choose_app_server_first_and_reject_unrecognized_auth() {
    let root = "app-server exec";
    let exec = "--json --ephemeral --ignore-user-config --ignore-rules --sandbox";
    for (auth, ok, status) in [
        ("Logged in using ChatGPT", true, EngineStatus::Ready),
        (
            "Logged in using an API key - TEST DATA secret",
            true,
            EngineStatus::Ready,
        ),
        ("Not logged in", false, EngineStatus::SignedOut),
        ("unexpected output", false, EngineStatus::Failed),
    ] {
        let i =
            Installation::from_probe("codex-cli 0.158.0", root, Some("--listen"), exec, ok, auth);
        assert_eq!(i.status, status);
        assert_eq!(i.mode, Some(Mode::AppServer));
        assert!(!format!("{i:?}").contains("secret"));
    }
    assert_eq!(
        Installation::from_probe(
            "codex-cli 0.158.0",
            "exec",
            None,
            exec,
            true,
            "Logged in using ChatGPT"
        )
        .mode,
        Some(Mode::Exec)
    );
    assert_eq!(
        Installation::from_probe(
            "codex-cli 0.158.0",
            root,
            None,
            exec,
            true,
            "Logged in using ChatGPT"
        )
        .status,
        EngineStatus::Failed
    );
}

#[test]
fn quota_windows_preserve_reported_reset_without_inventing_cost() {
    let mut lines: Vec<_> = APP.lines().map(str::to_owned).collect();
    lines.insert(4,json!({"method":"account/rateLimits/updated","params":{"rateLimits":{"primary":{"usedPercent":100,"windowDurationMins":300,"resetsAt":1792188000},"secondary":null}}}).to_string());
    let last = lines.len() - 1;
    lines[last]=json!({"method":"turn/completed","params":{"threadId":"TEST-thread","turn":{"id":"TEST-turn","status":"failed","error":{"codexErrorInfo":"usageLimitExceeded"}}}}).to_string();
    let (e, _) = make(&lines.join("\n"), Mode::AppServer, false);
    drain(&e);
    assert_eq!(e.usage_limits().unwrap().primary.unwrap().used_percent, 100);
    assert_eq!(e.usage_resets_at(), Some(1792188000000));
}
#[test]
fn launch_keeps_prompt_off_argv_and_requires_native_tool_isolation() {
    let mut t = request();
    t.prompt = "TEST DATA private prompt".into();
    let mcp = json!({"mcpServers":{"vellum":{"command":"vellum","args":["--run-dir","/tmp/TEST DATA","mcp"]}}});
    for mode in [Mode::AppServer, Mode::Exec] {
        let l = Launch::for_turn(&t, mcp.clone(), mode).unwrap();
        assert!(!l.args.join(" ").contains("private prompt"));
        assert!(!l.args.join(" ").contains("dangerously"));
        for flag in [
            "features.shell_tool=false",
            "features.stable_environment_tools=false",
            "features.hooks=false",
            "features.apps=false",
            "features.plugins=false",
            "analytics.enabled=false",
            "web_search=\"disabled\"",
        ] {
            assert!(l.args.iter().any(|a| a == flag), "{flag}");
        }
        if mode == Mode::Exec {
            assert!(l.input.unwrap().contains("private prompt"));
        }
    }
}
#[test]
fn bounds_tools_and_never_executes_observed_native_mcp_records() {
    let (e, _) = make(APP, Mode::AppServer, true);
    let b = RecordingBridge::new().with("probe", true, |_| ToolReply {
        value: json!({"large":"x".repeat(10000)}),
        failed: false,
    });
    let mut t = request();
    t.limits.max_tool_result_chars = 32;
    let mut s = e.run_turn(t, &b).unwrap();
    let mut events = vec![];
    while let Some(v) = block_on(s.next()) {
        events.push(v);
    }
    assert_eq!(b.calls().len(), 1);
    assert!(
        events.iter().any(
            |v| matches!(v,TurnEvent::ToolFinished{truncated:Some(t),..} if t.shown_chars==32)
        )
    );
    assert!(
        events
            .iter()
            .filter(|v| matches!(v, TurnEvent::Messages(_)))
            .all(|v| format!("{v:?}").len() < 2000)
    );
}
#[test]
fn out_of_order_handshake_and_foreign_turn_events_are_rejected() {
    for lines in [
        vec![json!({"id":3,"result":{"thread":{"id":"foreign"}}})],
        vec![
            json!({"id":1,"result":{}}),
            json!({"id":2,"result":{"config":{}}}),
            json!({"id":3,"result":{"thread":{"id":"TEST-thread"}}}),
            json!({"id":4,"result":{"turn":{"id":"TEST-turn"}}}),
            json!({"method":"item/agentMessage/delta","params":{"threadId":"foreign","turnId":"TEST-turn","itemId":"m","delta":"TEST DATA private"}}),
        ],
    ] {
        let text = lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let events = drain(&make(&text, Mode::AppServer, false).0);
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Ended(TurnEnd::Failed(_)))
        ));
        assert!(!format!("{events:?}").contains("private"));
    }
}

#[test]
fn app_server_direct_tool_request_runs_once_through_the_borrowed_bridge() {
    let mut lines: Vec<_> = APP.lines().map(str::to_owned).collect();
    lines.insert(4,json!({"id":"TEST-rpc-call","method":"item/tool/call","params":{"threadId":"TEST-thread","turnId":"TEST-turn","callId":"call-1","tool":"probe","arguments":{"value":1}}}).to_string());
    let (e, sent) = make(&lines.join("\n"), Mode::AppServer, false);
    let b = RecordingBridge::probe();
    let mut s = e.run_turn(request(), &b).unwrap();
    let mut events = vec![];
    while let Some(v) = block_on(s.next()) {
        events.push(v);
    }
    assert_eq!(b.calls(), vec![("probe".into(), json!({"value":1}))]);
    assert_eq!(
        events
            .iter()
            .filter(|v| matches!(v, TurnEvent::ToolStarted { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|v| matches!(v, TurnEvent::ToolFinished { .. }))
            .count(),
        1
    );
    let sent = sent.lock().unwrap();
    assert!(
        sent.iter()
            .any(|v| v["id"] == "TEST-rpc-call" && v["result"]["success"] == true)
    );
    assert_eq!(sent[3]["params"]["dynamicTools"][0]["name"], "probe");
}

#[test]
fn recorder_projects_exec_but_discards_native_identity_and_diagnostics() {
    let v = json!({"type":"turn.completed","usage":{"input_tokens":30,"output_tokens":5,"token":"TEST DATA secret"},"cwd":"TEST DATA secret"});
    let projected = recording::record(&v).unwrap();
    assert_eq!(projected["usage"]["input_tokens"], 30);
    assert!(!projected.to_string().contains("secret"));
    for v in [
        json!({"id":2,"result":{"config":{"key":"TEST DATA secret"}}}),
        json!({"type":"turn.failed","error":{"message":"TEST DATA secret"}}),
        json!({"method":"account/updated","params":{"email":"TEST DATA secret"}}),
    ] {
        assert!(
            !recording::record(&v)
                .map(|v| v.to_string())
                .unwrap_or_default()
                .contains("secret")
        );
    }
}

#[test]
fn recorded_app_server_turn_replays_the_canvas_call_text_and_exact_usage() {
    let (e, _) = make(
        include_str!("../../../../fixtures/engines/codex/0.158.0/success.recorded.ndjson"),
        Mode::AppServer,
        false,
    );
    let b = RecordingBridge::new().with("get_app_state", true, |_| ToolReply {
        value: json!({"testData":true}),
        failed: false,
    });
    let mut s = e.run_turn(request(), &b).unwrap();
    let mut events = vec![];
    while let Some(v) = block_on(s.next()) {
        events.push(v);
    }
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    assert_eq!(b.calls().len(), 1);
    assert_eq!(
        events
            .iter()
            .filter_map(|v| if let TurnEvent::Text(t) = v {
                Some(t.as_str())
            } else {
                None
            })
            .collect::<String>(),
        "Fixture complete"
    );
    assert!(events.iter().any(|v|matches!(v,TurnEvent::Usage(u) if u.input_tokens==Some(38398)&&u.output_tokens==Some(158)&&u.cached_input_tokens==Some(18944))));
}
#[test]
fn app_server_delivers_pictures_separately_from_bounded_tool_text() {
    let (e, sent) = make(APP, Mode::AppServer, true);
    let b=RecordingBridge::new().with("probe",true, |_|ToolReply{value:json!({"image":{"mime":"image/png","base64":"TEST DATA image"},"text":"x".repeat(1000)}),failed:false});
    let mut t = request();
    t.limits.max_tool_result_chars = 32;
    let mut s = e.run_turn(t, &b).unwrap();
    let mut history = None;
    while let Some(v) = block_on(s.next()) {
        if let TurnEvent::Messages(m) = v {
            history = Some(m);
        }
    }
    let sent = sent.lock().unwrap();
    let reply = sent.iter().find(|v| v["id"] == "fixture-call").unwrap();
    assert_eq!(reply["result"]["contentItems"][1]["type"], "inputImage");
    assert_eq!(
        reply["result"]["contentItems"][1]["imageUrl"],
        "data:image/png;base64,TEST DATA image"
    );
    assert!(!format!("{history:?}").contains("TEST DATA image"));
}
#[test]
fn native_permissions_unknown_tools_and_duplicate_calls_fail_closed() {
    for tail in [
        json!({"id":"x","method":"item/commandExecution/requestApproval","params":{}}),
        json!({"id":"x","method":"item/tool/call","params":{"threadId":"TEST-thread","turnId":"TEST-turn","callId":"x","tool":"exec_command","arguments":{}}}),
    ] {
        let text = APP
            .lines()
            .take(4)
            .chain(
                std::iter::once(tail.to_string())
                    .collect::<Vec<_>>()
                    .iter()
                    .map(String::as_str),
            )
            .collect::<Vec<_>>()
            .join("\n");
        let events = drain(&make(&text, Mode::AppServer, false).0);
        assert!(matches!(
            events.last(),
            Some(TurnEvent::Ended(TurnEnd::Failed(_)))
        ));
    }
}

#[test]
fn exec_projection_keeps_distinct_assistant_items_and_error_statuses() {
    let a=recording::record(&json!({"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"First"}})).unwrap();
    let b=recording::record(&json!({"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Second"}})).unwrap();
    assert_ne!(a["item"]["id"], b["item"]["id"]);
    for (message, status) in [
        (
            "You’ve hit your usage limit. TEST DATA diagnostic",
            EngineStatus::UsageLimit,
        ),
        (
            "Your access token could not be refreshed because your refresh token was already used. Please log out and sign in again.",
            EngineStatus::SignedOut,
        ),
    ] {
        let lines = json!({"type":"turn.failed","error":{"message":message}}).to_string();
        let (e, _) = make(&lines, Mode::Exec, false);
        let events = drain(&e);
        assert_eq!(e.availability(), status);
        assert!(!format!("{events:?}").contains("diagnostic"));
    }
}

#[test]
fn repeated_native_calls_never_repeat_a_canvas_edit() {
    let call=json!({"id":"rpc-a","method":"item/tool/call","params":{"threadId":"TEST-thread","turnId":"TEST-turn","callId":"once","tool":"probe","arguments":{"value":1}}}).to_string();
    let text = APP
        .lines()
        .take(4)
        .chain([call.as_str(), call.as_str()])
        .collect::<Vec<_>>()
        .join("\n");
    let (e, _) = make(&text, Mode::AppServer, false);
    let b = RecordingBridge::probe();
    let mut s = e.run_turn(request(), &b).unwrap();
    let mut end = None;
    while let Some(v) = block_on(s.next()) {
        if let TurnEvent::Ended(e) = v {
            end = Some(e);
        }
    }
    assert_eq!(b.calls().len(), 1);
    assert!(matches!(end, Some(TurnEnd::Failed(_))));
}
#[test]
fn output_is_bounded_before_native_usage_is_reported() {
    let lines = format!(
        "{}\n{}",
        APP.lines().take(4).collect::<Vec<_>>().join("\n"),
        json!({"method":"item/agentMessage/delta","params":{"threadId":"TEST-thread","turnId":"TEST-turn","itemId":"msg","delta":"x".repeat(4000)}})
    );
    let (e, _) = make(&lines, Mode::AppServer, false);
    let b = RecordingBridge::probe();
    let mut req = request();
    req.limits.max_output_tokens = 1;
    let mut s = e.run_turn(req, &b).unwrap();
    let mut end = None;
    while let Some(v) = block_on(s.next()) {
        if let TurnEvent::Ended(e) = v {
            end = Some(e);
        }
    }
    assert_eq!(
        end,
        Some(TurnEnd::Failed(EngineError::Model(ModelError::Limit)))
    );
}

#[test]
fn recorded_exec_turn_replays_both_messages_and_inclusive_usage() {
    let events = drain(
        &make(
            include_str!("../../../../fixtures/engines/codex/0.158.0/exec-success.recorded.ndjson"),
            Mode::Exec,
            false,
        )
        .0,
    );
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    let text: Vec<_> = events
        .iter()
        .filter_map(|v| {
            if let TurnEvent::Text(t) = v {
                Some(t.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(text.len(), 2);
    assert_eq!(text[1], "Fixture complete");
    assert!(events.iter().any(|v|matches!(v,TurnEvent::Usage(u) if u.input_tokens==Some(53849)&&u.cached_input_tokens==Some(33280)&&u.output_tokens==Some(91))));
    assert!(
        !events
            .iter()
            .any(|v| matches!(v, TurnEvent::ToolStarted { .. })),
        "exec's observed records never repeat MCP effects"
    );
}

#[test]
fn inherited_mcp_names_are_literal_and_their_credentials_are_not_forwarded() {
    let mut lines: Vec<_> = APP.lines().map(str::to_owned).collect();
    lines[1]=json!({"id":2,"result":{"config":{"mcp_servers":{"provider.with.dots":{"command":"TEST DATA","env":{"token":"TEST DATA secret"}}}}}}).to_string();
    let (e, sent) = make(&lines.join("\n"), Mode::AppServer, false);
    let events = drain(&e);
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    let sent = sent.lock().unwrap();
    assert_eq!(
        sent[3]["params"]["config"]["mcp_servers"]["provider.with.dots"]["enabled"],
        false
    );
    assert!(!sent[3].to_string().contains("secret"));
}

#[test]
fn completion_before_turn_start_acknowledgement_is_not_success() {
    let text = APP
        .lines()
        .take(3)
        .chain(APP.lines().last())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(matches!(
        drain(&make(&text, Mode::AppServer, false).0).last(),
        Some(TurnEvent::Ended(TurnEnd::Failed(_)))
    ));
}
