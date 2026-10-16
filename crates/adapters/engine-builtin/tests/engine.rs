//! TEST DATA: scripted models and canned tools; no network.
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use vellum_contracts::{
    fakes::{RecordingBridge, ScriptedModel, StalledModel},
    suites::{self, engine::Scenario, model::block_on},
};
use vellum_core::ports::*;
use vellum_engine_builtin::{BuiltinEngine, PromptPack};
use vellum_types::TurnId;

type Script = Vec<Result<ModelEvent, ModelError>>;

fn call(id: &str, name: &str, arguments: Value) -> Script {
    vec![
        Ok(ModelEvent::ToolCall(ModelCall {
            id: id.into(),
            name: name.into(),
            arguments,
            metadata: Value::Null,
        })),
        Ok(ModelEvent::Usage(ModelUsage {
            input_tokens: Some(100),
            output_tokens: Some(10),
            ..Default::default()
        })),
        Ok(ModelEvent::Finished(FinishReason::ToolCalls)),
    ]
}

fn say(text: &str) -> Script {
    vec![
        Ok(ModelEvent::Text(text.into())),
        Ok(ModelEvent::Finished(FinishReason::Stop)),
    ]
}

fn turn(n: u8) -> TurnId {
    TurnId::new(format!("01J00000000000000000000E{n:02}")).unwrap()
}

fn request(prompt: &str) -> TurnRequest {
    TurnRequest::new(turn(1), "fixture-model", prompt)
}

fn run(engine: &BuiltinEngine, req: TurnRequest, bridge: &dyn ToolBridge) -> Vec<TurnEvent> {
    let mut stream = engine.run_turn(req, bridge).expect("run_turn");
    let mut events = vec![];
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    events
}

fn end(events: &[TurnEvent]) -> TurnEnd {
    match events.last() {
        Some(TurnEvent::Ended(end)) => *end,
        other => panic!("no end: {other:?}"),
    }
}

fn engine(model: &Arc<ScriptedModel>) -> BuiltinEngine {
    BuiltinEngine::new(model.clone(), PromptPack::builtin())
}

/// A canvas: `insert_nodes` writes, `get_nodes` reads, `screenshot` returns a picture, `snapshot_layout` checks.
fn canvas() -> RecordingBridge {
    RecordingBridge::new()
        .with("insert_nodes", false, |input| {
            if input["nodes"][0]["width"].is_number() {
                ToolReply {
                    value: json!({"created": ["node1"]}),
                    failed: false,
                }
            } else {
                ToolReply {
                    value: json!({"error": {"kind": "invalid_input", "message": "width is a number, hug or fill.", "path": "nodes[0].width", "op": 0}}),
                    failed: true,
                }
            }
        })
        .with("get_nodes", true, |_| ToolReply {
            value: json!({"nodes": [{"id": "node1", "notes": "x".repeat(100_000)}]}),
            failed: false,
        })
        .with("screenshot", true, |_| ToolReply {
            value: json!({"node": "node1", "scale": 1, "image": {"mime": "image/png", "base64": "iVBORw0KGgo=", "width": 10, "height": 10}}),
            failed: false,
        })
        .with("snapshot_layout", true, |_| ToolReply {
            value: json!({"problems": []}),
            failed: false,
        })
}

