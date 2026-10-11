use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::FORMAT_VERSION;

/// Placeholder document. Real pages, nodes and variables replace this in Prompts 7 and 8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Document {
    pub meta: DocumentMeta,
}

/// The `meta` block of a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DocumentMeta {
    pub format_version: u32,
}

impl Document {
    /// An empty document at the current format version.
    pub fn new() -> Self {
        Self {
            meta: DocumentMeta {
                format_version: FORMAT_VERSION,
            },
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
