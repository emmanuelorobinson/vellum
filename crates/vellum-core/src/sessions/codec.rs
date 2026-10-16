//! The JSON an event's origin and body are stored as. Every store uses it, so a log moved between stores reads
//! the same. Field names are camelCase; reading refuses anything this version did not write.
use crate::ports::{
    ChatRole, EventBody, ImageRef, StepOutcome, StepRecord, StoreError, StoredMessage, StoredPart,
    Timestamp, Tokens, TurnTransition, UsageRecord,
};
use crate::turns::{TurnInput, TurnState};
use serde_json::{Map, Value, json};
use vellum_types::{Origin, TurnId};

pub fn origin_to_json(origin: &Origin) -> Value {
    serde_json::to_value(origin).expect("an origin is plain data")
}

pub fn origin_from_json(value: Value) -> Result<Origin, StoreError> {
    serde_json::from_value(value).map_err(|e| corrupt(format!("origin: {e}")))
}

fn corrupt(message: impl Into<String>) -> StoreError {
    StoreError::Corrupt(message.into())
}

fn image_json(image: &ImageRef) -> Value {
    Value::String(image.as_str().to_owned())
}

fn part_json(part: &StoredPart) -> Value {
    match part {
        StoredPart::Text(text) => json!({ "text": text }),
        StoredPart::Image { mime, image } => {
            json!({ "image": { "mime": mime, "hash": image.as_str() } })
        }
        StoredPart::ToolCall {
            id,
            name,
            arguments,
            metadata,
        } => {
            json!({ "toolCall": { "id": id, "name": name, "arguments": arguments, "metadata": metadata } })
        }
        StoredPart::ToolResult {
            id,
            name,
            result,
            failed,
        } => {
            json!({ "toolResult": { "id": id, "name": name, "result": result, "failed": failed } })
        }
        StoredPart::Continuation(value) => json!({ "continuation": value }),
    }
}

fn message_json(message: &StoredMessage) -> Value {
    json!({
        "role": match message.role { ChatRole::User => "user", ChatRole::Assistant => "assistant" },
        "parts": message.parts.iter().map(part_json).collect::<Vec<_>>(),
    })
}

pub fn body_to_json(body: &EventBody) -> Value {
    match body {
        EventBody::Prompt {
            text,
            context,
            images,
        } => json!({
            "type": "prompt",
            "text": text,
            "context": context,
            "images": images.iter().map(image_json).collect::<Vec<_>>(),
        }),
        EventBody::Messages(messages) => json!({
            "type": "messages",
            "messages": messages.iter().map(message_json).collect::<Vec<_>>(),
        }),
        EventBody::Plan { text } => json!({ "type": "plan", "text": text }),
        EventBody::Step(step) => {
            let mut out = json!({
                "type": "step",
                "tool": step.tool,
                "arguments": step.arguments,
                "result": step.result,
                "startedAt": step.started_at.0,
                "durationMs": step.duration_ms,
                "outcome": match step.outcome {
                    StepOutcome::Done => "done",
                    StepOutcome::Failed => "failed",
                    StepOutcome::Rejected => "rejected",
                },
                "detail": step.detail,
            });
            if let Some(image) = &step.thumbnail {
                out["thumbnail"] = image_json(image);
            }
            out
        }
        EventBody::Usage(usage) => {
            let mut out = json!({
                "type": "usage",
                "model": usage.model,
                "tokens": {
                    "input": usage.tokens.input,
                    "output": usage.tokens.output,
                    "cacheRead": usage.tokens.cache_read,
                    "cacheWrite": usage.tokens.cache_write,
                },
            });
            if let Some(micro) = usage.estimate {
                out["estimate"] = json!(micro);
            }
            out
        }
        EventBody::Turn(change) => json!({
            "type": "turn",
            "turn": change.turn.as_str(),
            "from": change.from.as_str(),
            "input": change.input.as_str(),
            "to": change.to.as_str(),
        }),
    }
}

