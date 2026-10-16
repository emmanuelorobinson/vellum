//! TEST DATA. Tagged `model-http`: loopback sockets only, no external model or credential.
use serde_json::{Value, json};
use std::sync::Arc;
use vellum_contracts::{fakes::MemorySecrets, suites::model::block_on};
use vellum_core::ports::*;
use vellum_testkit::model::{StubEndpoint, StubReply};

fn config(endpoint: &str) -> ModelConfig {
    ModelConfig {
        id: "test-provider".into(),
        endpoint: endpoint.into(),
        secret_key: Some("test".into()),
        caps: ModelCaps {
            tools: true,
            vision: true,
            reasoning: true,
            context_tokens: Some(100_000),
        },
    }
}
fn secrets() -> Arc<dyn SecretStore> {
    let store = MemorySecrets::default();
    store
        .set("test", SecretString::new("stub-private-credential"))
        .unwrap();
    Arc::new(store)
}
fn collect(
    provider: &dyn ModelProvider,
    request: ChatRequest,
) -> Result<Vec<ModelEvent>, ModelError> {
    block_on(async {
        let mut stream = provider.complete(request, Cancellation::default()).await?;
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event?);
        }
        Ok(events)
    })
}
fn request() -> ChatRequest {
    let mut request = ChatRequest::text("fixture-model", "Build a card");
    request.reasoning = Some(ReasoningEffort::Low);
    request.tools.push(ModelTool {
        name: "insert_nodes".into(),
        description: "Insert layers".into(),
        parameters: json!({"type":"object","properties":{}}),
    });
    request.messages[0].content.push(ChatPart::Image {
        mime: "image/png".into(),
        base64: "aGVsbG8=".into(),
    });
    request
}
fn chat_reply() -> StubReply {
    StubReply::sse(
        vec![
            json!({"choices":[{"delta":{"content":"Ready "}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"insert_nodes","arguments":"{\"nodes\":"}}]}}]}),
            json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"[]}"}}]},"finish_reason":"tool_calls"}]}),
            json!({"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":7}}),
        ],
        true,
    )
}

#[test]
#[ignore = "model-http: requires loopback sockets; run scripts/model-providers.mjs"]
fn chat_completions_streams_tools_vision_effort_and_usage() {
    let stub = StubEndpoint::start(vec![chat_reply()]).unwrap();
    let provider = vellum_llm_openai_compat::Provider::new(
        config(&stub.url("/v1/chat/completions")),
        secrets(),
        Arc::new(vellum_model_http::HttpTransport::default()),
    )
    .unwrap();
    let events = collect(&provider, request()).unwrap();
    assert!(events.contains(&ModelEvent::Text("Ready ".into())));
    assert!(events.contains(&ModelEvent::ToolCall(ModelCall {
        id: "call_1".into(),
        name: "insert_nodes".into(),
        arguments: json!({"nodes":[]}),
        metadata: Value::Null
    })));
    assert!(events.contains(&ModelEvent::Usage(ModelUsage {
        input_tokens: Some(12),
        output_tokens: Some(7),
        reasoning_tokens: None,
        cached_input_tokens: None
    })));
    assert_eq!(provider.status(), ProviderStatus::Connected);
    let seen = stub.requests();
    assert_eq!(seen[0].body["reasoning_effort"], "low");
    assert_eq!(
        seen[0].body["messages"][0]["content"][1]["type"],
        "image_url"
    );
    assert_eq!(seen[0].body["tools"][0]["function"]["name"], "insert_nodes");
    assert!(!format!("{seen:?}").contains("stub-private-credential"));
}

#[test]
fn provider_catalog_has_all_requested_presets() {
    use vellum_llm_openai_compat::{OPENROUTER_DEFAULT_MODEL, presets};
    assert_eq!(OPENROUTER_DEFAULT_MODEL, "deepseek/deepseek-v4.1-flash");
    for name in [
        "openrouter",
        "deepseek",
        "xai",
        "moonshot",
        "together",
        "fireworks",
        "zai",
        "opencode",
        "ollama",
        "lmstudio",
    ] {
        assert!(presets().iter().any(|preset| preset.id == name));
    }
}

