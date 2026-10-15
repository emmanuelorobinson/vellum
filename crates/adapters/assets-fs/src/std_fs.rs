use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use vellum_core::ports::{FileSystem, FsError, check_path};

/// A folder on the real disk, as a [`FileSystem`]. Every path is resolved inside the folder; `..` and absolute
/// paths are refused before they reach the disk. A symbolic link inside the folder is followed like any other
/// entry: keeping links from pointing out of a project is the project's business, not this adapter's.
#[derive(Debug, Clone)]
pub struct StdFs {
    root: PathBuf,
}

impl StdFs {
    /// The file system rooted at `root`, which should exist.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &str, allow_root: bool) -> Result<PathBuf, FsError> {
        check_path(path, allow_root)?;
        let mut full = self.root.clone();
        full.extend(path.split('/').filter(|part| !part.is_empty()));
        Ok(full)
    }
}

fn map(path: &str, error: std::io::Error) -> FsError {
    match error.kind() {
        ErrorKind::NotFound => FsError::NotFound(path.to_owned()),
        _ => FsError::Unavailable(format!("{path}: {error}")),
    }
}

impl FileSystem for StdFs {
    fn read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        fs::read(self.resolve(path, false)?).map_err(|e| map(path, e))
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), FsError> {
        fs::write(self.resolve(path, false)?, bytes).map_err(|e| map(path, e))
    }

    fn sync_file(&self, path: &str) -> Result<(), FsError> {
        // Opened for writing: Windows refuses to flush a handle that cannot write.
        fs::OpenOptions::new()
            .write(true)
            .open(self.resolve(path, false)?)
            .and_then(|file| file.sync_all())
            .map_err(|e| map(path, e))
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        let (source, target) = (self.resolve(from, false)?, self.resolve(to, false)?);
        fs::rename(source, target).map_err(|e| map(from, e))
    }

    fn remove_file(&self, path: &str) -> Result<(), FsError> {
        fs::remove_file(self.resolve(path, false)?).map_err(|e| map(path, e))
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        let mut names = Vec::new();
        for entry in fs::read_dir(self.resolve(dir, true)?).map_err(|e| map(dir, e))? {
            let entry = entry.map_err(|e| map(dir, e))?;
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names)
    }

    fn create_dir_all(&self, dir: &str) -> Result<(), FsError> {
        fs::create_dir_all(self.resolve(dir, true)?).map_err(|e| map(dir, e))
    }

    #[cfg(unix)]
    fn sync_dir(&self, dir: &str) -> Result<(), FsError> {
        fs::File::open(self.resolve(dir, true)?)
            .and_then(|folder| folder.sync_all())
            .map_err(|e| map(dir, e))
    }

    /// Windows cannot open a folder as a file; NTFS journals the rename itself.
    #[cfg(not(unix))]
    fn sync_dir(&self, dir: &str) -> Result<(), FsError> {
        self.resolve(dir, true).map(|_| ())
    }

    fn exists(&self, path: &str) -> Result<bool, FsError> {
        self.resolve(path, true)?
            .try_exists()
            .map_err(|e| map(path, e))
    }
}
