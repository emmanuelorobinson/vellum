//! `Value<T>`: a literal or a reference to a variable. Every bindable property in the file takes one
//! (ARCHITECTURE §4.1: "Every property can be a literal or a variable reference").

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::{IdError, VariableId};

/// A reference to a variable, written `{"$var": "<variable id>"}`.
///
/// It names the variable by its id, as every other reference in the file does (`componentId`, `asset`,
/// `textStyle`). A rename changes the variable's name and nothing else, and two branches that rename and
/// use a variable merge without leaving a dangling name. The name is for people and tools, and it lives in
/// the variable's own entry (see `Variable`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema, TS)]
#[ts(export)]
pub struct VarRef {
    #[serde(rename = "$var")]
    var: VariableId,
}

impl<'de> Deserialize<'de> for VarRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            #[serde(rename = "$var")]
            var: VariableId,
        }
        let Raw { var } = Raw::deserialize(deserializer)?;
        Ok(Self { var })
    }
}

impl VarRef {
    /// A reference to the variable with id `id`. Ids are checked the way every id is (guide §9).
    pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
        Ok(Self {
            var: VariableId::new(id)?,
        })
    }

    /// A reference to `variable`.
    pub fn to(variable: VariableId) -> Self {
        Self { var: variable }
    }

    /// The id of the variable this refers to.
    pub fn id(&self) -> &VariableId {
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

/// A plain value: `true`, a number or a string. Script inputs and instance props take one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Scalar {
    /// `true` or `false`.
    Bool(bool),
    /// A number.
    Number(f64),
    /// A string.
    Text(String),
}
