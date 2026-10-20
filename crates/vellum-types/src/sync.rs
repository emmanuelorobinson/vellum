//! Versioned sync messages. Credentials travel only in the transport's Authorization header.
use crate::Actor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SYNC_VERSION: u32 = 1;
pub const MAX_SYNC_COUNTER: u64 = 9_007_199_254_740_991;
pub const SYNC_ADAPTER: &str = "loro-1";
pub const MAX_UPDATE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, JsonSchema, TS)]
#[schemars(with = "String")]
#[ts(export, type = "string")]
pub struct RoomId(String);
impl Serialize for RoomId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for RoomId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
impl RoomId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Invalid room id".into());
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RoomId {
    type Error = String;
    fn try_from(v: String) -> Result<Self, String> {
        Self::new(v)
    }
}
impl From<RoomId> for String {
    fn from(v: RoomId) -> Self {
        v.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Role {
    Viewer,
    Commenter,
    Editor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Grant {
    pub actor: Actor,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ClientFrame {
    pub v: u32,
    pub adapter: String,
    pub epoch: u64,
    pub request: SyncRequest,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[ts(export)]
pub enum SyncRequest {
    Pull { after: u64 },
    Push { bytes: Vec<u8> },
    Comment { op: crate::comments::CommentRequest },
    Presence { state: PresenceState },
    Sharing { command: SharingCommand },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ServerFrame {
    pub v: u32,
    pub adapter: String,
    pub epoch: u64,
    pub revision: u64,
    pub body: SyncReply,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[ts(export)]
pub enum SyncReply {
    State {
        snapshot: Option<Vec<u8>>,
        updates: Vec<Vec<u8>>,
    },
    Ack,
    Collaboration {
        grant: Grant,
        participants: Vec<Participant>,
    },
    Sharing {
        state: SharingState,
    },
    Error {
        code: String,
    },
}

/// Persistence metadata contains no credentials and no vendor-native snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct RoomLog {
    pub epoch: u64,
    pub revision: u64,
    pub snapshot: String,
    pub updates: Vec<LoggedUpdate>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct LoggedUpdate {
    pub revision: u64,
    pub bytes: Vec<u8>,
}

pub fn client_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ClientFrame)).expect("schema serializes")
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PresenceState {
    pub name: String,
    pub page: Option<crate::PageId>,
    pub cursor: Option<crate::Point>,
    pub viewport: Option<PresenceViewport>,
    pub selection: Vec<crate::NodeId>,
    pub activity: String,
    pub hue: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PresenceViewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}
impl PresenceState {
    pub fn valid(&self) -> bool {
        !self.name.trim().is_empty()
            && self.name.len() <= 128
            && self.activity.len() <= 256
            && !self.name.contains('\0')
            && !self.activity.contains('\0')
            && self.selection.len() <= 128
            && self
                .cursor
                .as_ref()
                .is_none_or(|p| p.x.is_finite() && p.y.is_finite())
            && self.viewport.as_ref().is_none_or(|p| {
                p.x.is_finite()
                    && p.y.is_finite()
                    && p.zoom.is_finite()
                    && p.zoom > 0.0
                    && p.zoom <= 256.0
            })
            && self.hue.as_ref().is_none_or(|h| {
                matches!(
                    h.as_str(),
                    "teal" | "saffron" | "green" | "orange" | "rose" | "magenta" | "lime" | "sand"
                )
            })
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct Participant {
    pub connection: String,
    pub actor: Actor,
    pub expires_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<PresenceState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[ts(export)]
pub enum SharingCommand {
    Read,
    SetLive { enabled: bool },
    SetRole { actor: Actor, role: Role },
    Remove { actor: Actor },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SharingState {
    pub owner_actor: Actor,
    pub owner: bool,
    pub enabled: bool,
    pub can_live: bool,
    pub members: Vec<Grant>,
}
pub fn server_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ServerFrame)).expect("schema serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_ids_refuse_paths_and_protocol_is_strict() {
        for value in ["", "../room", "/root", "room%2fother", "room space"] {
            assert!(RoomId::new(value).is_err());
        }
        let room = RoomId::new("test-room_1").unwrap();
        assert_eq!(
            serde_json::from_str::<RoomId>(&serde_json::to_string(&room).unwrap()).unwrap(),
            room
        );
        assert!(serde_json::from_str::<ClientFrame>(r#"{"v":1,"adapter":"loro-1","epoch":0,"request":{"type":"pull","after":0},"token":"TEST DATA"}"#).is_err());
        assert!(client_schema()["$defs"]["SyncRequest"].is_object());
    }
}
