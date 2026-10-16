//! The JSON an event's origin and body are stored as. Every store uses it, so a log moved between stores reads
//! the same. Field names are camelCase; reading refuses anything this version did not write.
use crate::plans::{Placeholder, Plan, PlanEvent, PlanItem};
use crate::ports::Rect;
use crate::ports::{
    Action, Answer, ChatRole, DecisionSource, EventBody, EventSeq, ImageRef, PermissionKind,
    PermissionRecord, StepOutcome, StepRecord, StoreError, StoredMessage, StoredPart, Timestamp,
    Tokens, TurnTransition, UsageRecord,
};
use crate::turns::{TurnInput, TurnState};
use serde_json::{Map, Value, json};
use vellum_types::{AgentId, NodeId, Origin, PageId, TurnId};

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
        EventBody::Plan(event) => plan_json(event),
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
        EventBody::Permission(record) => {
            let mut out = json!({
                "type": "permission",
                "action": action_json(&record.action),
                "kind": record.kind.as_str(),
                "answer": record.answer.as_str(),
                "source": source_json(&record.source),
            });
            if let Some(engine) = &record.engine {
                out["engine"] = json!(engine);
            }
            out
        }
    }
}

fn action_json(action: &Action) -> Value {
    match action {
        Action::ReadFile { path } => json!({ "readFile": { "path": path } }),
        Action::WriteFile { path } => json!({ "writeFile": { "path": path } }),
        Action::RunCommand { program, args, cwd } => {
            json!({ "runCommand": { "program": program, "args": args, "cwd": cwd } })
        }
        Action::Network { url } => json!({ "network": { "url": url } }),
    }
}

fn source_json(source: &DecisionSource) -> Value {
    let mut out = json!({ "kind": source.kind() });
    match source {
        DecisionSource::SessionGrant { grant } => out["grant"] = json!(grant.0),
        DecisionSource::Rule { rule } => out["rule"] = json!(rule),
        DecisionSource::EngineAuto { engine } => out["engine"] = json!(engine),
        DecisionSource::Failed { reason } => out["reason"] = json!(reason),
        DecisionSource::Person
        | DecisionSource::SkipAll
        | DecisionSource::Unattended
        | DecisionSource::TurnStopped => {}
    }
    out
}

fn action(value: &Value) -> Result<Action, StoreError> {
    let only = Fields::new(
        value,
        "permission.action",
        &["readFile", "writeFile", "runCommand", "network"],
    )?;
    if only.object.len() != 1 {
        return Err(corrupt("an action has exactly one kind"));
    }
    let (kind, inner) = only.object.iter().next().expect("one entry");
    Ok(match kind.as_str() {
        "readFile" => Action::ReadFile {
            path: Fields::new(inner, "readFile", &["path"])?
                .str("path")?
                .to_owned(),
        },
        "writeFile" => Action::WriteFile {
            path: Fields::new(inner, "writeFile", &["path"])?
                .str("path")?
                .to_owned(),
        },
        "runCommand" => {
            let f = Fields::new(inner, "runCommand", &["program", "args", "cwd"])?;
            Action::RunCommand {
                program: f.str("program")?.to_owned(),
                args: f
                    .array("args")?
                    .iter()
                    .map(|a| {
                        a.as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| corrupt("runCommand.args holds text"))
                    })
                    .collect::<Result<_, _>>()?,
                cwd: f.str("cwd")?.to_owned(),
            }
        }
        _ => Action::Network {
            url: Fields::new(inner, "network", &["url"])?
                .str("url")?
                .to_owned(),
        },
    })
}