fn tool_results(req: &ChatRequest) -> Vec<(String, Value, bool)> {
    req.messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|p| match p {
            ChatPart::ToolResult {
                id, result, failed, ..
            } => Some((id.clone(), result.clone(), *failed)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_builtin_engine_obeys_the_contract() {
    suites::engine::agent_engine(&|scenario| match scenario {
        Scenario::CallProbe => Arc::new(BuiltinEngine::new(
            Arc::new(ScriptedModel::new(vec![
                call("c1", "probe", json!({"value": 1})),
                say("Done."),
            ])),
            PromptPack::builtin(),
        )),
        Scenario::WorkUntilStopped => Arc::new(BuiltinEngine::new(
            Arc::new(StalledModel { deaf: false }),
            PromptPack::builtin(),
        )),
    });
}

#[test]
fn stop_ends_the_turn_even_when_the_provider_ignores_cancellation() {
    let engine = Arc::new(BuiltinEngine::new(
        Arc::new(StalledModel { deaf: true }),
        PromptPack::builtin(),
    ));
    let bridge = canvas();
    let mut stream = engine.run_turn(request("Build a card"), &bridge).unwrap();
    let stopper = {
        let engine = engine.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let at = Instant::now();
            engine.interrupt(&turn(1));
            at
        })
    };
    let mut events = vec![];
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    let took = stopper.join().unwrap().elapsed();
    assert_eq!(end(&events), TurnEnd::Stopped);
    assert!(took < Duration::from_secs(3), "Stop took {took:?}");
    assert!(bridge.calls().is_empty());
}

#[test]
fn the_request_carries_the_prompt_pack_skills_tools_history_and_attachments() {
    let model = Arc::new(ScriptedModel::new(vec![say("Hello.")]));
    let mut req = request("Build a pricing card");
    req.effort = Some(ReasoningEffort::High);
    req.images = vec![("image/png".into(), "AAAA".into())];
    req.history = vec![
        ChatMessage {
            role: ChatRole::User,
            content: vec![ChatPart::Text("Earlier ask".into())],
        },
        ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ChatPart::Text("Earlier answer".into())],
        },
    ];
    req.skills = vec![SkillMeta {
        path: "format.md".into(),
        name: "Document format".into(),
        description: "TEST DATA skill description".into(),
        group: SkillGroup::Builtin,
    }];
    let events = run(&engine(&model), req, &canvas());
    assert_eq!(end(&events), TurnEnd::Done);
    let sent = &model.requests()[0];
    assert_eq!(sent.model, "fixture-model");
    assert_eq!(sent.reasoning, Some(ReasoningEffort::High));
    let pack = PromptPack::builtin();
    assert!(sent.system.starts_with(pack.system_prefix()));
    assert!(sent.system.contains("format.md"));
    assert!(sent.system.contains("TEST DATA skill description"));
    let mut names: Vec<_> = sent.tools.iter().map(|t| t.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["get_nodes", "insert_nodes", "screenshot", "snapshot_layout"]
    );
    assert_eq!(sent.messages.len(), 3, "history, then the ask");
    assert_eq!(
        sent.messages[2].content,
        vec![
            ChatPart::Text("Build a pricing card".into()),
            ChatPart::Image {
                mime: "image/png".into(),
                base64: "AAAA".into()
            }
        ]
    );
    assert!(events.contains(&TurnEvent::Text("Hello.".into())));
    // The turn's own messages, without the history.
    let Some(TurnEvent::Messages(messages)) =
        events.iter().find(|e| matches!(e, TurnEvent::Messages(_)))
    else {
        panic!("no messages");
    };
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, vec![ChatPart::Text("Hello.".into())]);
}

#[test]
fn a_failed_call_goes_back_to_the_model_which_can_retry() {
    let model = Arc::new(ScriptedModel::new(vec![
        call(
            "c1",
            "insert_nodes",
            json!({"nodes": [{"type": "frame", "width": "bad"}]}),
        ),
        call(
            "c2",
            "insert_nodes",
            json!({"nodes": [{"type": "frame", "width": 320}]}),
        ),
        call("c3", "screenshot", json!({"node": "node1"})),
        say("Built the card."),
    ]));
    let bridge = canvas();
    let events = run(&engine(&model), request("Build a card"), &bridge);
    assert_eq!(end(&events), TurnEnd::Done);
    assert_eq!(bridge.calls().len(), 3);
    let results = tool_results(&model.requests()[1]);
    assert_eq!(results.len(), 1);
    assert!(results[0].2, "the failure is marked");
    assert_eq!(results[0].1["error"]["path"], "nodes[0].width");
    assert!(
        events.iter().any(
            |e| matches!(e, TurnEvent::ToolFinished { call, failed: true, .. } if call == "c1")
        )
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TurnEvent::Usage(u) if u.input_tokens == Some(100)))
    );
}

#[test]
fn large_tool_results_are_cut_with_a_marker_never_dropped() {
    let model = Arc::new(ScriptedModel::new(vec![
        call("c1", "get_nodes", json!({})),
        say("Read it."),
    ]));
    let events = run(&engine(&model), request("Read the page"), &canvas());
    let finished = events
        .iter()
        .find_map(|e| match e {
            TurnEvent::ToolFinished { truncated, .. } => Some(*truncated),
            _ => None,
        })
        .unwrap()
        .expect("the result was cut");
    assert!(finished.total_chars > 100_000);
    assert_eq!(
        finished.shown_chars,
        TurnLimits::default().max_tool_result_chars
    );
    let (_, result, failed) = tool_results(&model.requests()[1]).remove(0);
    assert!(!failed);
    let marker = result["truncated"].as_str().expect("an explicit marker");
    assert!(marker.contains(&finished.total_chars.to_string()));
    let partial = result["partial"].as_str().unwrap();
    assert!(partial.starts_with("{\"nodes\":[{\"id\":\"node1\""));
    assert_eq!(partial.chars().count(), finished.shown_chars);
}

