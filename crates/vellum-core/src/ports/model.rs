//! Provider-neutral conversation and secret boundaries. No credentials are serializable.
use serde_json::Value;
use std::{
    collections::VecDeque,
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    task::{Context, Poll, Waker},
};

pub type ModelFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone)]
pub struct SecretString(String);
impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    /// Only credential stores and authenticated transports should expose this.
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([redacted])")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretError {
    Unavailable,
    Denied,
    InvalidKey,
    ReadOnly,
}
impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "credential store: {self:?}")
    }
}
impl std::error::Error for SecretError {}
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError>;
    fn set(&self, key: &str, value: SecretString) -> Result<(), SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelCaps {
    pub tools: bool,
    pub vision: bool,
    pub reasoning: bool,
    /// None means unknown, never unlimited.
    pub context_tokens: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderStatus {
    Connected,
    ApiKeySet,
    SignedIn,
    ConfiguredExternally,
    Failed,
}
#[derive(Default)]
pub struct ProviderState(AtomicU8);
impl ProviderState {
    pub fn status(&self, has_key: bool) -> ProviderStatus {
        match self.0.load(Ordering::Relaxed) {
            1 => ProviderStatus::Connected,
            2 => ProviderStatus::Failed,
            3 => ProviderStatus::SignedIn,
            _ if has_key => ProviderStatus::ApiKeySet,
            _ => ProviderStatus::ConfiguredExternally,
        }
    }
    pub fn connected(&self) {
        self.0.store(1, Ordering::Relaxed);
    }
    pub fn failed(&self) {
        self.0.store(2, Ordering::Relaxed);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}
impl ReasoningEffort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ModelCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    /// Opaque provider continuation data (e.g. Gemini thoughtSignature).
    pub metadata: Value,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ChatPart {
    Text(String),
    Image {
        mime: String,
        base64: String,
    },
    ToolCall(ModelCall),
    ToolResult {
        id: String,
        name: String,
        result: Value,
        failed: bool,
    },
    /// Provider-specific reasoning blocks that must survive a tool round trip.
    Continuation(Value),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: Vec<ChatPart>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ModelTool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub system: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ModelTool>,
    pub reasoning: Option<ReasoningEffort>,
    pub max_output_tokens: u32,
}
impl ChatRequest {
    pub fn text(model: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            system: String::new(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: vec![ChatPart::Text(text.into())],
            }],
            tools: vec![],
            reasoning: None,
            max_output_tokens: 4096,
        }
    }
    pub fn validate(&self, caps: ModelCaps) -> Result<(), ModelError> {
        if self.model.trim().is_empty() || self.messages.is_empty() || self.max_output_tokens == 0 {
            return Err(ModelError::InvalidRequest);
        }
        if (!caps.tools
            && (!self.tools.is_empty()
                || self
                    .messages
                    .iter()
                    .flat_map(|m| &m.content)
                    .any(|p| matches!(p, ChatPart::ToolCall(_) | ChatPart::ToolResult { .. }))))
            || (!caps.vision
                && self
                    .messages
                    .iter()
                    .flat_map(|m| &m.content)
                    .any(|p| matches!(p, ChatPart::Image { .. })))
            || (!caps.reasoning && self.reasoning.is_some())
        {
            return Err(ModelError::Unsupported);
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct ModelConfig {
    pub id: String,
    /// Full configured endpoint; never an URL with embedded credentials.
    pub endpoint: String,
    pub secret_key: Option<String>,
    /// Capabilities are supplied for the selected model, not guessed from its provider.
    pub caps: ModelCaps,
}
impl ModelConfig {
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.id.is_empty() || self.endpoint.is_empty() {
            return Err(ModelError::InvalidRequest);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    Refused,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ModelEvent {
    Text(String),
    Reasoning(String),
    Continuation(Value),
    ToolCall(ModelCall),
    Usage(ModelUsage),
    Finished(FinishReason),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelError {
    Authentication,
    RateLimited,
    Unavailable,
    Network,
    Timeout,
    Protocol,
    Truncated,
    Unsupported,
    InvalidRequest,
    Cancelled,
    Exhausted,
    Limit,
}
impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "model request: {self:?}")
    }
}
impl std::error::Error for ModelError {}

#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}
#[derive(Clone, Default)]
pub struct Cancellation(Arc<CancelState>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::SeqCst);
        let wakers = std::mem::take(&mut *self.0.wakers.lock().unwrap());
        for waker in wakers {
            waker.wake();
        }
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }
    pub fn poll_cancelled(&self, cx: &mut Context<'_>) -> Poll<()> {
        let mut wakers = self.0.wakers.lock().unwrap();
        if self.is_cancelled() {
            Poll::Ready(())
        } else {
            if !wakers.iter().any(|waker| waker.will_wake(cx.waker())) {
                wakers.push(cx.waker().clone());
            }
            Poll::Pending
        }
    }
}
pub trait ModelStream: Send {
    fn next(&mut self) -> ModelFuture<'_, Option<Result<ModelEvent, ModelError>>>;
}
pub type ChatStream = Box<dyn ModelStream>;
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ModelCaps;
    fn status(&self) -> ProviderStatus;
    fn complete(
        &self,
        req: ChatRequest,
        cancel: Cancellation,
    ) -> ModelFuture<'_, Result<ChatStream, ModelError>>;
}