/// Reads the fields of one JSON object, refusing any it was not asked about.
struct Fields<'a> {
    at: &'static str,
    object: &'a Map<String, Value>,
}
impl<'a> Fields<'a> {
    fn new(value: &'a Value, at: &'static str, allowed: &[&str]) -> Result<Self, StoreError> {
        let object = value
            .as_object()
            .ok_or_else(|| corrupt(format!("{at} is not an object")))?;
        if let Some(key) = object.keys().find(|k| !allowed.contains(&k.as_str())) {
            return Err(corrupt(format!("unknown field `{key}` in {at}")));
        }
        Ok(Self { at, object })
    }
    fn value(&self, key: &str) -> Result<&'a Value, StoreError> {
        self.object
            .get(key)
            .ok_or_else(|| corrupt(format!("{}.{key} is missing", self.at)))
    }
    fn str(&self, key: &str) -> Result<&'a str, StoreError> {
        self.value(key)?
            .as_str()
            .ok_or_else(|| corrupt(format!("{}.{key} is not text", self.at)))
    }
    fn u64(&self, key: &str) -> Result<u64, StoreError> {
        self.value(key)?
            .as_u64()
            .ok_or_else(|| corrupt(format!("{}.{key} is not a count", self.at)))
    }
    fn i64(&self, key: &str) -> Result<i64, StoreError> {
        self.value(key)?
            .as_i64()
            .ok_or_else(|| corrupt(format!("{}.{key} is not a time", self.at)))
    }
    fn bool(&self, key: &str) -> Result<bool, StoreError> {
        self.value(key)?
            .as_bool()
            .ok_or_else(|| corrupt(format!("{}.{key} is not true or false", self.at)))
    }
    fn array(&self, key: &str) -> Result<&'a Vec<Value>, StoreError> {
        self.value(key)?
            .as_array()
            .ok_or_else(|| corrupt(format!("{}.{key} is not a list", self.at)))
    }
}

fn image(value: &Value) -> Result<ImageRef, StoreError> {
    value
        .as_str()
        .and_then(ImageRef::new)
        .ok_or_else(|| corrupt("a picture reference is not a SHA-256"))
}

fn part(value: &Value) -> Result<StoredPart, StoreError> {
    let only = Fields::new(
        value,
        "part",
        &["text", "image", "toolCall", "toolResult", "continuation"],
    )?;
    if only.object.len() != 1 {
        return Err(corrupt("a part has exactly one kind"));
    }
    let (kind, inner) = only.object.iter().next().expect("one entry");
    Ok(match kind.as_str() {
        "text" => StoredPart::Text(
            inner
                .as_str()
                .ok_or_else(|| corrupt("part.text is not text"))?
                .to_owned(),
        ),
        "image" => {
            let f = Fields::new(inner, "part.image", &["mime", "hash"])?;
            StoredPart::Image {
                mime: f.str("mime")?.to_owned(),
                image: image(f.value("hash")?)?,
            }
        }
        "toolCall" => {
            let f = Fields::new(
                inner,
                "part.toolCall",
                &["id", "name", "arguments", "metadata"],
            )?;
            StoredPart::ToolCall {
                id: f.str("id")?.to_owned(),
                name: f.str("name")?.to_owned(),
                arguments: f.value("arguments")?.clone(),
                metadata: f.value("metadata")?.clone(),
            }
        }
        "toolResult" => {
            let f = Fields::new(
                inner,
                "part.toolResult",
                &["id", "name", "result", "failed"],
            )?;
            StoredPart::ToolResult {
                id: f.str("id")?.to_owned(),
                name: f.str("name")?.to_owned(),
                result: f.value("result")?.clone(),
                failed: f.bool("failed")?,
            }
        }
        _ => StoredPart::Continuation(inner.clone()),
    })
}