#[test]
fn screenshots_reach_a_vision_model_as_pictures_and_others_as_a_marker() {
    for vision in [true, false] {
        let model = Arc::new(
            ScriptedModel::new(vec![
                call("c1", "screenshot", json!({"node": "node1"})),
                say("Looks right."),
            ])
            .with_caps(ModelCaps {
                tools: true,
                vision,
                reasoning: false,
                context_tokens: None,
            }),
        );
        let events = run(&engine(&model), request("Check it"), &canvas());
        assert_eq!(end(&events), TurnEnd::Done, "vision {vision}: {events:?}");
        let sent = &model.requests()[1];
        let last = sent.messages.last().unwrap();
        let (_, result, _) = tool_results(sent).remove(0);
        assert!(
            result["image"]["base64"].as_str().unwrap().starts_with('['),
            "the picture's bytes are not repeated as text"
        );
        let pictures = last
            .content
            .iter()
            .filter(|p| matches!(p, ChatPart::Image { .. }))
            .count();
        assert_eq!(pictures, usize::from(vision));
        assert!(
            matches!(last.content[0], ChatPart::ToolResult { .. }),
            "results come first"
        );
    }
}

#[test]
fn after_writing_the_engine_asks_for_one_self_check() {
    let model = Arc::new(ScriptedModel::new(vec![
        call(
            "c1",
            "insert_nodes",
            json!({"nodes": [{"type": "frame", "width": 320}]}),
        ),
        say("Done, I think."),
        call("c2", "screenshot", json!({"node": "node1"})),
        say("Checked: it matches."),
    ]));
    let events = run(&engine(&model), request("Build a card"), &canvas());
    assert_eq!(end(&events), TurnEnd::Done);
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == TurnEvent::SelfCheck)
            .count(),
        1
    );
    assert_eq!(model.remaining(), 0);
    let asked = &model.requests()[2];
    let ChatPart::Text(text) = &asked.messages.last().unwrap().content[0] else {
        panic!("the self-check is a message");
    };
    assert_eq!(text, PromptPack::builtin().self_check());
}

#[test]
fn no_self_check_when_the_model_already_checked_or_only_read() {
    let model = Arc::new(ScriptedModel::new(vec![
        call(
            "c1",
            "insert_nodes",
            json!({"nodes": [{"type": "frame", "width": 320}]}),
        ),
        call("c2", "snapshot_layout", json!({"problems_only": true})),
        say("Built and checked."),
    ]));
    let events = run(&engine(&model), request("Build a card"), &canvas());
    assert_eq!(end(&events), TurnEnd::Done);
    assert!(!events.contains(&TurnEvent::SelfCheck));
    let model = Arc::new(ScriptedModel::new(vec![say("Nothing to change.")]));
    let events = run(&engine(&model), request("What is here?"), &canvas());
    assert!(!events.contains(&TurnEvent::SelfCheck));
}

#[test]
fn turns_give_up_after_too_many_failed_calls_or_rounds() {
    let bad = || call("c", "insert_nodes", json!({"nodes": [{"width": "bad"}]}));
    let model = Arc::new(ScriptedModel::new(vec![bad(), bad(), bad(), bad()]));
    let mut req = request("Build");
    req.limits.max_failed_calls = 2;
    let events = run(&engine(&model), req, &canvas());
    assert_eq!(
        end(&events),
        TurnEnd::Failed(EngineError::TooManyFailedCalls)
    );
    assert_eq!(model.requests().len(), 3);

    let read = || call("c", "snapshot_layout", json!({}));
    let model = Arc::new(ScriptedModel::new(vec![read(), read(), read()]));
    let mut req = request("Look");
    req.limits.max_rounds = 2;
    let events = run(&engine(&model), req, &canvas());
    assert_eq!(end(&events), TurnEnd::Failed(EngineError::RoundLimit));
    assert_eq!(model.requests().len(), 2);
}

#[test]
fn model_failures_and_refusals_end_the_turn_as_failed() {
    let model = Arc::new(ScriptedModel::new(vec![vec![Err(ModelError::RateLimited)]]));
    let events = run(&engine(&model), request("Build"), &canvas());
    assert_eq!(
        end(&events),
        TurnEnd::Failed(EngineError::Model(ModelError::RateLimited))
    );
    let model = Arc::new(ScriptedModel::new(vec![vec![Ok(ModelEvent::Finished(
        FinishReason::Refused,
    ))]]));
    let events = run(&engine(&model), request("Build"), &canvas());
    assert_eq!(end(&events), TurnEnd::Failed(EngineError::Refused));
    let model = Arc::new(ScriptedModel::new(vec![]));
    let events = run(&engine(&model), request("Build"), &canvas());
    assert_eq!(
        end(&events),
        TurnEnd::Failed(EngineError::Model(ModelError::Exhausted))
    );
}

fn small_context(scripts: Vec<Script>, tokens: u64) -> Arc<ScriptedModel> {
    Arc::new(ScriptedModel::new(scripts).with_caps(ModelCaps {
        tools: true,
        vision: true,
        reasoning: false,
        context_tokens: Some(tokens),
    }))
}