fn source(value: &Value) -> Result<DecisionSource, StoreError> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| corrupt("permission.source has no kind"))?;
    let with = |extra: &'static str| -> Result<Fields<'_>, StoreError> {
        let allowed: &[&str] = if extra.is_empty() {
            &["kind"]
        } else {
            match extra {
                "grant" => &["kind", "grant"],
                "rule" => &["kind", "rule"],
                "engine" => &["kind", "engine"],
                _ => &["kind", "reason"],
            }
        };
        Fields::new(value, "permission.source", allowed)
    };
    Ok(match kind {
        "person" => {
            with("")?;
            DecisionSource::Person
        }
        "session_grant" => DecisionSource::SessionGrant {
            grant: EventSeq(with("grant")?.u64("grant")?),
        },
        "rule" => DecisionSource::Rule {
            rule: with("rule")?.str("rule")?.to_owned(),
        },
        "engine_auto" => DecisionSource::EngineAuto {
            engine: with("engine")?.str("engine")?.to_owned(),
        },
        "skip_all" => {
            with("")?;
            DecisionSource::SkipAll
        }
        "unattended" => {
            with("")?;
            DecisionSource::Unattended
        }
        "turn_stopped" => {
            with("")?;
            DecisionSource::TurnStopped
        }
        "failed" => DecisionSource::Failed {
            reason: with("reason")?.str("reason")?.to_owned(),
        },
        other => return Err(corrupt(format!("unknown decision source `{other}`"))),
    })
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
        "plan" => EventBody::Plan(plan_event(value)?),
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
        "permission" => {
            let f = Fields::new(
                value,
                "permission",
                &["type", "action", "kind", "engine", "answer", "source"],
            )?;
            let kind = f.str("kind")?;
            let answer = f.str("answer")?;
            EventBody::Permission(PermissionRecord {
                action: action(f.value("action")?)?,
                kind: PermissionKind::parse(kind)
                    .ok_or_else(|| corrupt(format!("unknown permission kind `{kind}`")))?,
                engine: match f.object.get("engine") {
                    None => None,
                    Some(_) => Some(f.str("engine")?.to_owned()),
                },
                answer: Answer::parse(answer)
                    .ok_or_else(|| corrupt(format!("unknown permission answer `{answer}`")))?,
                source: source(f.value("source")?)?,
            })
        }
        other => return Err(corrupt(format!("unknown event type `{other}`"))),
    })
}

fn plan_item_json(item: &PlanItem) -> Value {
    let mut out = json!({
        "id": item.id,
        "name": item.name,
        "page": item.page.as_str(),
        "intent": item.intent,
    });
    if let Some(width) = item.width {
        out["width"] = json!(width);
    }
    if let Some(height) = item.height {
        out["height"] = json!(height);
    }
    if let Some(frame) = &item.frame {
        out["frame"] = json!(frame.as_str());
    }
    out
}

fn plan_body_json(plan: &Plan) -> Value {
    let mut out = json!({
        "title": plan.title,
        "items": plan.items.iter().map(plan_item_json).collect::<Vec<_>>(),
    });
    if let Some(changes) = plan.changes {
        out["changes"] = json!(changes);
    }
    out
}

fn placeholder_json(p: &Placeholder) -> Value {
    json!({
        "item": p.item,
        "frame": p.frame.as_str(),
        "name": p.name,
        "page": p.page.as_str(),
        "bounds": {"x": p.bounds.x, "y": p.bounds.y, "width": p.bounds.width, "height": p.bounds.height},
        "agent": p.agent.as_str(),
        "turn": p.turn.as_str(),
    })
}

fn plan_json(event: &PlanEvent) -> Value {
    match event {
        PlanEvent::Proposed(plan) => {
            json!({"type": "plan", "event": "proposed", "plan": plan_body_json(plan)})
        }
        PlanEvent::Built { plan, placeholders } => json!({
            "type": "plan",
            "event": "built",
            "plan": plan_body_json(plan),
            "placeholders": placeholders.iter().map(placeholder_json).collect::<Vec<_>>(),
        }),
        PlanEvent::Ticked(items) => json!({"type": "plan", "event": "ticked", "items": items}),
        PlanEvent::Finished => json!({"type": "plan", "event": "finished"}),
        PlanEvent::Abandoned => json!({"type": "plan", "event": "abandoned"}),
    }
}

impl Fields<'_> {
    fn f64(&self, key: &str) -> Result<f64, StoreError> {
        self.value(key)?
            .as_f64()
            .ok_or_else(|| corrupt(format!("{}.{key} is not a number", self.at)))
    }
    fn u32(&self, key: &str) -> Result<u32, StoreError> {
        u32::try_from(self.u64(key)?)
            .map_err(|_| corrupt(format!("{}.{key} is too large", self.at)))
    }
    fn optional<T>(
        &self,
        key: &str,
        read: impl Fn(&Self, &str) -> Result<T, StoreError>,
    ) -> Result<Option<T>, StoreError> {
        match self.object.get(key) {
            None => Ok(None),
            Some(_) => read(self, key).map(Some),
        }
    }
}

