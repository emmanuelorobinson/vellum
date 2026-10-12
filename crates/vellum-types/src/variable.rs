//! Variables (ARCHITECTURE §4.1): a type, a name and ordered values. A value is a literal of the variable's
//! type, or an alias to another variable of the same type. It can apply only under some theme axis values.
//! The last entry whose condition matches wins; choosing it is resolution (Prompt 16), not the file's job.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::color::Color;
use crate::enums::VariableType;
use crate::ids::VariableId;

/// A variable. The `type` tag says which kind; each kind reads its literals as that type, so a string
/// variable keeps `#ff0000` as the text it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export)]
pub enum Variable {
    /// A colour: `#RRGGBB` or `#RRGGBBAA` literals.
    Color(VariableBody<Color>),
    /// A number.
    Number(VariableBody<f64>),
    /// A string, kept exactly as written.
    String(VariableBody<String>),
    /// `true` or `false`.
    Boolean(VariableBody<bool>),
}

/// The name and values of a variable of one type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct VariableBody<T> {
    /// The name, by convention a path such as `color/primary`. People and tools use names; references use
    /// the variable's id.
    pub name: String,
    /// The values in order. The last entry whose condition matches is the one that applies.
    pub values: Vec<VariableValue<T>>,
}

/// One value of a variable: a literal, or an alias to another variable of the same type. It applies when
/// every axis in `when` has the value it names; with no `when`, it always applies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum VariableValue<T> {
    /// A value of the variable's own type.
    Literal(LiteralEntry<T>),
    /// The value of another variable of the same type.
    Alias(AliasEntry),
}

/// A literal value and the theme axis values it applies under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LiteralEntry<T> {
    /// The value.
    pub value: T,
    /// Theme axis values this entry needs (`{"Mode": "dark"}`); empty means always.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub when: BTreeMap<String, String>,
}

/// An alias to another variable's value and the theme axis values it applies under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AliasEntry {
    /// The variable whose value this takes, by id.
    #[serde(rename = "$alias")]
    pub alias: VariableId,
    /// Theme axis values this entry needs; empty means always.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub when: BTreeMap<String, String>,
}

impl Variable {
    /// The variable's type.
    pub fn variable_type(&self) -> VariableType {
        match self {
            Self::Color(_) => VariableType::Color,
            Self::Number(_) => VariableType::Number,
            Self::String(_) => VariableType::String,
            Self::Boolean(_) => VariableType::Boolean,
        }
    }

    /// The variables this one aliases, in entry order (a variable may alias the same one twice).
    pub fn aliases(&self) -> Vec<&VariableId> {
        fn of<T>(body: &VariableBody<T>) -> Vec<&VariableId> {
            body.values
                .iter()
                .filter_map(|entry| match entry {
                    VariableValue::Alias(alias) => Some(&alias.alias),
                    VariableValue::Literal(_) => None,
                })
                .collect()
        }
        match self {
            Self::Color(body) => of(body),
            Self::Number(body) => of(body),
            Self::String(body) => of(body),
            Self::Boolean(body) => of(body),
        }
    }

    /// The variable's name.
    pub fn name(&self) -> &str {
        match self {
            Self::Color(body) => &body.name,
            Self::Number(body) => &body.name,
            Self::String(body) => &body.name,
            Self::Boolean(body) => &body.name,
        }
    }
}
