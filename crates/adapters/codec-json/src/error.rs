//! What can go wrong reading or writing a file, in the words DESIGN S04 and S05 need.

use std::fmt;

/// A place in the text of a file. Both numbers start at 1, and the column counts characters, not bytes, as an
/// editor does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// The line.
    pub line: usize,
    /// The column.
    pub column: usize,
}

/// Why a file could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// The text is not JSON (S05).
    Syntax {
        /// Where the parser gave up.
        position: Position,
        /// What it expected.
        message: String,
    },
    /// The text is JSON but not a `.vellum` document (S05): an unknown field, a wrong type, a missing field.
    Invalid {
        /// The path to the object or value at fault, such as `pages[0].children[2].width`; empty for the
        /// document itself.
        path: String,
        /// Where it is in the text; none when the file was migrated first and the text no longer matches.
        position: Option<Position>,
        /// What is wrong.
        message: String,
    },
    /// The file was made by a newer version of Vellum (S04). It opens read-only.
    NewerFormat {
        /// The version in the file.
        found: u32,
        /// The newest version this build reads.
        supported: u32,
    },
    /// A migration from an older version failed.
    Migration {
        /// The version the migration was leaving.
        from: u32,
        /// What went wrong.
        message: String,
    },
    /// A number that is not finite cannot be written to JSON.
    NotFinite {
        /// Where it sits, such as `pages[0].children[0].opacity`.
        path: String,
    },
    /// The document says it is a version this codec does not write.
    WrongVersion {
        /// The version in the document.
        found: u32,
        /// The version this codec writes.
        expected: u32,
    },
}

impl CodecError {
    /// Where the fault is in the text, when that is known.
    pub fn position(&self) -> Option<Position> {
        match self {
            Self::Syntax { position, .. } => Some(*position),
            Self::Invalid { position, .. } => *position,
            _ => None,
        }
    }

    /// The path to the fault in the document, when there is one.
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Invalid { path, .. } | Self::NotFinite { path } => Some(path),
            _ => None,
        }
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { position, message } => write!(
                f,
                "This file is not valid JSON: {message} (line {}, column {}).",
                position.line, position.column
            ),
            Self::Invalid {
                path,
                position,
                message,
            } => {
                let place = if path.is_empty() {
                    "the document"
                } else {
                    path
                };
                write!(
                    f,
                    "This file is not a Vellum document: {message}, in {place}"
                )?;
                match position {
                    Some(p) => write!(f, " (line {}, column {}).", p.line, p.column),
                    None => f.write_str("."),
                }
            }
            Self::NewerFormat { found, .. } => write!(
                f,
                "Made with a newer version of Vellum (format {found}). Update to edit."
            ),
            Self::Migration { from, message } => write!(
                f,
                "This file is in an older format (format {from}) and could not be brought up to date: {message}."
            ),
            Self::NotFinite { path } => {
                write!(
                    f,
                    "The number at {path} is not finite, so it cannot be saved."
                )
            }
            Self::WrongVersion { found, expected } => write!(
                f,
                "The document is in format {found}, and this build writes format {expected}."
            ),
        }
    }
}

impl std::error::Error for CodecError {}
