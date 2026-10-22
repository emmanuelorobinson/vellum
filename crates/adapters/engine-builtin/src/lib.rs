//! AgentEngine: Vellum's own loop over any ModelProvider (ARCHITECTURE §3.4, §8).
//!
//! A turn is: the prompt pack's system prompt with the skills listed, the person's ask, then rounds of model
//! request → tool calls through the [`ToolBridge`] → results back to the model, until the model stops. Before it
//! is allowed to stop after writing, the engine asks it once to look at what it built (a screenshot, or a layout
//! check for a model that cannot see). Long results are cut with a marker, and the conversation is pruned and
//! then summarized when it nears the model's window (`context`).
//!
//! Stop (Invariant 12) cancels the turn's `Cancellation`. Every wait on the provider races it, so the turn ends
//! as soon as the stream is next polled, whether or not the provider honours cancellation, and no tool runs
//! after it.

mod context;
mod pack;

pub use pack::{PackError, PromptPack};

use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    future::{Future, poll_fn},
    pin::{Pin, pin},
    rc::Rc,
    sync::{Arc, Mutex},
    task::Poll,
};
use vellum_core::ports::*;
use vellum_types::TurnId;

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// Read-only tools whose success counts as checking the work.
const CHECKS: [&str; 2] = ["screenshot", "snapshot_layout"];

pub struct BuiltinEngine {
    provider: Arc<dyn ModelProvider>,
    pack: PromptPack,
    running: Arc<Mutex<BTreeMap<TurnId, Cancellation>>>,
}

impl BuiltinEngine {
    pub fn new(provider: Arc<dyn ModelProvider>, pack: PromptPack) -> Self {
        Self {
            provider,
            pack,
            running: Arc::default(),
        }
    }

    pub fn pack(&self) -> &PromptPack {
        &self.pack
    }
}

impl AgentEngine for BuiltinEngine {
    fn id(&self) -> &str {
        "builtin"
    }

    fn availability(&self) -> EngineStatus {
        if !self.provider.capabilities().tools || self.provider.status() == ProviderStatus::Failed {
            EngineStatus::Failed
        } else {
            EngineStatus::Ready
        }
    }

    fn run_turn<'a>(
        &'a self,
        turn: TurnRequest,
        tools: &'a dyn ToolBridge,
    ) -> Result<Box<dyn TurnStream + 'a>, EngineError> {
        if self.availability() != EngineStatus::Ready {
            return Err(EngineError::Unavailable);
        }
        if turn.model.trim().is_empty() || turn.limits.max_rounds == 0 {
            return Err(EngineError::InvalidRequest);
        }
        let cancel = Cancellation::default();
        {
            let mut running = self.running.lock().unwrap();
            if running.contains_key(&turn.turn) {
                return Err(EngineError::Busy);
            }
            running.insert(turn.turn.clone(), cancel.clone());
        }
        let queue = Rc::new(RefCell::new(VecDeque::new()));
        let id = turn.turn.clone();
        let work = Turn {
            engine: self,
            bridge: tools,
            sink: Sink(queue.clone()),
            cancel,
            request: turn,
        }
        .run();
        Ok(Box::new(TurnHandle {
            work: Some(Box::pin(work)),
            queue,
            running: self.running.clone(),
            turn: id,
        }))
    }

    fn interrupt(&self, turn: &TurnId) {
        if let Some(cancel) = self.running.lock().unwrap().get(turn) {
            cancel.cancel();
        }
    }
}