#[test]
#[ignore = "model-http: requires loopback sockets; run scripts/model-providers.mjs"]
fn responses_streams_function_arguments_and_checks_request() {
    let stub = StubEndpoint::start(vec![StubReply::sse(vec![
        json!({"type":"response.output_text.delta","delta":"Ready"}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":"fc1","call_id":"call_1","name":"insert_nodes","arguments":""}}),
        json!({"type":"response.function_call_arguments.delta","output_index":0,"delta":"{\"nodes\":[]}"}),
        json!({"type":"response.completed","response":{"usage":{"input_tokens":12,"output_tokens":7}}}),
    ], false)]).unwrap();
    let provider = vellum_llm_openai_responses::Provider::new(
        config(&stub.url("/v1/responses")),
        secrets(),
        Arc::new(vellum_model_http::HttpTransport::default()),
    )
    .unwrap();
    let events = collect(&provider, request()).unwrap();
    assert!(
        events.iter().any(
            |e| matches!(e, ModelEvent::ToolCall(call) if call.arguments == json!({"nodes":[]}))
        )
    );
    let seen = stub.requests();
    assert_eq!(seen[0].body["reasoning"]["effort"], "low");
    assert_eq!(seen[0].body["store"], false);
    assert_eq!(
        seen[0].body["input"][0]["content"][1]["type"],
        "input_image"
    );
}

#[test]
#[ignore = "model-http: requires loopback sockets; run scripts/model-providers.mjs"]
fn messages_streams_native_tools_and_vertex_routes() {
    use vellum_llm_anthropic::{Deployment, Provider};
    for deployment in [
        Deployment::Api,
        Deployment::Bedrock,
        Deployment::Foundry,
        Deployment::Custom,
        Deployment::Vertex,
    ] {
        let stub = StubEndpoint::start(vec![StubReply::sse(vec![
            json!({"type":"message_start","message":{"usage":{"input_tokens":12}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"call_1","name":"insert_nodes","input":{}}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"nodes\":[]}"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":7}}),
            json!({"type":"message_stop"}),
        ], false)]).unwrap();
        let provider = Provider::new(
            config(&stub.url("/messages")),
            deployment,
            secrets(),
            Arc::new(vellum_model_http::HttpTransport::default()),
        )
        .unwrap();
        let events = collect(&provider, request()).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ModelEvent::ToolCall(call) if call.name == "insert_nodes"))
        );
        let seen = stub.requests();
        assert_eq!(seen[0].body["tools"][0]["input_schema"]["type"], "object");
        assert_eq!(
            seen[0].body["messages"][0]["content"][1]["source"]["type"],
            "base64"
        );
        if deployment == Deployment::Vertex {
            assert!(seen[0].body.get("model").is_none());
            assert_eq!(seen[0].body["anthropic_version"], "vertex-2023-10-16");
        }
    }
}

