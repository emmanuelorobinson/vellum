//! Files (guide Prompt 24). Core never touches a disk; whatever persists a document, an asset or a cached
//! library does it through this port, which is also what lets a test pull the plug half way through a save.
//!
//! Paths are relative to a root the implementation owns, use `/`, and never contain `..`, a leading `/`, an
//! empty part, a `\` or a NUL (guide §9: paths use `/`). The empty path names the root folder, where a folder is
//! meant. Names are compared as given: no case folding, no normalisation.

use std::fmt;

/// Why a file operation failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    /// There is no file or folder here.
    NotFound(String),
    /// The path is not a relative `/` path inside the root.
    InvalidPath(String),
    /// The disk could not do it (full, read-only, permission, unplugged).
    Unavailable(String),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "{path:?} is not there."),
            Self::InvalidPath(path) => write!(
                f,
                "{path:?} is not a path inside the folder: use a relative path with / and no .."
            ),
            Self::Unavailable(why) => write!(f, "The disk could not do that: {why}"),
        }
    }
}

impl std::error::Error for FsError {}

/// Checks a path against the rules in the module docs. `allow_root` lets the empty path through, for the
/// operations that take a folder.
pub fn check_path(path: &str, allow_root: bool) -> Result<(), FsError> {
    if path.is_empty() {
        return if allow_root {
            Ok(())
        } else {
            Err(FsError::InvalidPath(path.to_owned()))
        };
    }
    let bad = path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..");
    if bad {
        Err(FsError::InvalidPath(path.to_owned()))
    } else {
        Ok(())
    }
}

/// A small file system: whole files, folders, rename, and the two syncs a durable write needs.
///
/// Nothing here is atomic except `rename`. `write` can leave part of a file behind if the power goes, which is
/// why saves go through [`crate::files::atomic_write`].
pub trait FileSystem {
    /// The bytes of the file.
    fn read(&self, path: &str) -> Result<Vec<u8>, FsError>;
    /// Creates the file or replaces its bytes. The folder it sits in must exist. Not durable, not atomic.
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), FsError>;
    /// Makes the bytes of the file survive a crash.
    fn sync_file(&self, path: &str) -> Result<(), FsError>;
    /// Moves a file, replacing what is at `to`. Readers see the old file or the new one, never neither.
    fn rename(&self, from: &str, to: &str) -> Result<(), FsError>;
    /// Removes a file. A missing file is `NotFound`.
    fn remove_file(&self, path: &str) -> Result<(), FsError>;
    /// The names (not paths) of what is in the folder, files and folders, sorted. A missing folder is `NotFound`.
    fn list(&self, dir: &str) -> Result<Vec<String>, FsError>;
    /// Makes the folder and the ones above it. Already there is fine.
    fn create_dir_all(&self, dir: &str) -> Result<(), FsError>;
    /// Makes the renames and new files in the folder survive a crash.
    fn sync_dir(&self, dir: &str) -> Result<(), FsError>;
    /// Whether a file or folder is there.
    fn exists(&self, path: &str) -> Result<bool, FsError>;
}