/// Events the turn has produced and the caller has not taken yet.
struct Sink(Rc<RefCell<VecDeque<TurnEvent>>>);
impl Sink {
    /// Queues an event and yields once, so the caller sees it before the turn does anything else.
    async fn emit(&self, event: TurnEvent) {
        self.0.borrow_mut().push_back(event);
        let mut yielded = false;
        poll_fn(|cx| {
            if yielded {
                Poll::Ready(())
            } else {
                yielded = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        })
        .await;
    }
}

struct TurnHandle<'a> {
    work: Option<Pin<Box<dyn Future<Output = ()> + 'a>>>,
    queue: Rc<RefCell<VecDeque<TurnEvent>>>,
    running: Arc<Mutex<BTreeMap<TurnId, Cancellation>>>,
    turn: TurnId,
}
impl TurnStream for TurnHandle<'_> {
    fn next(&mut self) -> EngineFuture<'_, Option<TurnEvent>> {
        Box::pin(poll_fn(|cx| {
            if let Some(event) = self.queue.borrow_mut().pop_front() {
                return Poll::Ready(Some(event));
            }
            let Some(work) = self.work.as_mut() else {
                return Poll::Ready(None);
            };
            let done = work.as_mut().poll(cx).is_ready();
            if done {
                self.work = None;
                self.running.lock().unwrap().remove(&self.turn);
            }
            match self.queue.borrow_mut().pop_front() {
                Some(event) => Poll::Ready(Some(event)),
                None if done => Poll::Ready(None),
                None => Poll::Pending,
            }
        }))
    }
}
impl Drop for TurnHandle<'_> {
    fn drop(&mut self) {
        self.running.lock().unwrap().remove(&self.turn);
    }
}

/// Waits for `future` unless the turn is stopped first; then the future is dropped, which cancels its request.
async fn unless_stopped<T>(cancel: &Cancellation, future: impl Future<Output = T>) -> Option<T> {
    let mut future = pin!(future);
    poll_fn(|cx| {
        if cancel.poll_cancelled(cx).is_ready() {
            return Poll::Ready(None);
        }
        future.as_mut().poll(cx).map(Some)
    })
    .await
}

struct Turn<'a> {
    engine: &'a BuiltinEngine,
    bridge: &'a dyn ToolBridge,
    sink: Sink,
    cancel: Cancellation,
    request: TurnRequest,
}

/// What one model request came back with.
struct Reply {
    parts: Vec<ChatPart>,
    calls: Vec<ModelCall>,
    reason: FinishReason,
}

/// How a step of the turn went, when it did not simply carry on.
enum Stop {
    End(TurnEnd),
}
impl From<EngineError> for Stop {
    fn from(error: EngineError) -> Self {
        Stop::End(TurnEnd::Failed(error))
    }
}

