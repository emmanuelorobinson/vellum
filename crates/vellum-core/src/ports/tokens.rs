//! `TokenFormat` (ARCHITECTURE §3.5): a file format for design tokens. A format only converts between text and a
//! [`TokenSet`]; which document the tokens come from, what changes in it and where the file lives are not its
//! business. Adding SwiftUI or Kotlin tokens is one new implementation and a pass of the contract suite.
use std::fmt;

use crate::tokens::{TokenOptions, TokenSet, TokenWarning};

/// Text a format wrote, and what it could not say.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenExport {
    /// The file's contents.
    pub text: String,
    /// Tokens or values the format could not hold, left out of `text`.
    pub warnings: Vec<TokenWarning>,
}

/// Tokens a format read, and what it ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenImport {
    /// What the file holds.
    pub set: TokenSet,
    /// Parts of the file that were not tokens or could not be read as one, left out of `set`.
    pub warnings: Vec<TokenWarning>,
}

/// Why a file could not be written or read at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenError {
    /// What was wrong, in a sentence.
    pub message: String,
    /// The line it was on, counting from 1, when known.
    pub line: Option<u32>,
}

impl TokenError {
    /// An error with no position.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
        }
    }

    /// The same error, on `line`.
    #[must_use]
    pub fn at_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for TokenError {}

/// A token file format.
pub trait TokenFormat {
    /// The name tools and settings use: `css-vars`, `tailwind-v4`, `dtcg-json`.
    fn id(&self) -> &'static str;
    /// The name a person sees: "CSS variables".
    fn label(&self) -> &'static str;
    /// A file name to suggest, such as `tokens.css`.
    fn file_name(&self) -> &'static str;
    /// The options a person starts from for a document whose theme axes are `themes`.
    fn default_options(
        &self,
        themes: &std::collections::BTreeMap<String, Vec<String>>,
    ) -> TokenOptions {
        TokenOptions::for_themes(themes)
    }
    /// Writes `set`. The same set and options always give the same text.
    fn export(&self, set: &TokenSet, options: &TokenOptions) -> Result<TokenExport, TokenError>;
    /// Reads `text`. A value it cannot read is a warning; text that is not the format at all is an error.
    fn import(&self, text: &str, options: &TokenOptions) -> Result<TokenImport, TokenError>;
}
