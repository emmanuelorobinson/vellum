//! Typed ids for everything that lives inside a `.vellum` file (guide §9, Identifiers).
//!
//! `IdGen` mints ten-character nanoid-style strings. The types accept anything in the nanoid alphabet
//! up to 128 characters (a content hash is a valid [`AssetId`]), and never a `/`, because paths use `/`
//! as their separator. Ids outside the file (sessions, turns, agents) are ULIDs and arrive with the
//! protocol types.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use ts_rs::TS;

use crate::defaults::serde_as_string;

/// The longest id any type here accepts, in bytes.
pub const MAX_ID_LEN: usize = 128;

/// Why a string is not a valid id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// The string was empty.
    Empty,
    /// The string was longer than [`MAX_ID_LEN`].
    TooLong(usize),
    /// The string held a character outside `A-Z a-z 0-9 _ -`.
    BadChar(char),
    /// A node id path had an empty segment (`a//b`, a leading or trailing `/`).
    EmptySegment,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("an id cannot be empty"),
            Self::TooLong(len) => write!(
                f,
                "an id is at most {MAX_ID_LEN} characters, this one has {len}"
            ),
            Self::BadChar('/') => f.write_str("an id cannot contain \"/\""),
            Self::BadChar(c) => write!(
                f,
                "an id uses only A-Z, a-z, 0-9, \"_\" and \"-\", not {c:?}"
            ),
            Self::EmptySegment => f.write_str("a node id path cannot have an empty segment"),
        }
    }
}

impl std::error::Error for IdError {}

fn check(text: &str) -> Result<(), IdError> {
    if text.is_empty() {
        return Err(IdError::Empty);
    }
    if text.len() > MAX_ID_LEN {
        return Err(IdError::TooLong(text.len()));
    }
    match text
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-')))
    {
        Some(bad) => Err(IdError::BadChar(bad)),
        None => Ok(()),
    }
}

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, JsonSchema, TS)]
        #[schemars(with = "String")]
        #[ts(export, type = "string")]
        pub struct $name(String);

        impl $name {
            /// Checks `id` and wraps it.
            pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
                let id = id.into();
                check(&id)?;
                Ok(Self(id))
            }

            /// The id as text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = IdError;
            fn from_str(id: &str) -> Result<Self, IdError> {
                Self::new(id)
            }
        }

        serde_as_string!($name);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

id_type!(
    /// A node's id, unique within its document and never reused.
    NodeId
);
id_type!(
    /// A page's id.
    PageId
);
id_type!(
    /// A variable's id.
    VariableId
);
id_type!(
    /// A component's id.
    ComponentId
);
id_type!(
    /// A text or effect style's id.
    StyleId
);
id_type!(
    /// An asset's id: the content hash of the file beside the document (`<name>.assets/`).
    AssetId
);

/// The address of a node inside an instance: node ids joined by `/` (`card/actions/button`). It always
/// has at least one segment.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, JsonSchema, TS)]
#[schemars(with = "String")]
#[ts(export, type = "string")]
pub struct NodeIdPath(Vec<NodeId>);

impl NodeIdPath {
    /// The number of segments, at least one.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always false: a path has at least one segment. Here for clippy's sake.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The ids from the outermost down.
    pub fn segments(&self) -> impl Iterator<Item = &NodeId> {
        self.0.iter()
    }

    /// The outermost id.
    pub fn first(&self) -> &NodeId {
        &self.0[0]
    }

    /// The innermost id: the node the path points at.
    pub fn last(&self) -> &NodeId {
        &self.0[self.0.len() - 1]
    }

    /// This path with `id` added below it.
    #[must_use]
    pub fn join(mut self, id: NodeId) -> Self {
        self.0.push(id);
        self
    }
}

impl From<NodeId> for NodeIdPath {
    fn from(id: NodeId) -> Self {
        Self(vec![id])
    }
}

impl FromStr for NodeIdPath {
    type Err = IdError;
    fn from_str(text: &str) -> Result<Self, IdError> {
        text.split('/')
            .map(|segment| {
                if segment.is_empty() {
                    Err(IdError::EmptySegment)
                } else {
                    NodeId::new(segment)
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }
}

serde_as_string!(NodeIdPath);

impl fmt::Display for NodeIdPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, id) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("/")?;
            }
            f.write_str(id.as_str())?;
        }
        Ok(())
    }
}