impl Turn<'_> {
    async fn run(self) {
        let mut log = vec![];
        let end = match self.work(&mut log).await {
            Ok(()) => TurnEnd::Done,
            Err(Stop::End(end)) => end,
        };
        let end = if self.cancel.is_cancelled() {
            TurnEnd::Stopped
        } else {
            end
        };
        self.sink.emit(TurnEvent::Messages(log)).await;
        self.sink.emit(TurnEvent::Ended(end)).await;
    }

    fn stopped(&self) -> Result<(), Stop> {
        if self.cancel.is_cancelled() {
            Err(Stop::End(TurnEnd::Stopped))
        } else {
            Ok(())
        }
    }

    async fn work(&self, log: &mut Vec<ChatMessage>) -> Result<(), Stop> {
        let caps = self.engine.provider.capabilities();
        let limits = self.request.limits;
        let offered = self.bridge.tools();
        let tools: Vec<ModelTool> = offered.iter().map(|t| t.tool.clone()).collect();
        let read_only = |name: &str| offered.iter().any(|t| t.tool.name == name && t.read_only);
        let can_check = offered
            .iter()
            .any(|t| CHECKS.contains(&t.tool.name.as_str()));
        let system = self.engine.pack.system(&self.request.skills);

        let mut ask = vec![ChatPart::Text(self.request.prompt.clone())];
        if !self.request.skills.is_empty() {
            ask.push(ChatPart::Text(pack::skill_catalog(&self.request.skills)));
        }
        ask.extend(
            self.request
                .images
                .iter()
                .map(|(mime, base64)| ChatPart::Image {
                    mime: mime.clone(),
                    base64: base64.clone(),
                }),
        );
        let ask = ChatMessage {
            role: ChatRole::User,
            content: ask,
        };
        let mut conversation = self.request.history.clone();
        conversation.push(ask.clone());
        log.push(ask);

        let (mut rounds, mut failures) = (0, 0);
        let (mut unchecked, mut asked_to_check) = (false, false);
        loop {
            self.stopped()?;
            if rounds >= limits.max_rounds {
                return Err(EngineError::RoundLimit.into());
            }
            self.fit(&system, &tools, &mut conversation, &mut rounds)
                .await?;
            self.stopped()?;
            if rounds >= limits.max_rounds {
                return Err(EngineError::RoundLimit.into());
            }
            rounds += 1;
            let request = ChatRequest {
                model: self.request.model.clone(),
                system: system.clone(),
                messages: conversation.clone(),
                tools: tools.clone(),
                reasoning: self.request.effort.filter(|_| caps.reasoning),
                max_output_tokens: limits.max_output_tokens,
            };
            let reply = self.ask(request, true).await?;
            if !reply.parts.is_empty() {
                let message = ChatMessage {
                    role: ChatRole::Assistant,
                    content: reply.parts,
                };
                conversation.push(message.clone());
                log.push(message);
            }
            match reply.reason {
                FinishReason::Refused => return Err(EngineError::Refused.into()),
                FinishReason::Length if reply.calls.is_empty() => {
                    return Err(EngineError::Model(ModelError::Truncated).into());
                }
                _ if reply.calls.is_empty() => {
                    if unchecked && can_check && !asked_to_check {
                        asked_to_check = true;
                        let nudge = ChatMessage {
                            role: ChatRole::User,
                            content: vec![ChatPart::Text(self.engine.pack.self_check().to_owned())],
                        };
                        conversation.push(nudge.clone());
                        log.push(nudge);
                        self.sink.emit(TurnEvent::SelfCheck).await;
                        continue;
                    }
                    return Ok(());
                }
                _ => {}
            }
            let mut results = vec![];
            let mut pictures = vec![];
            for call in &reply.calls {
                if self.cancel.is_cancelled() {
                    // Every call needs an answer, or the next turn's history is invalid.
                    results.push(ChatPart::ToolResult {
                        id: call.id.clone(),
                        name: call.name.clone(),
                        result: serde_json::json!({"error": {"kind": "stopped", "message": "The person stopped the turn before this call ran."}}),
                        failed: true,
                    });
                    continue;
                }
                self.sink
                    .emit(TurnEvent::ToolStarted {
                        call: call.id.clone(),
                        name: call.name.clone(),
                        arguments: call.arguments.clone(),
                    })
                    .await;
                let reply = self.bridge.call(&call.name, call.arguments.clone());
                let (value, picture) = take_picture(reply.value, caps.vision);
                let (value, truncated) = context::truncate(value, limits.max_tool_result_chars);
                if reply.failed {
                    failures += 1;
                } else if !read_only(&call.name) {
                    unchecked = true;
                } else if CHECKS.contains(&call.name.as_str()) {
                    unchecked = false;
                }
                if let Some((mime, base64)) = picture {
                    pictures.push(ChatPart::Text(format!(
                        "Picture from {} ({}):",
                        call.name, call.id
                    )));
                    pictures.push(ChatPart::Image { mime, base64 });
                }
                results.push(ChatPart::ToolResult {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    result: value,
                    failed: reply.failed,
                });
                self.sink
                    .emit(TurnEvent::ToolFinished {
                        call: call.id.clone(),
                        name: call.name.clone(),
                        failed: reply.failed,
                        truncated,
                    })
                    .await;
            }
            results.extend(pictures);
            let message = ChatMessage {
                role: ChatRole::User,
                content: results,
            };
            conversation.push(message.clone());
            log.push(message);
            self.stopped()?;
            if failures > limits.max_failed_calls {
                return Err(EngineError::TooManyFailedCalls.into());
            }
        }
    }

    /// One model request, streamed: text and reasoning go out as they arrive.
    async fn ask(&self, request: ChatRequest, show: bool) -> Result<Reply, Stop> {
        let provider = &self.engine.provider;
        let mut stream = match unless_stopped(
            &self.cancel,
            provider.complete(request, self.cancel.clone()),
        )
        .await
        {
            None | Some(Err(ModelError::Cancelled)) => return Err(Stop::End(TurnEnd::Stopped)),
            Some(Err(error)) => return Err(EngineError::Model(error).into()),
            Some(Ok(stream)) => stream,
        };
        let mut reply = Reply {
            parts: vec![],
            calls: vec![],
            reason: FinishReason::Stop,
        };
        loop {
            let event = match unless_stopped(&self.cancel, stream.next()).await {
                None | Some(Some(Err(ModelError::Cancelled))) => {
                    return Err(Stop::End(TurnEnd::Stopped));
                }
                Some(None) => return Err(EngineError::Model(ModelError::Truncated).into()),
                Some(Some(Err(error))) => return Err(EngineError::Model(error).into()),
                Some(Some(Ok(event))) => event,
            };
            match event {
                ModelEvent::Text(text) => {
                    if let Some(ChatPart::Text(last)) = reply.parts.last_mut() {
                        last.push_str(&text);
                    } else {
                        reply.parts.push(ChatPart::Text(text.clone()));
                    }
                    if show {
                        self.sink.emit(TurnEvent::Text(text)).await;
                    }
                }
                ModelEvent::Reasoning(text) => {
                    if show {
                        self.sink.emit(TurnEvent::Reasoning(text)).await;
                    }
                }
                ModelEvent::Continuation(value) => reply.parts.push(ChatPart::Continuation(value)),
                ModelEvent::ToolCall(call) => {
                    reply.parts.push(ChatPart::ToolCall(call.clone()));
                    reply.calls.push(call);
                }
                ModelEvent::Usage(usage) => self.sink.emit(TurnEvent::Usage(usage)).await,
                ModelEvent::Finished(reason) => {
                    reply.reason = reason;
                    return Ok(reply);
                }
            }
        }
    }

    /// Keeps the conversation inside the model's window: prune old results and pictures, then summarize older
    /// rounds. Fails only when even that leaves it too big.
    async fn fit(
        &self,
        system: &str,
        tools: &[ModelTool],
        conversation: &mut Vec<ChatMessage>,
        rounds: &mut u32,
    ) -> Result<(), Stop> {
        let limits = self.request.limits;
        let (budget, threshold) = context::budget(
            self.engine.provider.capabilities().context_tokens,
            limits.max_output_tokens,
        );
        let size = |c: &[ChatMessage]| context::estimate(system, tools, c);
        if size(conversation) <= threshold {
            return Ok(());
        }
        let Some(cut) = context::cut_point(conversation) else {
            return self.fits(size(conversation), budget);
        };
        let pruned = context::prune(conversation, cut);
        let mut summarized = 0;
        if size(conversation) > threshold && *rounds < limits.max_rounds {
            *rounds += 1;
            let request = ChatRequest {
                model: self.request.model.clone(),
                system: self.engine.pack.summarize().to_owned(),
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    content: vec![ChatPart::Text(context::transcript(&conversation[..cut]))],
                }],
                tools: vec![],
                reasoning: None,
                max_output_tokens: limits.max_output_tokens,
            };
            let reply = self.ask(request, false).await?;
            let summary: String = reply
                .parts
                .iter()
                .filter_map(|p| match p {
                    ChatPart::Text(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            if summary.trim().is_empty() || reply.reason != FinishReason::Stop {
                return Err(EngineError::ContextFull.into());
            }
            conversation.splice(
                ..cut,
                [context::summary_message(
                    &self.request.prompt,
                    summary.trim(),
                )],
            );
            summarized = cut;
        }
        if pruned + summarized > 0 {
            self.sink
                .emit(TurnEvent::Compacted { pruned, summarized })
                .await;
        }
        self.fits(size(conversation), budget)
    }

    fn fits(&self, size: u64, budget: u64) -> Result<(), Stop> {
        if size > budget {
            Err(EngineError::ContextFull.into())
        } else {
            Ok(())
        }
    }
}

/// Takes the picture out of a tool result (`image.base64`), leaving a marker in its place. A model that cannot
/// see gets the marker only.
fn take_picture(mut value: Value, vision: bool) -> (Value, Option<(String, String)>) {
    let Some(image) = value.get_mut("image").and_then(Value::as_object_mut) else {
        return (value, None);
    };
    let (Some(Value::String(mime)), Some(Value::String(base64))) =
        (image.get("mime").cloned(), image.get("base64").cloned())
    else {
        return (value, None);
    };
    let marker = if vision {
        "[attached below as a picture]"
    } else {
        "[not shown: this model cannot see pictures; use snapshot_layout with problems_only to check layout]"
    };
    image.insert("base64".into(), Value::String(marker.into()));
    (value, vision.then_some((mime, base64)))
}
