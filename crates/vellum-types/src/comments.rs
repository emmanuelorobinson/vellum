//! Portable, attributed comment threads. Missing anchors preserve the conversation and last position.
use crate::{Actor, NodeId, Point};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CommentThread {
    pub author: Actor,
    pub anchor: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<crate::PageId>,
    pub point: Point,
    pub created_ms: i64,
    pub resolved: bool,
    pub resolved_by: Option<Actor>,
    pub messages: BTreeMap<NodeId, CommentMessage>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CommentMessage {
    pub author: Actor,
    pub at_ms: i64,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[ts(export)]
pub enum CommentRequest {
    Create {
        thread: NodeId,
        message: NodeId,
        anchor: Option<NodeId>,
        #[serde(default)]
        page: Option<crate::PageId>,
        point: Point,
        text: String,
    },
    Reply {
        thread: NodeId,
        message: NodeId,
        text: String,
    },
    Resolve {
        thread: NodeId,
        resolved: bool,
    },
}