fn message(value: &Value) -> Result<StoredMessage, StoreError> {
    let f = Fields::new(value, "message", &["role", "parts"])?;
    let role = match f.str("role")? {
        "user" => ChatRole::User,
        "assistant" => ChatRole::Assistant,
        other => return Err(corrupt(format!("unknown role `{other}`"))),
    };
    Ok(StoredMessage {
        role,
        parts: f
            .array("parts")?
            .iter()
            .map(part)
            .collect::<Result<_, _>>()?,
    })
}

pub fn body_from_json(value: &Value) -> Result<EventBody, StoreError> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| corrupt("an event has no type"))?;
    Ok(match kind {
        "prompt" => {
            let f = Fields::new(value, "prompt", &["type", "text", "context", "images"])?;
            EventBody::Prompt {
                text: f.str("text")?.to_owned(),
                context: f
                    .array("context")?
                    .iter()
                    .map(|c| {
                        c.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| corrupt("prompt.context holds text"))
                    })
                    .collect::<Result<_, _>>()?,
                images: f
                    .array("images")?
                    .iter()
                    .map(image)
                    .collect::<Result<_, _>>()?,
            }
        }
        "messages" => {
            let f = Fields::new(value, "messages", &["type", "messages"])?;
            EventBody::Messages(
                f.array("messages")?
                    .iter()
                    .map(message)
                    .collect::<Result<_, _>>()?,
            )
        }
        "plan" => {
            let f = Fields::new(value, "plan", &["type", "text"])?;
            EventBody::Plan {
                text: f.str("text")?.to_owned(),
            }
        }
        "step" => {
            let f = Fields::new(
                value,
                "step",
                &[
                    "type",
                    "tool",
                    "arguments",
                    "result",
                    "thumbnail",
                    "startedAt",
                    "durationMs",
                    "outcome",
                    "detail",
                ],
            )?;
            EventBody::Step(StepRecord {
                tool: f.str("tool")?.to_owned(),
                arguments: f.str("arguments")?.to_owned(),
                result: f.str("result")?.to_owned(),
                thumbnail: f.object.get("thumbnail").map(image).transpose()?,
                started_at: Timestamp(f.i64("startedAt")?),
                duration_ms: f.u64("durationMs")?,
                outcome: match f.str("outcome")? {
                    "done" => StepOutcome::Done,
                    "failed" => StepOutcome::Failed,
                    "rejected" => StepOutcome::Rejected,
                    other => return Err(corrupt(format!("unknown step outcome `{other}`"))),
                },
                detail: f.value("detail")?.clone(),
            })
        }
        "usage" => {
            let f = Fields::new(value, "usage", &["type", "model", "tokens", "estimate"])?;
            let t = Fields::new(
                f.value("tokens")?,
                "usage.tokens",
                &["input", "output", "cacheRead", "cacheWrite"],
            )?;
            EventBody::Usage(UsageRecord {
                model: f.str("model")?.to_owned(),
                tokens: Tokens {
                    input: t.u64("input")?,
                    output: t.u64("output")?,
                    cache_read: t.u64("cacheRead")?,
                    cache_write: t.u64("cacheWrite")?,
                },
                estimate: match f.object.get("estimate") {
                    None => None,
                    Some(_) => Some(f.u64("estimate")?),
                },
            })
        }
        "turn" => {
            let f = Fields::new(value, "turn", &["type", "turn", "from", "input", "to"])?;
            let state = |key: &str| -> Result<TurnState, StoreError> {
                let name = f.str(key)?;
                TurnState::parse(name)
                    .ok_or_else(|| corrupt(format!("unknown turn state `{name}`")))
            };
            let input = f.str("input")?;
            EventBody::Turn(TurnTransition {
                turn: TurnId::new(f.str("turn")?).map_err(|_| corrupt("turn is not a ULID"))?,
                from: state("from")?,
                input: TurnInput::parse(input)
                    .ok_or_else(|| corrupt(format!("unknown turn input `{input}`")))?,
                to: state("to")?,
            })
        }
        other => return Err(corrupt(format!("unknown event type `{other}`"))),
    })
}
