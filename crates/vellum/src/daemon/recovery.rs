//! Recovery copies (ARCHITECTURE §6 Host behaviours, DESIGN.md A06; guide Prompt 28).
//!
//! While an editor has unsaved changes to a file, the daemon writes them to `~/.vellum/recovery/` every few
//! seconds. A save removes the copy. After an unexpected quit, opening the file again lists the copy, and the
//! person chooses Save, Save As…, Discard backup or Cancel. Recovery copies are not history; Git is. Nothing here
//! is needed to open a file (guide §7 Ownership).
//!
//! Each copy is two files named by a hash of the document's path: `<key>.vellum.recovery`, the document in the
//! canonical format, and `<key>.json`, which says whose copy it is and when it was made. Both are written
//! atomically, the document first, so a crash never leaves a description of a copy that is not there.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vellum_core::files::atomic_write;
use vellum_core::ports::{Codec, FileError, FileSystem, FsError, Timestamp};
use vellum_types::Document;

/// Makes `<home>/.vellum/recovery/`, readable by the user only (0700 on Unix; the user profile's own ACL on
/// Windows), and returns it. Recovery copies hold design content, so a folder that is a symlink, or is
/// readable by others, is refused rather than used or repaired.
pub fn recovery_dir(home: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    recovery_dir_in(&home.join(".vellum"))
}

/// As [`recovery_dir`], inside the daemon's private folder `base` (`~/.vellum`).
pub fn recovery_dir_in(base: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let base = base.to_path_buf();
    let dir = base.join("recovery");
    for path in [&base, &dir] {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err(std::io::Error::other(format!(
                    "{} is not a private folder",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                #[cfg(unix)]
                std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700)
                    .create(path)?;
                #[cfg(not(unix))]
                std::fs::DirBuilder::new().create(path)?;
            }
            Err(e) => return Err(e),
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(&dir)?.permissions().mode() & 0o077 != 0 {
            return Err(std::io::Error::other(format!(
                "{} is readable by other users",
                dir.display()
            )));
        }
    }
    Ok(dir)
}

/// How often unsaved changes are copied, in milliseconds.
pub const RECOVERY_INTERVAL_MS: i64 = 5_000;

const DOCUMENT: &str = ".vellum.recovery";
const META: &str = ".json";

/// A recovery copy found on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryCopy {
    /// The document's path, as the daemon was given it.
    pub path: String,
    /// When the copy was made (UTC milliseconds).
    pub saved_at: Timestamp,
    /// The unsaved document.
    pub document: Document,
}

/// A recovery copy that is there but cannot be used. Listed, so it is never silently lost.
#[derive(Debug, Clone, PartialEq)]
pub struct DamagedCopy {
    /// The files' common name in the recovery folder.
    pub key: String,
    /// Why it cannot be read.
    pub reason: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Meta {
    path: String,
    saved_at: i64,
}

/// The recovery folder.
pub struct Recovery {
    fs: Arc<dyn FileSystem>,
    codec: Arc<dyn Codec>,
    hash: fn(&[u8]) -> String,
}

impl Recovery {
    /// Copies kept at the root of `fs`, encoded with `codec`, named by `hash` of each document's path.
    pub fn new(fs: Arc<dyn FileSystem>, codec: Arc<dyn Codec>, hash: fn(&[u8]) -> String) -> Self {
        Self { fs, codec, hash }
    }

    fn key(&self, path: &str) -> String {
        let hash = (self.hash)(path.as_bytes());
        hash.chars().take(32).collect()
    }

    /// Writes (or replaces) the copy of `path`'s unsaved document.
    pub fn write(&self, path: &str, document: &Document, now: Timestamp) -> Result<(), FileError> {
        let key = self.key(path);
        let bytes = self.codec.encode(document)?;
        atomic_write(&*self.fs, &format!("{key}{DOCUMENT}"), &bytes)?;
        let meta = serde_json::to_vec(&Meta {
            path: path.to_owned(),
            saved_at: now.0,
        })
        .map_err(|e| FsError::Unavailable(e.to_string()))?;
        atomic_write(&*self.fs, &format!("{key}{META}"), &meta)?;
        Ok(())
    }

    /// Removes `path`'s copy, if there is one.
    pub fn discard(&self, path: &str) -> Result<(), FsError> {
        let key = self.key(path);
        // The description first: without it the document file is an orphan, never offered.
        for name in [format!("{key}{META}"), format!("{key}{DOCUMENT}")] {
            match self.fs.remove_file(&name) {
                Ok(()) | Err(FsError::NotFound(_)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// `path`'s copy, if there is a usable one.
    pub fn find(&self, path: &str) -> Option<RecoveryCopy> {
        self.read(&self.key(path)).ok().filter(|c| c.path == path)
    }

    fn read(&self, key: &str) -> Result<RecoveryCopy, String> {
        let meta = self
            .fs
            .read(&format!("{key}{META}"))
            .map_err(|e| e.to_string())?;
        let meta: Meta = serde_json::from_slice(&meta).map_err(|e| e.to_string())?;
        let bytes = self
            .fs
            .read(&format!("{key}{DOCUMENT}"))
            .map_err(|e| e.to_string())?;
        let document = self.codec.decode(&bytes).map_err(|e| e.to_string())?;
        Ok(RecoveryCopy {
            path: meta.path,
            saved_at: Timestamp(meta.saved_at),
            document,
        })
    }

    /// Every copy, newest first, and every copy that could not be read.
    pub fn list(&self) -> (Vec<RecoveryCopy>, Vec<DamagedCopy>) {
        let names = match self.fs.list("") {
            Ok(names) => names,
            Err(_) => return (Vec::new(), Vec::new()),
        };
        let (mut copies, mut damaged) = (Vec::new(), Vec::new());
        for key in names.iter().filter_map(|n| n.strip_suffix(META)) {
            match self.read(key) {
                Ok(copy) => copies.push(copy),
                Err(reason) => damaged.push(DamagedCopy {
                    key: key.to_owned(),
                    reason,
                }),
            }
        }
        copies.sort_by(|a, b| b.saved_at.cmp(&a.saved_at).then(a.path.cmp(&b.path)));
        (copies, damaged)
    }
}
