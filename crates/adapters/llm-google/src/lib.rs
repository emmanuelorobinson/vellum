//! Gemini GenerateContent SSE with native function calls and opaque thought signatures.
use serde_json::{Value, json};
use std::sync::Arc;
use vellum_core::ports::*;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
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
            let body = encode(&req);
            let wire = self
                .0
                .open(&req, body, "x-goog-api-key", false, vec![], cancel.clone())
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
fn encode(req: &ChatRequest) -> Value {
    let contents: Vec<_> = req.messages.iter().map(|message| {
        let parts: Vec<Value> = message.content.iter().map(|part| match part {
            ChatPart::Text(text) => json!({"text":text}),
            ChatPart::Image { mime, base64 } => json!({"inlineData":{"mimeType":mime,"data":base64}}),
            ChatPart::ToolCall(call) => {
                let mut part = json!({"functionCall":{"id":call.id,"name":call.name,"args":call.arguments}});
                if let Some(signature) = call.metadata.get("thoughtSignature") { part["thoughtSignature"] = signature.clone(); }
                part
            }
            ChatPart::ToolResult { id, name, result, failed } => json!({"functionResponse":{"id":id,"name":name,"response":if *failed { json!({"error":result}) } else { json!({"result":result}) }}}),
            ChatPart::Continuation(part) => part.clone(),
        }).collect();
        json!({"role":if message.role == ChatRole::User { "user" } else { "model" },"parts":parts})
    }).collect();
    let mut body = json!({"contents":contents,"systemInstruction":{"parts":[{"text":req.system}]},"generationConfig":{"maxOutputTokens":req.max_output_tokens}});
    if !req.tools.is_empty() {
        body["tools"] = json!([{"functionDeclarations":req.tools.iter().map(|t| json!({"name":t.name,"description":t.description,"parameters":t.parameters})).collect::<Vec<_>>()}]);
    }
    if let Some(effort) = req.reasoning {
        body["generationConfig"]["thinkingConfig"] = json!({"thinkingLevel":effort.as_str()});
    }
    body
}
#[derive(Default)]
struct Decoder {
    call_index: u64,
    has_calls: bool,
}
impl ModelDecoder for Decoder {
    fn decode(&mut self, event: WireEvent) -> Result<Vec<ModelEvent>, ModelError> {
        let WireEvent::Data(value) = event else {
            return Err(ModelError::Truncated);
        };
        if value.get("error").is_some() {
            return Err(ModelError::Unavailable);
        }
        let mut events = vec![];
        if let Some(usage) = value.get("usageMetadata") {
            events.push(ModelEvent::Usage(ModelUsage {
                input_tokens: usage["promptTokenCount"].as_u64(),
                output_tokens: usage["candidatesTokenCount"].as_u64(),
                reasoning_tokens: usage["thoughtsTokenCount"].as_u64(),
                cached_input_tokens: usage["cachedContentTokenCount"].as_u64(),
            }));
        }
        if value["promptFeedback"].get("blockReason").is_some() {
            events.push(ModelEvent::Finished(FinishReason::Refused));
            return Ok(events);
        }
        for candidate in value["candidates"].as_array().ok_or(ModelError::Protocol)? {
            if candidate["index"].as_u64().is_some_and(|i| i != 0) {
                continue;
            }
            if let Some(parts) = candidate["content"]["parts"].as_array() {
                for part in parts {
                    if let Some(text) = part["text"].as_str() {
                        if part["thought"] == true {
                            events.push(ModelEvent::Reasoning(text.into()));
                        } else {
                            events.push(ModelEvent::Text(text.into()));
                        }
                        if part.get("thoughtSignature").is_some() {
                            events.push(ModelEvent::Continuation(part.clone()));
                        }
                    }
                    if let Some(call) = part.get("functionCall") {
                        if !call["args"].is_object() {
                            return Err(ModelError::Protocol);
                        }
                        let id = call["id"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("gemini-{}", self.call_index));
                        self.call_index += 1;
                        self.has_calls = true;
                        events.push(ModelEvent::ToolCall(ModelCall {
                            id,
                            name: call["name"].as_str().ok_or(ModelError::Protocol)?.into(),
                            arguments: call["args"].clone(),
                            metadata: json!({"thoughtSignature":part["thoughtSignature"]}),
                        }));
                    }
                }
            }
            if let Some(reason) = candidate["finishReason"].as_str() {
                events.push(ModelEvent::Finished(match reason {
                    "STOP" if self.has_calls => FinishReason::ToolCalls,
                    "STOP" => FinishReason::Stop,
                    "MAX_TOKENS" => FinishReason::Length,
                    "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" => {
                        FinishReason::Refused
                    }
                    _ => return Err(ModelError::Unsupported),
                }));
            }
        }
        Ok(events)
    }
}
