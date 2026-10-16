//! Design tokens as a document's variables written to, and read from, a file format (ARCHITECTURE §3.5).
//!
//! A [`TokenSet`] is the neutral form every [`TokenFormat`](crate::ports::TokenFormat) reads and writes: each token
//! has a name, a type, the value it has by default and the values it has under one non-default theme value. That
//! is the shape CSS custom properties (`:root`, `.dark`), Tailwind's `@theme` and DTCG files can all hold. A
//! variable whose value depends on several axes at once (`Mode: Dark` and `Device: Phone` together) has no such
//! spelling; [`collect`] writes the values per axis and says so in a warning rather than dropping it silently.
//!
//! - [`collect`] reads a document's variables into a set, resolving which entry applies under each theme.
//! - [`ThemeMapping`] says which selector each theme value is written under, and is editable by the person.
//! - [`plan`] compares an imported set with a document and returns the diff (added, changed, removed) and the
//!   ops that make the document match it. Applying them is the caller's one transaction.
//!
//! Everything here is pure: formats are adapters, files are the caller's.

mod collect;
mod mapping;
mod plan;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;

use vellum_types::{Color, VariableType};

pub use collect::collect;
pub use mapping::{MappingError, ModeSelector, ThemeMapping};
pub use plan::{Change, ImportPlan, PlanError, TokenDiff, plan};

/// A token's value: a literal of its type, or the name of another token of the same type.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenValue {
    /// A colour.
    Color(Color),
    /// A number.
    Number(f64),
    /// A string, exactly as written.
    String(String),
    /// `true` or `false`.
    Boolean(bool),
    /// The value of the token with this name.
    Alias(String),
}

impl TokenValue {
    /// The type of a literal; `None` for an alias, which takes its target's.
    pub fn kind(&self) -> Option<VariableType> {
        match self {
            Self::Color(_) => Some(VariableType::Color),
            Self::Number(_) => Some(VariableType::Number),
            Self::String(_) => Some(VariableType::String),
            Self::Boolean(_) => Some(VariableType::Boolean),
            Self::Alias(_) => None,
        }
    }
}

/// The value a token has when one theme axis takes one non-default value.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeValue {
    /// The theme axis, such as `Mode`.
    pub axis: String,
    /// Its value, such as `Dark`.
    pub mode: String,
    /// What the token is then.
    pub value: TokenValue,
}

/// One token.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The variable's name, a path such as `color/primary`.
    pub name: String,
    /// Its type.
    pub kind: VariableType,
    /// The value under the default theme.
    pub value: TokenValue,
    /// The values that differ from `value` under one non-default theme value, ordered by axis, then by the
    /// axis's own value order.
    pub modes: Vec<ModeValue>,
}

/// Tokens and the theme axes they vary on.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TokenSet {
    /// Theme axes and their values, the first value of each being the default.
    pub themes: BTreeMap<String, Vec<String>>,
    /// The tokens, ordered by name.
    pub tokens: Vec<Token>,
}

impl TokenSet {
    /// The token called `name`.
    pub fn get(&self, name: &str) -> Option<&Token> {
        self.tokens.iter().find(|token| token.name == name)
    }
}

/// Something a format or a comparison wants the person to know. Never an error: the rest still happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenWarning {
    /// The token it is about, when it is about one.
    pub token: Option<String>,
    /// What happened, in a sentence.
    pub message: String,
}

impl TokenWarning {
    /// A warning about the token `name`.
    pub fn token(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            token: Some(name.into()),
            message: message.into(),
        }
    }

    /// A warning about the file or the set as a whole.
    pub fn general(message: impl Into<String>) -> Self {
        Self {
            token: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for TokenWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.token {
            Some(token) => write!(f, "{token}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// What a format needs besides the tokens. Every field is data the person can change.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenOptions {
    /// Which selector each theme value is written under.
    pub mapping: ThemeMapping,
    /// Token namespaces written under another name in the file: `space` → `spacing` writes `space/4` as
    /// `--spacing-4`, and reads it back as `space/4`.
    pub namespaces: BTreeMap<String, String>,
    /// Name prefixes whose numbers are lengths, written with `px` (`radius` covers `radius/md`).
    pub lengths: Vec<String>,
    /// The names already in the document and their types, so a file's `--color-brand-primary` is read as the
    /// existing `color/brand/primary` and not split the way a new name is, and a reference to a token the file
    /// does not define can still be typed.
    pub known: BTreeMap<String, VariableType>,
}

/// The prefixes whose numbers are lengths unless the person says otherwise.
pub const DEFAULT_LENGTHS: &[&str] = &[
    "space",
    "spacing",
    "radius",
    "size",
    "width",
    "height",
    "gap",
    "border",
    "stroke",
    "blur",
    "container",
    "breakpoint",
    "text",
    "font/size",
];

impl TokenOptions {
    /// The defaults for a document whose theme axes are `themes`.
    pub fn for_themes(themes: &BTreeMap<String, Vec<String>>) -> Self {
        Self {
            mapping: ThemeMapping::derive(themes),
            namespaces: BTreeMap::new(),
            lengths: DEFAULT_LENGTHS.iter().map(|s| (*s).to_owned()).collect(),
            known: BTreeMap::new(),
        }
    }

    /// Whether the numbers of the token `name` are lengths.
    pub fn is_length(&self, name: &str) -> bool {
        self.lengths.iter().any(|prefix| {
            name == prefix
                || name
                    .strip_prefix(prefix.as_str())
                    .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('-'))
        })
    }
}