/// HTTP is supplied by the composition root; protocol adapters never import another adapter.
pub struct ModelHttpRequest {
    pub endpoint: String,
    pub headers: Vec<(String, String)>,
    pub credential: Option<(String, SecretString)>,
    pub body: Value,
}
impl fmt::Debug for ModelHttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ModelHttpRequest([redacted])")
    }
}
#[derive(Debug)]
pub enum WireEvent {
    Data(Value),
    Done,
}
pub trait ModelWireStream: Send {
    fn next(&mut self) -> ModelFuture<'_, Option<Result<WireEvent, ModelError>>>;
}
pub type WireStream = Box<dyn ModelWireStream>;
pub struct ModelResponse {
    wire: WireStream,
    secret: Option<SecretString>,
}
pub trait ModelTransport: Send + Sync {
    fn start(
        &self,
        request: ModelHttpRequest,
        cancel: Cancellation,
    ) -> ModelFuture<'_, Result<WireStream, ModelError>>;
}
pub struct ProviderConnection {
    pub config: ModelConfig,
    pub state: Arc<ProviderState>,
    secrets: Arc<dyn SecretStore>,
    transport: Arc<dyn ModelTransport>,
}
impl ProviderConnection {
    pub fn new(
        config: ModelConfig,
        secrets: Arc<dyn SecretStore>,
        transport: Arc<dyn ModelTransport>,
    ) -> Result<Self, ModelError> {
        config.validate()?;
        Ok(Self {
            config,
            secrets,
            transport,
            state: Arc::default(),
        })
    }
    pub fn status(&self) -> ProviderStatus {
        let has_key = self
            .config
            .secret_key
            .as_ref()
            .map(|key| self.secrets.get(key));
        match has_key {
            Some(Err(_)) => ProviderStatus::Failed,
            other => self.state.status(matches!(other, Some(Ok(Some(_))))),
        }
    }
    pub async fn open(
        &self,
        req: &ChatRequest,
        body: Value,
        auth_header: &str,
        bearer: bool,
        headers: Vec<(String, String)>,
        cancel: Cancellation,
    ) -> Result<ModelResponse, ModelError> {
        let result = async {
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            req.validate(self.config.caps)?;
            let credential = match &self.config.secret_key {
                Some(key) => {
                    let secret = self
                        .secrets
                        .get(key)
                        .map_err(|_| ModelError::Authentication)?
                        .filter(|key| !key.expose().is_empty())
                        .ok_or(ModelError::Authentication)?;
                    let value = if bearer {
                        SecretString::new(format!("Bearer {}", secret.expose()))
                    } else {
                        secret
                    };
                    Some((auth_header.to_owned(), value))
                }
                None => None,
            };
            let secret = credential.as_ref().map(|(_, value)| {
                SecretString::new(
                    value
                        .expose()
                        .strip_prefix("Bearer ")
                        .unwrap_or(value.expose()),
                )
            });
            let wire = self
                .transport
                .start(
                    ModelHttpRequest {
                        endpoint: self.config.endpoint.clone(),
                        headers,
                        credential,
                        body,
                    },
                    cancel,
                )
                .await?;
            Ok(ModelResponse { wire, secret })
        }
        .await;
        if result.as_ref().is_err_and(|error| {
            !matches!(
                error,
                ModelError::Cancelled | ModelError::Unsupported | ModelError::InvalidRequest
            )
        }) {
            self.state.failed();
        }
        result
    }
}
pub trait ModelDecoder: Send {
    fn decode(&mut self, event: WireEvent) -> Result<Vec<ModelEvent>, ModelError>;
}
#[derive(Default)]
pub struct CallAccumulator {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub metadata: Value,
}
impl CallAccumulator {
    pub fn append(&mut self, fragment: &str) -> Result<(), ModelError> {
        if self.arguments.len() + fragment.len() > 1024 * 1024 {
            return Err(ModelError::Limit);
        }
        self.arguments.push_str(fragment);
        Ok(())
    }
    pub fn finish(self) -> Result<ModelCall, ModelError> {
        if self.id.is_empty() || self.name.is_empty() {
            return Err(ModelError::Protocol);
        }
        let arguments: Value = serde_json::from_str(if self.arguments.is_empty() {
            "{}"
        } else {
            &self.arguments
        })
        .map_err(|_| ModelError::Protocol)?;
        if !arguments.is_object() {
            return Err(ModelError::Protocol);
        }
        Ok(ModelCall {
            id: self.id,
            name: self.name,
            arguments,
            metadata: self.metadata,
        })
    }
}
struct DecodedStream<D> {
    wire: Option<WireStream>,
    cancel: Cancellation,
    decoder: D,
    queue: VecDeque<ModelEvent>,
    done: bool,
    state: Arc<ProviderState>,
    redactor: Option<OutputRedactor>,
}
pub fn decoded_stream<D: ModelDecoder + 'static>(
    response: ModelResponse,
    decoder: D,
    state: Arc<ProviderState>,
    cancel: Cancellation,
) -> ChatStream {
    Box::new(DecodedStream {
        wire: Some(response.wire),
        cancel,
        decoder,
        queue: VecDeque::new(),
        done: false,
        state,
        redactor: response.secret.map(OutputRedactor::new),
    })
}
impl<D: ModelDecoder> ModelStream for DecodedStream<D> {
    fn next(&mut self) -> ModelFuture<'_, Option<Result<ModelEvent, ModelError>>> {
        Box::pin(async move {
            loop {
                if self.cancel.is_cancelled() && (!self.done || !self.queue.is_empty()) {
                    self.done = true;
                    self.queue.clear();
                    self.wire = None;
                    return Some(Err(ModelError::Cancelled));
                }
                if let Some(event) = self.queue.pop_front() {
                    return Some(Ok(event));
                }
                if self.done {
                    return None;
                }
                let result = match self.wire.as_mut().expect("active wire").next().await {
                    Some(Ok(event)) => self.decoder.decode(event),
                    Some(Err(error)) => Err(error),
                    None => Err(ModelError::Truncated),
                };
                match result {
                    Ok(events) => {
                        self.done = events.iter().any(|e| matches!(e, ModelEvent::Finished(_)));
                        if self.done {
                            self.state.connected();
                            self.wire = None;
                        }
                        let events = if let Some(redactor) = &mut self.redactor {
                            redactor.events(events)
                        } else {
                            events
                        };
                        self.queue.extend(events);
                    }
                    Err(error) => {
                        self.done = true;
                        if error != ModelError::Cancelled {
                            self.state.failed();
                        }
                        self.wire = None;
                        return Some(Err(error));
                    }
                }
            }
        })
    }
}

