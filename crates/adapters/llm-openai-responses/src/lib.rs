//! Responses protocol; full-history replay with server storage disabled.
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
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
            let wire = self
                .0
                .open(
                    &req,
                    encode(&req),
                    "Authorization",
                    true,
                    vec![],
                    cancel.clone(),
                )
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
    let mut input = vec![];
    for message in &req.messages {
        let mut content = vec![];
        let flush = |input: &mut Vec<Value>, content: &mut Vec<Value>| {
            if !content.is_empty() {
                input.push(json!({"role":if message.role == ChatRole::User { "user" } else { "assistant" },"content":std::mem::take(content)}));
            }
        };
        for part in &message.content {
            match part {
                ChatPart::Text(text) => content.push(json!({"type":if message.role == ChatRole::Assistant { "output_text" } else { "input_text" },"text":text})),
                ChatPart::Image { mime, base64 } => content.push(json!({"type":"input_image","image_url":format!("data:{mime};base64,{base64}")})),
                part => {
                    flush(&mut input, &mut content);
                    match part {
                        ChatPart::ToolCall(call) => input.push(json!({"type":"function_call","call_id":call.id,"name":call.name,"arguments":call.arguments.to_string()})),
                        ChatPart::ToolResult { id, result, .. } => input.push(json!({"type":"function_call_output","call_id":id,"output":result.to_string()})),
                        ChatPart::Continuation(item) => input.push(item.clone()),
                        _ => unreachable!("text and images are handled above"),
                    }
                }
            }
        }
        flush(&mut input, &mut content);
    }
    let mut body = json!({"model":req.model,"instructions":req.system,"input":input,"stream":true,"store":false,"max_output_tokens":req.max_output_tokens});
    if !req.tools.is_empty() {
        body["tools"] = req.tools.iter().map(|t| json!({"type":"function","name":t.name,"description":t.description,"parameters":t.parameters})).collect();
    }
    if let Some(effort) = req.reasoning {
        body["reasoning"] = json!({"effort":effort.as_str()});
    }
    body
}
#[derive(Default)]
struct Decoder {
    calls: BTreeMap<u64, CallAccumulator>,
    refused: bool,
}
impl ModelDecoder for Decoder {
    fn decode(&mut self, event: WireEvent) -> Result<Vec<ModelEvent>, ModelError> {
        let WireEvent::Data(value) = event else {
            return Err(ModelError::Truncated);
        };
        let mut events = vec![];
        match value["type"].as_str().ok_or(ModelError::Protocol)? {
            "response.output_text.delta" => events.push(ModelEvent::Text(
                value["delta"].as_str().ok_or(ModelError::Protocol)?.into(),
            )),
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => events
                .push(ModelEvent::Reasoning(
                    value["delta"].as_str().ok_or(ModelError::Protocol)?.into(),
                )),
            "response.output_item.added" if value["item"]["type"] == "function_call" => {
                if self.calls.len() >= 128 {
                    return Err(ModelError::Limit);
                }
                self.calls.insert(
                    value["output_index"].as_u64().ok_or(ModelError::Protocol)?,
                    CallAccumulator {
                        id: value["item"]["call_id"]
                            .as_str()
                            .ok_or(ModelError::Protocol)?
                            .into(),
                        name: value["item"]["name"]
                            .as_str()
                            .ok_or(ModelError::Protocol)?
                            .into(),
                        arguments: value["item"]["arguments"].as_str().unwrap_or("").into(),
                        metadata: Value::Null,
                    },
                );
            }
            "response.function_call_arguments.delta" => {
                let call = self
                    .calls
                    .get_mut(&value["output_index"].as_u64().ok_or(ModelError::Protocol)?)
                    .ok_or(ModelError::Protocol)?;
                call.append(value["delta"].as_str().ok_or(ModelError::Protocol)?)?;
            }
            "response.output_item.done" if value["item"]["type"] == "reasoning" => {
                events.push(ModelEvent::Continuation(value["item"].clone()))
            }
            "response.completed" | "response.incomplete" => {
                let reason = if self.refused {
                    FinishReason::Refused
                } else if value["type"] == "response.incomplete" {
                    if value["response"]["incomplete_details"]["reason"] == "max_output_tokens" {
                        FinishReason::Length
                    } else {
                        FinishReason::Refused
                    }
                } else if self.calls.is_empty() {
                    FinishReason::Stop
                } else {
                    FinishReason::ToolCalls
                };
                for call in std::mem::take(&mut self.calls).into_values() {
                    events.push(ModelEvent::ToolCall(call.finish()?));
                }
                let usage = &value["response"]["usage"];
                events.push(ModelEvent::Usage(ModelUsage {
                    input_tokens: usage["input_tokens"].as_u64(),
                    output_tokens: usage["output_tokens"].as_u64(),
                    reasoning_tokens: usage["output_tokens_details"]["reasoning_tokens"].as_u64(),
                    cached_input_tokens: usage["input_tokens_details"]["cached_tokens"].as_u64(),
                    cache_write_tokens: None,
                }));
                events.push(ModelEvent::Finished(reason));
            }
            "response.refusal.delta" | "response.refusal.done" => self.refused = true,
            "error" | "response.failed" => return Err(ModelError::Unavailable),
            _ => {}
        }
        Ok(events)
    }
}
