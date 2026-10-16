//! Chat Completions protocol with explicit model capabilities and credential lookup.
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use vellum_core::ports::*;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub const OPENROUTER_DEFAULT_MODEL: &str = "deepseek/deepseek-v4.1-flash";
pub struct Preset {
    pub id: &'static str,
    pub endpoint: &'static str,
}
pub fn presets() -> &'static [Preset] {
    &[
        Preset {
            id: "openrouter",
            endpoint: "https://openrouter.ai/api/v1/chat/completions",
        },
        Preset {
            id: "deepseek",
            endpoint: "https://api.deepseek.com/chat/completions",
        },
        Preset {
            id: "xai",
            endpoint: "https://api.x.ai/v1/chat/completions",
        },
        Preset {
            id: "moonshot",
            endpoint: "https://api.moonshot.ai/v1/chat/completions",
        },
        Preset {
            id: "together",
            endpoint: "https://api.together.xyz/v1/chat/completions",
        },
        Preset {
            id: "fireworks",
            endpoint: "https://api.fireworks.ai/inference/v1/chat/completions",
        },
        Preset {
            id: "zai",
            endpoint: "https://api.z.ai/api/paas/v4/chat/completions",
        },
        Preset {
            id: "opencode",
            endpoint: "https://opencode.ai/zen/v1/chat/completions",
        },
        Preset {
            id: "ollama",
            endpoint: "http://localhost:11434/v1/chat/completions",
        },
        Preset {
            id: "lmstudio",
            endpoint: "http://localhost:1234/v1/chat/completions",
        },
    ]
}
pub struct Provider(ProviderConnection);
impl Provider {
    pub fn new(
        config: ModelConfig,
        secrets: Arc<dyn SecretStore>,
        transport: Arc<dyn ModelTransport>,
    ) -> Result<Self, ModelError> {
        Ok(Self(ProviderConnection::new(config, secrets, transport)?))
    }
}
impl ModelProvider for Provider {
    fn id(&self) -> &str {
        &self.0.config.id
    }
    fn capabilities(&self) -> ModelCaps {
        self.0.config.caps
    }
    fn status(&self) -> ProviderStatus {
        self.0.status()
    }
    fn complete(
        &self,
        req: ChatRequest,
        cancel: Cancellation,
    ) -> ModelFuture<'_, Result<ChatStream, ModelError>> {
        Box::pin(async move {
            let body = encode(&req)?;
            let wire = self
                .0
                .open(&req, body, "Authorization", true, vec![], cancel.clone())
                .await?;
            Ok(decoded_stream(
                wire,
                Decoder::default(),
                self.0.state.clone(),
                cancel,
            ))
        })
    }
}
fn encode(req: &ChatRequest) -> Result<Value, ModelError> {
    let mut messages = vec![];
    if !req.system.is_empty() {
        messages.push(json!({"role":"system","content":req.system}));
    }
    for message in &req.messages {
        let mut content = vec![];
        let mut calls = vec![];
        let mut results = vec![];
        for part in &message.content {
            match part {
                ChatPart::Text(text) => content.push(json!({"type":"text","text":text})),
                ChatPart::Image { mime, base64 } => content.push(json!({"type":"image_url","image_url":{"url":format!("data:{mime};base64,{base64}")}})),
                ChatPart::ToolCall(call) => calls.push(json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":call.arguments.to_string()}})),
                ChatPart::ToolResult { id, result, .. } => results.push(json!({"role":"tool","tool_call_id":id,"content":result.to_string()})),
                ChatPart::Continuation(_) => return Err(ModelError::Unsupported),
            }
        }
        // Tool messages answer the previous assistant message, so they come before any user content (a
        // screenshot's picture follows its tool result).
        messages.extend(results);
        if !content.is_empty() || !calls.is_empty() {
            let mut value = json!({"role":if message.role == ChatRole::User { "user" } else { "assistant" },"content":content});
            if !calls.is_empty() {
                value["tool_calls"] = json!(calls);
            }
            messages.push(value);
        }
    }
    let mut body = json!({"model":req.model,"messages":messages,"stream":true,"stream_options":{"include_usage":true},"max_tokens":req.max_output_tokens});
    if !req.tools.is_empty() {
        body["tools"] = req.tools.iter().map(|t| json!({"type":"function","function":{"name":t.name,"description":t.description,"parameters":t.parameters}})).collect();
    }
    if let Some(effort) = req.reasoning {
        body["reasoning_effort"] = json!(effort.as_str());
    }
    Ok(body)
}
#[derive(Default)]
struct Decoder {
    calls: BTreeMap<u64, CallAccumulator>,
    reason: Option<FinishReason>,
}
impl ModelDecoder for Decoder {
    fn decode(&mut self, event: WireEvent) -> Result<Vec<ModelEvent>, ModelError> {
        let mut events = vec![];
        let WireEvent::Data(value) = event else {
            let reason = self.reason.ok_or(ModelError::Truncated)?;
            for call in std::mem::take(&mut self.calls).into_values() {
                events.push(ModelEvent::ToolCall(call.finish()?));
            }
            events.push(ModelEvent::Finished(reason));
            return Ok(events);
        };
        if value.get("error").is_some() {
            return Err(ModelError::Unavailable);
        }
        if let Some(usage) = value.get("usage").filter(|u| u.is_object()) {
            events.push(ModelEvent::Usage(ModelUsage {
                input_tokens: usage["prompt_tokens"].as_u64(),
                output_tokens: usage["completion_tokens"].as_u64(),
                reasoning_tokens: usage["completion_tokens_details"]["reasoning_tokens"].as_u64(),
                cached_input_tokens: usage["prompt_tokens_details"]["cached_tokens"].as_u64(),
                cache_write_tokens: None,
            }));
        }
        for choice in value["choices"].as_array().ok_or(ModelError::Protocol)? {
            if choice["index"].as_u64().is_some_and(|i| i != 0) {
                continue;
            }
            let delta = &choice["delta"];
            if let Some(text) = delta["content"].as_str() {
                events.push(ModelEvent::Text(text.into()));
            }
            if let Some(text) = delta["reasoning_content"].as_str() {
                events.push(ModelEvent::Reasoning(text.into()));
            }
            if let Some(calls) = delta["tool_calls"].as_array() {
                for call in calls {
                    let index = call["index"].as_u64().ok_or(ModelError::Protocol)?;
                    if self.calls.len() >= 128 && !self.calls.contains_key(&index) {
                        return Err(ModelError::Limit);
                    }
                    let pending = self.calls.entry(index).or_default();
                    if let Some(id) = call["id"].as_str() {
                        pending.id.push_str(id);
                    }
                    if let Some(name) = call["function"]["name"].as_str() {
                        pending.name.push_str(name);
                    }
                    if let Some(args) = call["function"]["arguments"].as_str() {
                        pending.append(args)?;
                    }
                }
            }
            if let Some(reason) = choice["finish_reason"].as_str() {
                self.reason = Some(match reason {
                    "stop" => FinishReason::Stop,
                    "tool_calls" => FinishReason::ToolCalls,
                    "length" => FinishReason::Length,
                    "content_filter" => FinishReason::Refused,
                    _ => return Err(ModelError::Protocol),
                });
            }
        }
        Ok(events)
    }
}