#[test]
#[ignore = "model-http: requires loopback sockets; run scripts/model-providers.mjs"]
fn gemini_preserves_thought_signatures_and_native_tool_results() {
    let stub = StubEndpoint::start(vec![StubReply::sse(vec![
        json!({"candidates":[{"content":{"parts":[{"functionCall":{"id":"call_1","name":"insert_nodes","args":{"nodes":[]}},"thoughtSignature":"opaque-signature"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":12,"candidatesTokenCount":7}}),
    ], false)]).unwrap();
    let provider = vellum_llm_google::Provider::new(
        config(&stub.url("/streamGenerateContent?alt=sse")),
        secrets(),
        Arc::new(vellum_model_http::HttpTransport::default()),
    )
    .unwrap();
    let events = collect(&provider, request()).unwrap();
    assert!(events.iter().any(|e| matches!(e, ModelEvent::ToolCall(call) if call.metadata["thoughtSignature"] == "opaque-signature")));
    let seen = stub.requests();
    assert_eq!(
        seen[0].body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        "low"
    );
    assert_eq!(
        seen[0].body["contents"][0]["parts"][1]["inlineData"]["mimeType"],
        "image/png"
    );
}

#[test]
#[ignore = "model-http: requires loopback sockets; run scripts/model-providers.mjs"]
fn truncated_stream_and_http_failures_are_typed_and_redacted() {
    for (reply, expected) in [
        (
            StubReply::http(401, "stub-private-credential"),
            ModelError::Authentication,
        ),
        (
            StubReply::http(429, "stub-private-credential"),
            ModelError::RateLimited,
        ),
        (
            StubReply::sse(
                vec![json!({"choices":[{"delta":{"content":"partial"}}]})],
                false,
            ),
            ModelError::Truncated,
        ),
    ] {
        let stub = StubEndpoint::start(vec![reply]).unwrap();
        let provider = vellum_llm_openai_compat::Provider::new(
            config(&stub.url("/chat")),
            secrets(),
            Arc::new(vellum_model_http::HttpTransport::default()),
        )
        .unwrap();
        let error = collect(&provider, ChatRequest::text("model", "hi")).unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?}: {error}").contains("stub-private-credential"));
        assert_eq!(provider.status(), ProviderStatus::Failed);
    }
}

#[test]
#[ignore = "model-http: requires loopback sockets; run scripts/model-providers.mjs"]
fn cancellation_and_dropping_a_stream_close_the_request() {
    for stop in [true, false] {
        let mut reply = StubReply::sse(vec![], false);
        reply.chunks = vec![
            b"data: {\"choices\":[{\"delta\":{\"content\":\"started\"}}]}\n\n".to_vec(),
            b"data: [DONE]\n\n".to_vec(),
        ];
        reply.delay = std::time::Duration::from_secs(5);
        let stub = StubEndpoint::start(vec![reply]).unwrap();
        let provider = vellum_llm_openai_compat::Provider::new(
            config(&stub.url("/chat")),
            secrets(),
            Arc::new(vellum_model_http::HttpTransport::default()),
        )
        .unwrap();
        let cancel = Cancellation::default();
        let mut stream =
            block_on(provider.complete(ChatRequest::text("model", "hi"), cancel.clone())).unwrap();
        assert_eq!(
            block_on(stream.next()).unwrap().unwrap(),
            ModelEvent::Text("started".into())
        );
        if stop {
            cancel.cancel();
            assert_eq!(
                block_on(stream.next()).unwrap().unwrap_err(),
                ModelError::Cancelled
            );
        }
        drop(stream);
        let started = std::time::Instant::now();
        while stub.closed() == 0 && started.elapsed() < std::time::Duration::from_secs(1) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            stub.closed(),
            1,
            "dropping/cancelling must close the HTTP request promptly"
        );
    }
}

#[test]
fn all_providers_pass_the_shared_behavior_contract_without_network() {
    use vellum_contracts::{fakes::ReplayTransport, suites};
    let transcripts = [
        vec![json!({"choices":[{"delta":{"content":"ready"},"finish_reason":"stop"}]})],
        vec![
            json!({"type":"response.output_text.delta","delta":"ready"}),
            json!({"type":"response.completed","response":{}}),
        ],
        vec![
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"ready"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}),
            json!({"type":"message_stop"}),
        ],
        vec![
            json!({"candidates":[{"content":{"parts":[{"text":"ready"}]},"finishReason":"STOP"}]}),
        ],
    ];
    for (index, transcript) in transcripts.into_iter().enumerate() {
        let mut wire: Vec<_> = transcript
            .into_iter()
            .map(|v| Ok(WireEvent::Data(v)))
            .collect();
        if index == 0 {
            wire.push(Ok(WireEvent::Done));
        }
        let transport = Arc::new(ReplayTransport::new(vec![wire]));
        let cfg = config("https://example.invalid/provider");
        let provider: Box<dyn ModelProvider> = match index {
            0 => Box::new(
                vellum_llm_openai_compat::Provider::new(cfg, secrets(), transport).unwrap(),
            ),
            1 => Box::new(
                vellum_llm_openai_responses::Provider::new(cfg, secrets(), transport).unwrap(),
            ),
            2 => Box::new(
                vellum_llm_anthropic::Provider::new(
                    cfg,
                    vellum_llm_anthropic::Deployment::Api,
                    secrets(),
                    transport,
                )
                .unwrap(),
            ),
            _ => Box::new(vellum_llm_google::Provider::new(cfg, secrets(), transport).unwrap()),
        };
        suites::model::provider(provider.as_ref());
        assert_eq!(provider.status(), ProviderStatus::Connected);
    }
}

#[test]
fn unsupported_capabilities_and_missing_credentials_do_not_reach_transport() {
    use vellum_contracts::fakes::ReplayTransport;
    let transport = Arc::new(ReplayTransport::new(vec![]));
    let mut cfg = config("https://example.invalid/provider");
    cfg.caps = ModelCaps::default();
    let provider =
        vellum_llm_openai_compat::Provider::new(cfg.clone(), secrets(), transport.clone()).unwrap();
    assert_eq!(
        collect(&provider, request()).unwrap_err(),
        ModelError::Unsupported
    );
    assert!(transport.requests().is_empty());
    cfg.caps = config("").caps;
    let provider = vellum_llm_openai_compat::Provider::new(
        cfg,
        Arc::new(MemorySecrets::default()),
        transport.clone(),
    )
    .unwrap();
    assert_eq!(provider.status(), ProviderStatus::ConfiguredExternally);
    assert_eq!(
        collect(&provider, request()).unwrap_err(),
        ModelError::Authentication
    );
    assert!(transport.requests().is_empty());
}

#[test]
fn cancelled_stream_does_not_emit_a_queued_tool_call() {
    use vellum_contracts::fakes::ReplayTransport;
    let transport = Arc::new(ReplayTransport::new(vec![vec![Ok(WireEvent::Data(
        json!({"candidates":[{"content":{"parts":[{"text":"ready"},{"functionCall":{"id":"call","name":"test","args":{}}}]},"finishReason":"STOP"}]}),
    ))]]));
    let provider = vellum_llm_google::Provider::new(
        config("https://example.invalid/provider"),
        secrets(),
        transport,
    )
    .unwrap();
    let cancel = Cancellation::default();
    let mut stream =
        block_on(provider.complete(ChatRequest::text("test", "hi"), cancel.clone())).unwrap();
    assert!(matches!(
        block_on(stream.next()),
        Some(Ok(ModelEvent::Text(_)))
    ));
    cancel.cancel();
    assert_eq!(
        block_on(stream.next()).unwrap().unwrap_err(),
        ModelError::Cancelled
    );
    assert!(block_on(stream.next()).is_none());
}

#[test]
#[ignore = "model-http: requires loopback sockets and ephemeral canary; run scripts/model-providers.mjs"]
fn provider_canary_artifacts_contain_no_credentials() {
    let canary = std::env::var("VELLUM_CANARY_API_KEY")
        .expect("canary runner must supply an ephemeral credential");
    let artifacts = std::env::var_os("VELLUM_CANARY_ARTIFACTS").expect("canary artifact directory");
    let store = Arc::new(MemorySecrets::default());
    store
        .set("test", SecretString::new(canary.clone()))
        .unwrap();
    let stub = StubEndpoint::start(vec![StubReply::http(401, &canary)]).unwrap();
    let cfg = config(&stub.url("/chat"));
    let config_debug = format!("{cfg:?}");
    let provider = vellum_llm_openai_compat::Provider::new(
        cfg,
        store.clone(),
        Arc::new(vellum_model_http::HttpTransport::default()),
    )
    .unwrap();
    let error = collect(&provider, ChatRequest::text("test", "hello")).unwrap_err();
    let evidence = format!(
        "{config_debug}\n{error:?}\n{error}\n{:?}\n{:?}",
        stub.requests(),
        store.get("test").unwrap()
    );
    assert!(
        !evidence.contains(&canary),
        "provider diagnostics must redact credentials"
    );
    std::fs::write(
        std::path::PathBuf::from(artifacts).join("provider.log"),
        evidence,
    )
    .unwrap();
}

#[test]
fn output_cannot_echo_a_credential_across_text_deltas() {
    use vellum_contracts::fakes::ReplayTransport;
    let transport = Arc::new(ReplayTransport::new(vec![vec![
        Ok(WireEvent::Data(
            json!({"choices":[{"delta":{"content":"Echo: stub-private-"}}]}),
        )),
        Ok(WireEvent::Data(
            json!({"choices":[{"delta":{"content":"credential"},"finish_reason":"stop"}]}),
        )),
        Ok(WireEvent::Done),
    ]]));
    let provider = vellum_llm_openai_compat::Provider::new(
        config("https://example.invalid/provider"),
        secrets(),
        transport,
    )
    .unwrap();
    let events = collect(&provider, ChatRequest::text("test", "hello")).unwrap();
    let text: String = events
        .into_iter()
        .filter_map(|e| {
            if let ModelEvent::Text(text) = e {
                Some(text)
            } else {
                None
            }
        })
        .collect();
    assert!(
        !text.contains("stub-private-credential"),
        "model output must redact the credential even across events"
    );
    assert!(text.contains("[redacted]"));
}

#[test]
fn native_tool_results_preserve_ids_and_continuation_metadata() {
    use vellum_contracts::fakes::ReplayTransport;
    for index in 0..4 {
        let terminal = match index {
            0 => vec![
                Ok(WireEvent::Data(
                    json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
                )),
                Ok(WireEvent::Done),
            ],
            1 => vec![Ok(WireEvent::Data(
                json!({"type":"response.completed","response":{}}),
            ))],
            2 => vec![
                Ok(WireEvent::Data(
                    json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}),
                )),
                Ok(WireEvent::Data(json!({"type":"message_stop"}))),
            ],
            _ => vec![Ok(WireEvent::Data(
                json!({"candidates":[{"finishReason":"STOP"}]}),
            ))],
        };
        let transport = Arc::new(ReplayTransport::new(vec![terminal]));
        let cfg = config("https://example.invalid/provider");
        let provider: Box<dyn ModelProvider> = match index {
            0 => Box::new(
                vellum_llm_openai_compat::Provider::new(cfg, secrets(), transport.clone()).unwrap(),
            ),
            1 => Box::new(
                vellum_llm_openai_responses::Provider::new(cfg, secrets(), transport.clone())
                    .unwrap(),
            ),
            2 => Box::new(
                vellum_llm_anthropic::Provider::new(
                    cfg,
                    vellum_llm_anthropic::Deployment::Api,
                    secrets(),
                    transport.clone(),
                )
                .unwrap(),
            ),
            _ => Box::new(
                vellum_llm_google::Provider::new(cfg, secrets(), transport.clone()).unwrap(),
            ),
        };
        let mut request = ChatRequest::text("test", "hello");
        let call = ModelCall {
            id: "call_1".into(),
            name: "insert_nodes".into(),
            arguments: json!({"nodes":[]}),
            metadata: json!({"thoughtSignature":"signature-from-model"}),
        };
        let mut assistant = vec![];
        if index == 1 {
            assistant.push(ChatPart::Continuation(
                json!({"type":"reasoning","id":"reason1","encrypted_content":"opaque"}),
            ));
        }
        if index == 2 {
            assistant.push(ChatPart::Continuation(
                json!({"type":"thinking","thinking":"summary","signature":"signed"}),
            ));
        }
        assistant.push(ChatPart::ToolCall(call));
        request.messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: assistant,
        });
        request.messages.push(ChatMessage {
            role: ChatRole::User,
            content: vec![ChatPart::ToolResult {
                id: "call_1".into(),
                name: "insert_nodes".into(),
                result: json!({"created":[]}),
                failed: false,
            }],
        });
        collect(provider.as_ref(), request).unwrap();
        let requests = transport.requests();
        let body = &requests[0];
        match index {
            0 => {
                assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
                assert_eq!(body["messages"][2]["tool_call_id"], "call_1");
            }
            1 => {
                assert_eq!(body["input"][1]["encrypted_content"], "opaque");
                assert_eq!(body["input"][2]["call_id"], "call_1");
                assert_eq!(body["input"][3]["type"], "function_call_output");
            }
            2 => {
                assert_eq!(body["messages"][1]["content"][0]["signature"], "signed");
                assert_eq!(body["messages"][2]["content"][0]["tool_use_id"], "call_1");
            }
            _ => {
                assert_eq!(
                    body["contents"][1]["parts"][0]["thoughtSignature"],
                    "signature-from-model"
                );
                assert_eq!(
                    body["contents"][2]["parts"][0]["functionResponse"]["id"],
                    "call_1"
                );
            }
        }
    }
}