fn id<T>(
    text: &str,
    what: &str,
    make: fn(String) -> Result<T, vellum_types::IdError>,
) -> Result<T, StoreError> {
    make(text.to_owned()).map_err(|_| corrupt(format!("{what} is not an id")))
}

fn plan_item(value: &Value) -> Result<PlanItem, StoreError> {
    let f = Fields::new(
        value,
        "plan.items",
        &["id", "name", "page", "intent", "width", "height", "frame"],
    )?;
    Ok(PlanItem {
        id: f.u32("id")?,
        name: f.str("name")?.to_owned(),
        page: id(f.str("page")?, "plan item page", PageId::new)?,
        intent: f.str("intent")?.to_owned(),
        width: f.optional("width", Fields::f64)?,
        height: f.optional("height", Fields::f64)?,
        frame: f
            .optional("frame", Fields::str)?
            .map(|frame| id(frame, "plan item frame", NodeId::new))
            .transpose()?,
    })
}

fn plan_body(value: &Value) -> Result<Plan, StoreError> {
    let f = Fields::new(value, "plan", &["title", "items", "changes"])?;
    Ok(Plan {
        title: f.str("title")?.to_owned(),
        items: f
            .array("items")?
            .iter()
            .map(plan_item)
            .collect::<Result<_, _>>()?,
        changes: f.optional("changes", Fields::u32)?,
    })
}

fn placeholder(value: &Value) -> Result<Placeholder, StoreError> {
    let f = Fields::new(
        value,
        "placeholder",
        &["item", "frame", "name", "page", "bounds", "agent", "turn"],
    )?;
    let b = Fields::new(
        f.value("bounds")?,
        "placeholder.bounds",
        &["x", "y", "width", "height"],
    )?;
    Ok(Placeholder {
        item: f.u32("item")?,
        frame: id(f.str("frame")?, "placeholder frame", NodeId::new)?,
        name: f.str("name")?.to_owned(),
        page: id(f.str("page")?, "placeholder page", PageId::new)?,
        bounds: Rect::new(b.f64("x")?, b.f64("y")?, b.f64("width")?, b.f64("height")?),
        agent: id(f.str("agent")?, "placeholder agent", AgentId::new)?,
        turn: id(f.str("turn")?, "placeholder turn", TurnId::new)?,
    })
}

fn plan_event(value: &Value) -> Result<PlanEvent, StoreError> {
    let event = value
        .get("event")
        .and_then(Value::as_str)
        .ok_or_else(|| corrupt("a plan event has no kind"))?;
    Ok(match event {
        "proposed" => {
            let f = Fields::new(value, "plan event", &["type", "event", "plan"])?;
            PlanEvent::Proposed(plan_body(f.value("plan")?)?)
        }
        "built" => {
            let f = Fields::new(
                value,
                "plan event",
                &["type", "event", "plan", "placeholders"],
            )?;
            PlanEvent::Built {
                plan: plan_body(f.value("plan")?)?,
                placeholders: f
                    .array("placeholders")?
                    .iter()
                    .map(placeholder)
                    .collect::<Result<_, _>>()?,
            }
        }
        "ticked" => {
            let f = Fields::new(value, "plan event", &["type", "event", "items"])?;
            PlanEvent::Ticked(
                f.array("items")?
                    .iter()
                    .map(|item| {
                        item.as_u64()
                            .and_then(|n| u32::try_from(n).ok())
                            .ok_or_else(|| corrupt("plan ticks are item numbers"))
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
        "finished" | "abandoned" => {
            Fields::new(value, "plan event", &["type", "event"])?;
            if event == "finished" {
                PlanEvent::Finished
            } else {
                PlanEvent::Abandoned
            }
        }
        other => return Err(corrupt(format!("unknown plan event `{other}`"))),
    })
}