/// Retain a possible credential prefix across text deltas. Full tool JSON and opaque blocks
/// are scrubbed too. Errors discard held suffixes instead of leaking a partial credential.
struct OutputRedactor {
    secret: SecretString,
    text: String,
    reasoning: String,
}
impl OutputRedactor {
    fn new(secret: SecretString) -> Self {
        Self {
            secret,
            text: String::new(),
            reasoning: String::new(),
        }
    }
    fn delta(secret: &str, pending: &mut String, text: &str) -> String {
        pending.push_str(text);
        *pending = pending.replace(secret, "[redacted]");
        let keep = (1..secret.len().min(pending.len() + 1))
            .rev()
            .find(|count| {
                let split = pending.len() - count;
                pending.is_char_boundary(split) && secret.starts_with(&pending[split..])
            })
            .unwrap_or(0);
        let split = pending.len() - keep;
        pending.drain(..split).collect()
    }
    fn json(&self, value: &mut Value) {
        match value {
            Value::String(text) => *text = text.replace(self.secret.expose(), "[redacted]"),
            Value::Array(items) => items.iter_mut().for_each(|v| self.json(v)),
            Value::Object(items) => {
                *items = std::mem::take(items)
                    .into_iter()
                    .map(|(key, mut value)| {
                        self.json(&mut value);
                        (key.replace(self.secret.expose(), "[redacted]"), value)
                    })
                    .collect();
            }
            _ => {}
        }
    }
    fn events(&mut self, events: Vec<ModelEvent>) -> Vec<ModelEvent> {
        let mut safe = vec![];
        for event in events {
            match event {
                ModelEvent::Text(text) => {
                    let text = Self::delta(self.secret.expose(), &mut self.text, &text);
                    if !text.is_empty() {
                        safe.push(ModelEvent::Text(text));
                    }
                }
                ModelEvent::Reasoning(text) => {
                    let text = Self::delta(self.secret.expose(), &mut self.reasoning, &text);
                    if !text.is_empty() {
                        safe.push(ModelEvent::Reasoning(text));
                    }
                }
                ModelEvent::ToolCall(mut call) => {
                    call.id = call.id.replace(self.secret.expose(), "[redacted]");
                    call.name = call.name.replace(self.secret.expose(), "[redacted]");
                    self.json(&mut call.arguments);
                    self.json(&mut call.metadata);
                    safe.push(ModelEvent::ToolCall(call));
                }
                ModelEvent::Continuation(mut value) => {
                    self.json(&mut value);
                    safe.push(ModelEvent::Continuation(value));
                }
                ModelEvent::Finished(reason) => {
                    if !self.text.is_empty() {
                        safe.push(ModelEvent::Text(std::mem::take(&mut self.text)));
                    }
                    if !self.reasoning.is_empty() {
                        safe.push(ModelEvent::Reasoning(std::mem::take(&mut self.reasoning)));
                    }
                    safe.push(ModelEvent::Finished(reason));
                }
                event => safe.push(event),
            }
        }
        safe
    }
}