#[test]
fn a_responses_refusal_is_not_reported_as_a_successful_stop() {
    use vellum_contracts::fakes::ReplayTransport;
    let transport = Arc::new(ReplayTransport::new(vec![vec![
        Ok(WireEvent::Data(
            json!({"type":"response.refusal.delta","delta":"refused"}),
        )),
        Ok(WireEvent::Data(
            json!({"type":"response.completed","response":{}}),
        )),
    ]]));
    let provider = vellum_llm_openai_responses::Provider::new(
        config("https://example.invalid/provider"),
        secrets(),
        transport,
    )
    .unwrap();
    let events = collect(&provider, ChatRequest::text("test", "hello")).unwrap();
    assert!(events.contains(&ModelEvent::Finished(FinishReason::Refused)));
}

#[test]
#[ignore = "model-http: requires loopback sockets; run scripts/model-providers.mjs"]
fn http_transport_obeys_the_shared_wire_contract() {
    let stub =
        StubEndpoint::start(vec![StubReply::sse(vec![json!({"probe":"ready"})], true)]).unwrap();
    vellum_contracts::suites::model::transport(
        &vellum_model_http::HttpTransport::default(),
        &stub.url("/probe"),
    );
    assert_eq!(
        stub.requests().len(),
        1,
        "cancelled call must never open a connection"
    );
}