#[test]
fn old_tool_results_are_pruned_with_markers_when_context_runs_short() {
    let read = |id: &str| call(id, "get_nodes", json!({}));
    // Each cut result is about 24k characters (~6k tokens); a 40k-token window fits about four.
    let model = small_context(
        vec![
            read("c1"),
            read("c2"),
            read("c3"),
            read("c4"),
            read("c5"),
            say("Read everything."),
        ],
        40_000,
    );
    let mut req = request("Read all of it");
    req.limits.max_output_tokens = 2_000;
    let events = run(&engine(&model), req, &canvas());
    assert_eq!(end(&events), TurnEnd::Done, "{events:?}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TurnEvent::Compacted { pruned, summarized: 0 } if *pruned > 0))
    );
    let last = model.requests().pop().unwrap();
    let results = tool_results(&last);
    assert_eq!(results.len(), 5, "every call keeps its result");
    let first = &results[0].1;
    assert!(
        first["removed"].as_str().unwrap().contains("get_nodes"),
        "the oldest result is replaced by a marker that names the tool: {first}"
    );
    let newest = &results[4].1;
    assert!(newest["partial"].is_string(), "the newest result is kept");
}

#[test]
fn when_pruning_is_not_enough_earlier_rounds_are_summarized() {
    let write = |id: &str, width: u32| {
        call(
            id,
            "insert_nodes",
            json!({"nodes": [{"type": "frame", "width": width, "name": "y".repeat(40_000)}]}),
        )
    };
    let model = small_context(
        vec![
            write("c1", 1),
            write("c2", 2),
            write("c3", 3),
            // Three 10k-token calls overflow a 30k window; results are tiny, so pruning cannot help. The summary:
            say("Inserted frames 1 and 2."),
            call("c4", "snapshot_layout", json!({})),
            say("Done."),
        ],
        30_000,
    );
    let mut req = request("Build three frames");
    req.limits.max_output_tokens = 1_000;
    let events = run(&engine(&model), req, &canvas());
    assert_eq!(end(&events), TurnEnd::Done, "{events:?}");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TurnEvent::Compacted { summarized, .. } if *summarized > 0))
    );
    let requests = model.requests();
    let summary_request = &requests[3];
    assert!(
        summary_request.tools.is_empty(),
        "a summary request offers no tools"
    );
    assert_eq!(summary_request.system, PromptPack::builtin().summarize());
    let after = &requests[4];
    let ChatPart::Text(first) = &after.messages[0].content[0] else {
        panic!("the summary leads");
    };
    assert!(
        first.contains("Build three frames"),
        "the ask survives: {first}"
    );
    assert!(first.contains("Inserted frames 1 and 2."));
    assert!(
        matches!(after.messages[1].role, ChatRole::Assistant),
        "roles still alternate"
    );
    // Every remaining tool result still has its call.
    let calls: Vec<_> = after
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|p| match p {
            ChatPart::ToolCall(c) => Some(c.id.clone()),
            _ => None,
        })
        .collect();
    for (id, ..) in tool_results(after) {
        assert!(calls.contains(&id), "orphan result {id}");
    }
}

#[test]
fn prompt_packs_are_versioned_files() {
    let pack = PromptPack::builtin();
    assert_eq!(pack.id(), "builtin");
    assert!(pack.version() >= 1);
    let text = "---\nid: tuned\nversion: 7\n---\n# system\nYou design.\n\n{{skills}}\n# self-check\nLook again.\n# summarize\nSum up.\n";
    let pack = PromptPack::parse(text).unwrap();
    assert_eq!((pack.id(), pack.version()), ("tuned", 7));
    assert_eq!(pack.self_check(), "Look again.");
    assert!(pack.system(&[]).starts_with("You design."));
    assert!(PromptPack::parse("# system\nno header\n").is_err());
    assert!(PromptPack::parse("---\nid: x\nversion: 1\n---\n# system\nNo skills slot.\n# self-check\na\n# summarize\nb\n").is_err());
}

#[test]
fn availability_follows_the_provider() {
    let ok = BuiltinEngine::new(Arc::new(ScriptedModel::new(vec![])), PromptPack::builtin());
    assert_eq!(ok.id(), "builtin");
    assert_eq!(ok.availability(), EngineStatus::Ready);
    let no_tools = BuiltinEngine::new(
        Arc::new(ScriptedModel::new(vec![]).with_caps(ModelCaps::default())),
        PromptPack::builtin(),
    );
    assert_eq!(no_tools.availability(), EngineStatus::Failed);
    assert!(matches!(
        no_tools.run_turn(request("x"), &canvas()),
        Err(EngineError::Unavailable)
    ));
}
