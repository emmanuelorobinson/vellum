//! Local transport handshake. Credentials never enter document or session types.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const DAEMON_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum DaemonChannel {
    Mcp,
    Control,
}

// Deliberately no Debug: even malformed credentials must not reach diagnostics.
#[derive(Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DaemonHello {
    pub v: u32,
    pub token: String,
    pub channel: DaemonChannel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "status", rename_all = "kebab-case")]
#[ts(export)]
pub enum DaemonWelcome {
    Accepted { v: u32 },
    Unauthorized,
    Version { supported: Vec<u32> },
}
