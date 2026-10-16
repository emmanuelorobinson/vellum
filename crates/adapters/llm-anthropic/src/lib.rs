//! Anthropic Messages, including the native Bedrock SSE route and Vertex rawPredict.
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use vellum_core::ports::*;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deployment {
    Api,
    Bedrock,
    Vertex,
    Foundry,
    Custom,
}
pub struct Provider {
    connection: ProviderConnection,
    deployment: Deployment,
}
impl Provider {
    /// endpoint is the complete Messages URL, or Vertex :streamRawPredict URL.
    /// Bedrock uses /anthropic/v1/messages with a Bedrock key/token; cloud credential
    /// acquisition/refresh belongs to the host, never ambient SDK discovery here.
    pub fn new(
        config: ModelConfig,
        deployment: Deployment,
        secrets: Arc<dyn SecretStore>,
        transport: Arc<dyn ModelTransport>,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            connection: ProviderConnection::new(config, secrets, transport)?,
            deployment,
        })
    }
}
impl ModelProvider for Provider {
    fn id(&self) -> &str {
        &self.connection.config.id
    }
    fn capabilities(&self) -> ModelCaps {
        self.connection.config.caps
    }
    fn status(&self) -> ProviderStatus {
        self.connection.status()
    }
    fn complete(
        &self,
        req: ChatRequest,
        cancel: Cancellation,
    ) -> ModelFuture<'_, Result<ChatStream, ModelError>> {
        Box::pin(async move {
            let body = encode(&req, self.deployment);
            let (header, bearer) = match self.deployment {
                Deployment::Vertex => ("Authorization", true),
                Deployment::Foundry => ("api-key", false),
                _ => ("x-api-key", false),
            };
            let headers = vec![("anthropic-version".into(), "2023-06-01".into())];
            let wire = self
                .connection
                .open(&req, body, header, bearer, headers, cancel.clone())
                .await?;
            Ok(decoded_stream(
                wire,
                Decoder::default(),
                self.connection.state.clone(),
                cancel,
            ))
        })
    }
}
fn encode(req: &ChatRequest, deployment: Deployment) -> Value {
    let messages: Vec<_> = req.messages.iter().map(|message| {
        let content: Vec<Value> = message.content.iter().map(|part| match part {
            ChatPart::Text(text) => json!({"type":"text","text":text}),
            ChatPart::Image { mime, base64 } => json!({"type":"image","source":{"type":"base64","media_type":mime,"data":base64}}),
            ChatPart::ToolCall(call) => json!({"type":"tool_use","id":call.id,"name":call.name,"input":call.arguments}),
            ChatPart::ToolResult { id, result, failed, .. } => json!({"type":"tool_result","tool_use_id":id,"content":result.to_string(),"is_error":failed}),
            ChatPart::Continuation(value) => value.clone(),
        }).collect();
        json!({"role":if message.role == ChatRole::User { "user" } else { "assistant" },"content":content})
    }).collect();
    let mut body = json!({"model":req.model,"system":req.system,"messages":messages,"max_tokens":req.max_output_tokens,"stream":true});
    if !req.tools.is_empty() {
        body["tools"] = req
            .tools
            .iter()
            .map(|t| json!({"name":t.name,"description":t.description,"input_schema":t.parameters}))
            .collect();
    }
    if let Some(effort) = req.reasoning {
        body["thinking"] = json!({"type":"adaptive"});
        body["output_config"] = json!({"effort":effort.as_str()});
    }
    if deployment == Deployment::Vertex {
        body.as_object_mut().unwrap().remove("model");
        body["anthropic_version"] = json!("vertex-2023-10-16");
    }
    body
}
#[derive(Default)]
struct Decoder {
    calls: BTreeMap<u64, CallAccumulator>,
    blocks: BTreeMap<u64, Value>,
    reason: Option<FinishReason>,
    usage: ModelUsage,
}
impl ModelDecoder for Decoder {
    fn decode(&mut self, event: WireEvent) -> Result<Vec<ModelEvent>, ModelError> {
        let WireEvent::Data(value) = event else {
            return Err(ModelError::Truncated);
        };
        let mut events = vec![];
        match value["type"].as_str().ok_or(ModelError::Protocol)? {
            "message_start" => {
                let usage = &value["message"]["usage"];
                self.usage.input_tokens = usage["input_tokens"].as_u64();
                self.usage.cached_input_tokens = usage["cache_read_input_tokens"].as_u64();
            }
            "content_block_start" => {
                let index = value["index"].as_u64().ok_or(ModelError::Protocol)?;
                let block = &value["content_block"];
                if self.calls.len() + self.blocks.len() >= 128 {
                    return Err(ModelError::Limit);
                }
                match block["type"].as_str() {
                    Some("tool_use") => {
                        self.calls.insert(
                            index,
                            CallAccumulator {
                                id: block["id"].as_str().ok_or(ModelError::Protocol)?.into(),
                                name: block["name"].as_str().ok_or(ModelError::Protocol)?.into(),
                                ..Default::default()
                            },
                        );
                    }
                    Some("thinking" | "redacted_thinking") => {
                        self.blocks.insert(index, block.clone());
                    }
                    Some("text") => {
                        if let Some(text) = block["text"].as_str().filter(|t| !t.is_empty()) {
                            events.push(ModelEvent::Text(text.into()));
                        }
                    }
                    _ => return Err(ModelError::Unsupported),
                }
            }
            "content_block_delta" => {
                let index = value["index"].as_u64().ok_or(ModelError::Protocol)?;
                let delta = &value["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => events.push(ModelEvent::Text(
                        delta["text"].as_str().ok_or(ModelError::Protocol)?.into(),
                    )),
                    Some("input_json_delta") => self
                        .calls
                        .get_mut(&index)
                        .ok_or(ModelError::Protocol)?
                        .append(delta["partial_json"].as_str().ok_or(ModelError::Protocol)?)?,
                    Some("thinking_delta" | "signature_delta") => {
                        let field = if delta["type"] == "thinking_delta" {
                            "thinking"
                        } else {
                            "signature"
                        };
                        let text = delta[field].as_str().ok_or(ModelError::Protocol)?;
                        let block = self.blocks.get_mut(&index).ok_or(ModelError::Protocol)?;
                        let mut previous = block[field].as_str().unwrap_or("").to_owned();
                        if previous.len() + text.len() > 1024 * 1024 {
                            return Err(ModelError::Limit);
                        }
                        previous.push_str(text);
                        block[field] = json!(previous);
                        if field == "thinking" {
                            events.push(ModelEvent::Reasoning(text.into()));
                        }
                    }
                    _ => return Err(ModelError::Unsupported),
                }
            }
            "content_block_stop" => {
                let index = value["index"].as_u64().ok_or(ModelError::Protocol)?;
                if let Some(call) = self.calls.remove(&index) {
                    events.push(ModelEvent::ToolCall(call.finish()?));
                }
                if let Some(block) = self.blocks.remove(&index) {
                    events.push(ModelEvent::Continuation(block));
                }
            }
            "message_delta" => {
                if let Some(reason) = value["delta"]["stop_reason"].as_str() {
                    self.reason = Some(match reason {
                        "end_turn" | "stop_sequence" => FinishReason::Stop,
                        "tool_use" => FinishReason::ToolCalls,
                        "max_tokens" => FinishReason::Length,
                        "refusal" => FinishReason::Refused,
                        _ => return Err(ModelError::Unsupported),
                    });
                }
                self.usage.output_tokens = value["usage"]["output_tokens"].as_u64();
                self.usage.reasoning_tokens =
                    value["usage"]["output_tokens_details"]["thinking_tokens"].as_u64();
            }
            "message_stop" => {
                if !self.calls.is_empty() || !self.blocks.is_empty() {
                    return Err(ModelError::Truncated);
                }
                events.push(ModelEvent::Usage(self.usage));
                events.push(ModelEvent::Finished(
                    self.reason.ok_or(ModelError::Truncated)?,
                ));
            }
            "error" => return Err(ModelError::Unavailable),
            "ping" => {}
            _ => {}
        }
        Ok(events)
    }
}
