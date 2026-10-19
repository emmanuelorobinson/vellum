//! Read-only FileSystem over a pinned Git tree. Never checks out or executes repository files, hooks,
//! filters, external diffs, symlinks or submodules. Assets/libraries come from the same commit as the design.
use super::{git, invalid};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
use vellum_core::ports::{FileSystem, FsError, check_path};
#[derive(Clone, PartialEq, Eq)]
struct Entry {
    mode: String,
    oid: String,
}
pub(super) struct Snapshot {
    root: PathBuf,
    entries: BTreeMap<String, Entry>,
}
impl Snapshot {
    pub fn new(root: &Path, sha: &str) -> std::io::Result<Self> {
        let bytes = git(root, &["ls-tree", "-r", "-z", "--full-tree", sha])?;
        let mut entries = BTreeMap::new();
        for record in bytes.split(|b| *b == 0).filter(|r| !r.is_empty()) {
            let tab = record
                .iter()
                .position(|b| *b == b'\t')
                .ok_or_else(|| invalid("Invalid Git tree entry"))?;
            let (header, tail) = record.split_at(tab);
            let path = &tail[1..];
            let header = std::str::from_utf8(header).map_err(invalid)?;
            let fields: Vec<_> = header.split(' ').collect();
            if fields.len() != 3 {
                return Err(invalid("Invalid Git tree header"));
            }
            let path = std::str::from_utf8(path)
                .map_err(|_| invalid("Git previews require UTF-8 paths"))?;
            entries.insert(
                path.into(),
                Entry {
                    mode: fields[0].into(),
                    oid: fields[2].into(),
                },
            );
        }
        Ok(Self {
            root: root.into(),
            entries,
        })
    }
    pub fn same_path(&self, other: &Self, path: &str) -> bool {
        let prefix = format!("{path}/");
        let under = |key: &&String| key.as_str() == path || key.starts_with(&prefix);
        self.entries
            .keys()
            .filter(under)
            .chain(other.entries.keys().filter(under))
            .all(|key| self.entries.get(key) == other.entries.get(key))
    }
    pub fn documents(&self) -> std::io::Result<BTreeSet<String>> {
        for (path, entry) in &self.entries {
            if (path.ends_with(".vellum") || path.ends_with(".vellum/index.json"))
                && !matches!(entry.mode.as_str(), "100644" | "100755")
            {
                return Err(invalid(format!(
                    "{path}: symlinks and submodules cannot be previewed"
                )));
            }
        }
        Ok(self
            .entries
            .keys()
            .filter_map(|p| {
                let mut root = String::new();
                for part in p.split('/') {
                    if !root.is_empty() {
                        root.push('/');
                    }
                    root.push_str(part);
                    if part.ends_with(".vellum") {
                        return Some(root);
                    }
                }
                None
            })
            .collect())
    }
}
fn readonly() -> FsError {
    FsError::Unavailable("Git snapshots are read-only".into())
}
impl FileSystem for Snapshot {
    fn read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        check_path(path, false)?;
        let entry = self
            .entries
            .get(path)
            .ok_or_else(|| FsError::NotFound(path.into()))?;
        if !matches!(entry.mode.as_str(), "100644" | "100755") {
            return Err(FsError::Unavailable(format!(
                "{path}: symlinks and submodules cannot be previewed"
            )));
        }
        let size = git(&self.root, &["cat-file", "-s", &entry.oid])
            .map_err(|e| FsError::Unavailable(e.to_string()))?;
        let size = std::str::from_utf8(&size)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .ok_or_else(|| FsError::Unavailable("Invalid blob size".into()))?;
        if size > 64 * 1024 * 1024 {
            return Err(FsError::Unavailable(format!(
                "{path}: Git preview blobs are limited to 64 MiB"
            )));
        }
        git(&self.root, &["cat-file", "blob", &entry.oid])
            .map_err(|e| FsError::Unavailable(e.to_string()))
    }
    fn list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        check_path(dir, true)?;
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let names: BTreeSet<_> = self
            .entries
            .keys()
            .filter_map(|p| p.strip_prefix(&prefix))
            .filter_map(|p| p.split('/').next())
            .map(str::to_owned)
            .collect();
        if names.is_empty() && !dir.is_empty() {
            Err(FsError::NotFound(dir.into()))
        } else {
            Ok(names.into_iter().collect())
        }
    }
    fn exists(&self, path: &str) -> Result<bool, FsError> {
        check_path(path, true)?;
        Ok(path.is_empty()
            || self.entries.contains_key(path)
            || self
                .entries
                .keys()
                .any(|p| p.starts_with(&format!("{path}/"))))
    }
    fn write(&self, _: &str, _: &[u8]) -> Result<(), FsError> {
        Err(readonly())
    }
    fn sync_file(&self, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
    fn sync_dir(&self, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
    fn remove_file(&self, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
    fn create_dir_all(&self, _: &str) -> Result<(), FsError> {
        Err(readonly())
    }
}
