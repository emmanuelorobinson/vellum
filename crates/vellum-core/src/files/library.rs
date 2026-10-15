//! Libraries on disk (ARCHITECTURE §4.4): turning a document into one, reading `*.lib.vellum` files, keeping a
//! last-known copy in the cache folder (`~/.vellum/cache/`), and carrying asset bytes when content is pasted.
//!
//! Document paths are whatever the host names them, relative to the root of the [`FileSystem`] it hands in, and
//! they should be the same names for the same file every time: the cache is shared by every project on the
//! machine and keyed by path. A host that roots its file system at the project must also give the project's
//! libraries a path that is not just `ui.lib.vellum`, or two projects share one cached copy.

use std::collections::BTreeSet;
use std::fmt;

use vellum_types::{AssetId, Document, DocumentKind};

use super::{asset_in_path, atomic_write};
use crate::ops::Shell;
use crate::ports::{AssetHash, AssetStore, Codec, FileError, FileSystem, FsError};
use crate::resolver::libraries::{Libraries, LibrarySource, normalize};

/// Why a document cannot become a library, or a path cannot name one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    /// It already is one. Turning a document into a library is one-way, and there is no way back.
    AlreadyLibrary,
    /// The name does not end in `.vellum`.
    NotAVellumFile(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyLibrary => f.write_str("This file is already a library."),
            Self::NotAVellumFile(path) => write!(f, "{path:?} is not a .vellum file."),
        }
    }
}

impl std::error::Error for LibraryError {}

/// The shell that makes `doc` a library. Apply it with `Op::SetShell`, like any other change; the editor asks
/// first, because nothing turns a library back into a document. (An undo restores it, and so does checking out
/// an older file: neither is a command.)
pub fn turn_into_library(doc: &Document) -> Result<Shell, LibraryError> {
    if doc.meta.kind == DocumentKind::Library {
        return Err(LibraryError::AlreadyLibrary);
    }
    let mut shell = Shell::of(doc);
    shell.meta.kind = DocumentKind::Library;
    Ok(shell)
}

/// The file a document's library goes in: `designs/app.vellum` becomes `designs/app.lib.vellum`.
pub fn library_path_for(path: &str) -> Result<String, LibraryError> {
    if path.ends_with(".lib.vellum") {
        return Ok(path.to_owned());
    }
    match path.strip_suffix(".vellum") {
        Some(stem) if !stem.is_empty() && !stem.ends_with('/') => Ok(format!("{stem}.lib.vellum")),
        _ => Err(LibraryError::NotAVellumFile(path.to_owned())),
    }
}

/// Reads libraries from a file system through a codec. Missing and unreadable are different answers, as
/// [`LibrarySource`] asks: a file that is not there is `None`, one that cannot be read says why in the words of
/// DESIGN S05.
pub struct FileLibrarySource<'a> {
    fs: &'a dyn FileSystem,
    codec: &'a dyn Codec,
}

impl<'a> FileLibrarySource<'a> {
    /// A source reading from `fs`.
    pub fn new(fs: &'a dyn FileSystem, codec: &'a dyn Codec) -> Self {
        Self { fs, codec }
    }
}

impl LibrarySource for FileLibrarySource<'_> {
    fn load(&self, path: &str) -> Result<Option<Document>, String> {
        // `/proj/ui.lib.vellum` is the file `proj/ui.lib.vellum` under the root.
        match self.fs.read(path.trim_start_matches('/')) {
            Ok(bytes) => self
                .codec
                .decode(&bytes)
                .map(Some)
                .map_err(|e| e.to_string()),
            Err(FsError::NotFound(_)) => Ok(None),
            Err(other) => Err(other.to_string()),
        }
    }
}

/// What [`LibraryCache::persist`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PersistReport {
    /// Copies written because they were new or changed.
    pub written: usize,
    /// Copies already on disk exactly as they are.
    pub unchanged: usize,
}

/// What [`LibraryCache::warm`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WarmReport {
    /// Paths whose last-known copy was loaded.
    pub seeded: Vec<String>,
    /// Paths that have a copy that could not be used, and why. A path with no copy at all is not here.
    pub skipped: Vec<(String, String)>,
}

/// Last-known copies of libraries, in a folder of a [`FileSystem`] (the host roots it at `~/.vellum/cache/`).
///
/// A copy is two files made from the library's path: `<name>-<hash>.lib.vellum`, written by the codec, and
/// `<name>-<hash>.source`, the path itself. The hash is a short fingerprint of the path (FNV-1a, not
/// cryptographic); the sidecar is how a collision or a stray file is told from the real copy, so a library is
/// never shown from another path's copy.
pub struct LibraryCache<'a> {
    fs: &'a dyn FileSystem,
    codec: &'a dyn Codec,
    dir: String,
}

