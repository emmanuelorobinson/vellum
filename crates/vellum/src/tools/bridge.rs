//! The registry as an agent engine sees it (ARCHITECTURE §3.4: the built-in agent is one more front end). Each
//! call is an ordinary tool run in the bridge's session, so it is recorded as a step, attributed to the session's
//! agent and turn, and passes the Guard like a call from any MCP client.
use std::cell::RefCell;

use serde_json::Value;
use vellum_core::ports::{BridgeTool, ModelTool, ToolBridge, ToolReply};

use super::{Registry, Session, ToolCtx};
use crate::daemon::Documents;

pub struct RegistryBridge<'a> {
    registry: &'a Registry,
    state: RefCell<(&'a mut Documents, &'a mut Session)>,
}

impl<'a> RegistryBridge<'a> {
    /// Calls go to `docs` as `session`. Set the session's turn first so its changes can be undone as one.
    pub fn new(registry: &'a Registry, docs: &'a mut Documents, session: &'a mut Session) -> Self {
        Self {
            registry,
            state: RefCell::new((docs, session)),
        }
    }
}

impl ToolBridge for RegistryBridge<'_> {
    fn tools(&self) -> Vec<BridgeTool> {
        self.registry
            .tools()
            .map(|tool| BridgeTool {
                tool: ModelTool {
                    name: tool.name().to_owned(),
                    description: tool.description().to_owned(),
                    parameters: serde_json::to_value(tool.input_schema())
                        .expect("a schema is JSON"),
                },
                read_only: tool.read_only(),
            })
            .collect()
    }

    fn call(&self, name: &str, input: Value) -> ToolReply {
        let mut state = self.state.borrow_mut();
        let (docs, session) = &mut *state;
        let mut ctx = ToolCtx::new(docs, session);
        match self.registry.call(&mut ctx, name, input) {
            Ok(value) => ToolReply {
                value,
                failed: false,
            },
            Err(error) => ToolReply {
                value: error.to_json(),
                failed: true,
            },
        }
    }
}
