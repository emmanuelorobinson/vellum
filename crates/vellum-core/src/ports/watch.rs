//! Watching documents on disk (ARCHITECTURE §4.5; guide Prompt 28). A checkout, a merge, another editor or an
//! agent writing the file directly all show up here; the daemon reads the file again, diffs it and applies the
//! difference as ops attributed to `Actor::File`. The watcher only says *which document* may have changed; it
//! never reads the content, so it cannot decide anything about it.

use super::FsError;

/// Says which watched documents may have changed on disk.
///
/// A document is named by its native path: a file, or a folder in the directory format, where a change to any
/// file inside it counts. Editors save by writing a temporary file and renaming it over the document, and Git
/// replaces files the same way, so a watcher follows the name, not the file it first saw.
pub trait FileWatcher {
    /// The adapter's name. Stable.
    fn id(&self) -> &'static str;
    /// Starts watching the document at `path`. The folder it sits in must exist; the document need not, since
    /// a rename can bring it. Watching it again is fine.
    fn watch(&mut self, path: &str) -> Result<(), FsError>;
    /// Stops watching it. Not watching it is fine.
    fn unwatch(&mut self, path: &str);
    /// The watched documents that changed since the last call and have since been quiet long enough for a save
    /// to be over, each once, sorted. Never blocks.
    fn changed(&mut self) -> Vec<String>;
}
