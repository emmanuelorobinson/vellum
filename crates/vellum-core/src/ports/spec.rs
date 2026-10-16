//! `SpecFormat` (ARCHITECTURE §3.5): what `get_spec` tells an agent about a frame so it can write the code. The
//! format is framework-neutral; hints for one stack (shadcn, React Native, SwiftUI) are a separate
//! [`StackProfile`](crate::spec::StackProfile) the caller puts beside it.
use std::fmt;

use serde_json::Value;
use vellum_types::{Document, NodeId};

/// Why a spec could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError {
    /// What was wrong, in a sentence.
    pub message: String,
}

impl SpecError {
    /// An error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SpecError {}

/// A way to describe a frame to a coding agent.
pub trait SpecFormat {
    /// The name tools use: `layout-spec-v1`. It is also the `format` field of what `build` returns.
    fn id(&self) -> &'static str;
    /// Describes the layer `frame` and everything in it. Every token the description refers to is listed in it,
    /// by variable name; the same document and layer always give the same description.
    fn build(&self, doc: &Document, frame: &NodeId) -> Result<Value, SpecError>;
}
