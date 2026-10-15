//! A document kept at a path (guide Prompt 24). `Codec` turns a document into bytes; this port turns it into
//! files, which is what a directory format needs: one file per page and an index. ARCHITECTURE §4.3 calls the
//! directory format a second `Codec`; bytes in and bytes out cannot say which page a byte belongs to, so the
//! port is wider (ADR-018). The single-file format is the same port over a `Codec` (`files::SingleFile`).
//!
//! Switching formats is read with one and write with the other, and loses nothing.

use std::fmt;

use vellum_types::Document;

use super::{CodecError, FileSystem, FsError};

/// A document read from a path.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The document, migrated to the current format version.
    pub document: Document,
    /// Files beside the document's own that belong to no page of it: what an interrupted save or a hand edit
    /// left. Never read, never removed by a read; the next write clears them.
    pub orphans: Vec<String>,
    /// A save was interrupted before it finished. Every file is whole, but pages may come from different
    /// moments. The editor says so and offers the recovery copy (Prompt 28).
    pub interrupted: bool,
}

/// What a write did, with names relative to the document's path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Saved {
    /// Files written, because they were new or changed.
    pub written: Vec<String>,
    /// Files already on disk exactly as they are.
    pub unchanged: usize,
    /// Files removed because the document no longer has them.
    pub removed: Vec<String>,
}

/// Why a document could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileError {
    /// The disk refused, or nothing is at the path.
    Fs(FsError),
    /// The content is not a document, or would not encode.
    Codec(CodecError),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fs(e) => e.fmt(f),
            Self::Codec(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for FileError {}

impl From<FsError> for FileError {
    fn from(e: FsError) -> Self {
        Self::Fs(e)
    }
}

impl From<CodecError> for FileError {
    fn from(e: CodecError) -> Self {
        Self::Codec(e)
    }
}

/// Reads and writes a document at a path of a file system. Deterministic: the same document always makes the
/// same files (Invariant 5).
pub trait DocumentFiles {
    /// The format's name in the registry. Stable.
    fn id(&self) -> &'static str;
    /// The document at `path`.
    fn read(&self, fs: &dyn FileSystem, path: &str) -> Result<Loaded, FileError>;
    /// Writes `doc` at `path`. Each file is replaced whole or not at all, and only files that changed are
    /// touched.
    fn write(&self, fs: &dyn FileSystem, path: &str, doc: &Document) -> Result<Saved, FileError>;
}