fn fingerprint(path: &str) -> u64 {
    path.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

impl<'a> LibraryCache<'a> {
    /// A cache in `dir` of `fs`.
    pub fn new(fs: &'a dyn FileSystem, codec: &'a dyn Codec, dir: impl Into<String>) -> Self {
        Self {
            fs,
            codec,
            dir: dir.into(),
        }
    }

    fn names(&self, path: &str) -> (String, String) {
        let path = normalize(path);
        let file = path.rsplit('/').next().unwrap_or("");
        let stem: String = file
            .strip_suffix(".lib.vellum")
            .unwrap_or(file)
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .take(40)
            .collect();
        let key = format!("{stem}-{:016x}", fingerprint(&path));
        (
            format!("{}/{key}.lib.vellum", self.dir),
            format!("{}/{key}.source", self.dir),
        )
    }

    /// Writes a copy of every library `libraries` knows (fresh ones and the ones it seeded), skipping those
    /// already on disk exactly as they are.
    pub fn persist(&self, libraries: &Libraries<'_>) -> Result<PersistReport, FileError> {
        let mut report = PersistReport::default();
        for (path, doc) in libraries.cached() {
            let (file, sidecar) = self.names(path);
            let bytes = self.codec.encode(doc)?;
            let source = normalize(path).into_bytes();
            let current = self.fs.read(&file).is_ok_and(|b| b == bytes)
                && self.fs.read(&sidecar).is_ok_and(|b| b == source);
            if current {
                report.unchanged += 1;
                continue;
            }
            // The copy first, the name it belongs to second: a crash between them leaves a copy no path
            // claims, which `warm` skips and the next `persist` completes.
            atomic_write(self.fs, &file, &bytes)?;
            atomic_write(self.fs, &sidecar, &source)?;
            report.written += 1;
        }
        Ok(report)
    }

    /// Seeds `libraries` with the last-known copy of each path that has one, before the first resolve pass.
    pub fn warm(&self, libraries: &mut Libraries<'_>, paths: &[&str]) -> WarmReport {
        let mut report = WarmReport::default();
        for path in paths {
            let (file, sidecar) = self.names(path);
            let bytes = match self.fs.read(&file) {
                Ok(bytes) => bytes,
                Err(FsError::NotFound(_)) => continue,
                Err(other) => {
                    report.skipped.push(((*path).to_owned(), other.to_string()));
                    continue;
                }
            };
            let claimed = self.fs.read(&sidecar).ok();
            if claimed.as_deref() != Some(normalize(path).as_bytes()) {
                report.skipped.push((
                    (*path).to_owned(),
                    "the cached copy does not say it belongs to this path".to_owned(),
                ));
                continue;
            }
            let seeded = self
                .codec
                .decode(&bytes)
                .map_err(|e| e.to_string())
                .and_then(|doc| libraries.seed(path, doc));
            match seeded {
                Ok(()) => report.seeded.push((*path).to_owned()),
                Err(why) => report.skipped.push(((*path).to_owned(), why)),
            }
        }
        report
    }
}

/// What [`copy_dependencies`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CopyReport {
    /// Assets stored in the target, in the order the plan named them.
    pub copied: Vec<AssetHash>,
    /// Ids the plan needs that the source does not have (or that cannot be a stored hash).
    pub missing: Vec<String>,
    /// Ids the source could not give or the target would not take, with the reason.
    pub failed: Vec<(String, String)>,
    /// Plain files (scripts, shaders beside the document), sorted. A plan cannot move them; the host copies
    /// them next to the target document.
    pub files: Vec<String>,
}

/// Carries the bytes a paste plan names (`PastePlan::dependencies`) from one document's assets to another's:
/// asset ids, `font:<id>` and `*.assets/<hash>` paths. Anything else is a file for the host. Nothing is
/// fabricated: an id the source lacks is reported in `missing`, and the pasted content keeps pointing at it.
pub fn copy_dependencies(
    from: &dyn AssetStore,
    to: &dyn AssetStore,
    dependencies: &[String],
) -> CopyReport {
    let mut report = CopyReport::default();
    let mut seen = BTreeSet::new();
    let mut files = BTreeSet::new();
    for dependency in dependencies {
        let id = if let Some(id) = dependency.strip_prefix("font:") {
            AssetId::new(id).ok()
        } else if let Ok(id) = AssetId::new(dependency.as_str()) {
            Some(id)
        } else {
            let in_assets = asset_in_path(dependency);
            if in_assets.is_none() && !dependency.is_empty() {
                files.insert(dependency.clone());
            }
            in_assets
        };
        let Some(id) = id else { continue };
        if !seen.insert(id.as_str().to_owned()) {
            continue;
        }
        let Ok(hash) = AssetHash::new(id.as_str()) else {
            report.missing.push(id.to_string());
            continue;
        };
        match from.get(&hash) {
            Ok(bytes) => match to.put(&bytes, "application/octet-stream") {
                Ok(stored) if stored == hash => report.copied.push(hash),
                Ok(stored) => report.failed.push((
                    id.to_string(),
                    format!("the target stored it as {stored}, not under its own hash"),
                )),
                Err(e) => report.failed.push((id.to_string(), e.to_string())),
            },
            Err(crate::ports::AssetError::NotFound(_)) => report.missing.push(id.to_string()),
            Err(e) => report.failed.push((id.to_string(), e.to_string())),
        }
    }
    report.files = files.into_iter().collect();
    report
}
