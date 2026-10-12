//! `Value<T>`: a literal or a reference to a variable. Every bindable property in the file takes one
//! (ARCHITECTURE §4.1: "Every property can be a literal or a variable reference").

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A reference to a variable, written `{"$var": "color/primary"}`.
///
/// The key is opaque text for now: Prompt 8 defines variables and decides whether it names a
/// variable by id or by its name path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema, TS)]
#[ts(export)]
pub struct VarRef {
    #[serde(rename = "$var")]
    #[schemars(length(min = 1))]
    var: String,
}

impl<'de> Deserialize<'de> for VarRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            #[serde(rename = "$var")]
            var: String,
        }
        let Raw { var } = Raw::deserialize(deserializer)?;
        Self::new(var).map_err(serde::de::Error::custom)
    }
}

/// Why a variable reference was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyVarRef;

impl fmt::Display for EmptyVarRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a variable reference cannot be empty")
    }
}

impl std::error::Error for EmptyVarRef {}

impl VarRef {
    /// A reference to the variable `var`.
    pub fn new(var: impl Into<String>) -> Result<Self, EmptyVarRef> {
        let var = var.into();
        if var.is_empty() {
            Err(EmptyVarRef)
        } else {
            Ok(Self { var })
        }
    }

    /// The variable's key.
    pub fn as_str(&self) -> &str {
        &self.var
    }
}

/// A literal `T`, or a reference to a variable of the matching type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Value<T> {
    /// A reference. Listed first so `{"$var": …}` is never mistaken for a structured literal.
    Var(VarRef),
    /// The value itself.
    Literal(T),
}

impl<T> Value<T> {
    /// The literal, if this is not a reference.
    pub fn literal(&self) -> Option<&T> {
        match self {
            Self::Literal(value) => Some(value),
            Self::Var(_) => None,
        }
    }
}

impl<T> From<T> for Value<T> {
    fn from(value: T) -> Self {
        Self::Literal(value)
    }
}

impl<T: Default> Default for Value<T> {
    fn default() -> Self {
        Self::Literal(T::default())
    }
}
