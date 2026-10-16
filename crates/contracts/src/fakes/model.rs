use std::{
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
};
use vellum_core::ports::*;

#[derive(Default)]
pub struct MemorySecrets(Mutex<BTreeMap<String, SecretString>>);
impl SecretStore for MemorySecrets {
    fn get(&self, key: &str) -> Result<Option<SecretString>, SecretError> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn set(&self, key: &str, value: SecretString) -> Result<(), SecretError> {
        self.0.lock().unwrap().insert(key.into(), value);
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}
type Script = Vec<Result<ModelEvent, ModelError>>;
/// In-memory wire boundary. Captures only request bodies; credentials are discarded.
pub struct ReplayTransport {
    scripts: Mutex<VecDeque<Vec<Result<WireEvent, ModelError>>>>,
    requests: Mutex<Vec<serde_json::Value>>,
}
impl ReplayTransport {
    pub fn new(scripts: Vec<Vec<Result<WireEvent, ModelError>>>) -> Self {
        Self {
            scripts: Mutex::new(scripts.into()),
            requests: Mutex::new(vec![]),
        }
    }
    pub fn requests(&self) -> Vec<serde_json::Value> {
        self.requests.lock().unwrap().clone()
    }
}
struct ReplayStream(VecDeque<Result<WireEvent, ModelError>>);
impl ModelWireStream for ReplayStream {
    fn next(&mut self) -> ModelFuture<'_, Option<Result<WireEvent, ModelError>>> {
        Box::pin(async move { self.0.pop_front() })
    }
}
impl ModelTransport for ReplayTransport {
    fn start(
        &self,
        request: ModelHttpRequest,
        cancel: Cancellation,
    ) -> ModelFuture<'_, Result<WireStream, ModelError>> {
        Box::pin(async move {
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            self.requests.lock().unwrap().push(request.body);
            let events = self
                .scripts
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(ModelError::Exhausted)?;
            Ok(Box::new(ReplayStream(events.into())) as WireStream)
        })
    }
}
pub struct ScriptedModel {
    scripts: Mutex<VecDeque<Script>>,
    state: ProviderState,
}
impl ScriptedModel {
    pub fn new(scripts: Vec<Script>) -> Self {
        Self {
            scripts: Mutex::new(scripts.into()),
            state: ProviderState::default(),
        }
    }
}
struct ScriptStream {
    events: VecDeque<Result<ModelEvent, ModelError>>,
    cancel: Cancellation,
    done: bool,
}
impl ModelStream for ScriptStream {
    fn next(&mut self) -> ModelFuture<'_, Option<Result<ModelEvent, ModelError>>> {
        Box::pin(async move {
            if self.done {
                return None;
            }
            if self.cancel.is_cancelled() {
                self.done = true;
                return Some(Err(ModelError::Cancelled));
            }
            let event = self
                .events
                .pop_front()
                .unwrap_or(Err(ModelError::Truncated));
            self.done = matches!(event, Ok(ModelEvent::Finished(_)) | Err(_));
            Some(event)
        })
    }
}
impl ModelProvider for ScriptedModel {
    fn id(&self) -> &str {
        "scripted"
    }
    fn capabilities(&self) -> ModelCaps {
        ModelCaps {
            tools: true,
            vision: true,
            reasoning: true,
            context_tokens: None,
        }
    }
    fn status(&self) -> ProviderStatus {
        self.state.status(false)
    }
    fn complete(
        &self,
        req: ChatRequest,
        cancel: Cancellation,
    ) -> ModelFuture<'_, Result<ChatStream, ModelError>> {
        Box::pin(async move {
            req.validate(self.capabilities())?;
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            let events = self
                .scripts
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(ModelError::Exhausted)?;
            Ok(Box::new(ScriptStream {
                events: events.into(),
                cancel,
                done: false,
            }) as ChatStream)
        })
    }
}
