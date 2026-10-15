//! Temp file, fsync, rename, fsync the folder: after a crash the target is the old file or the new one.

use crate::ports::{FileSystem, FsError, check_path};

/// The end of the name of a file being written. The temporary file sits beside its target (a rename cannot cross
/// a disk), starts with a `.` so listings skip it, and is only ever ours to remove.
pub const TEMP_SUFFIX: &str = ".vellum-tmp";

fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

fn temp_path(path: &str) -> String {
    let (dir, name) = split(path);
    if dir.is_empty() {
        format!(".{name}{TEMP_SUFFIX}")
    } else {
        format!("{dir}/.{name}{TEMP_SUFFIX}")
    }
}

/// Writes `bytes` to `path` so that a crash at any point leaves the file as it was, or as it is now, never half
/// of either. The folder is made if it is missing. A failed write removes its temporary file when it can.
///
/// Writers of one path must take turns; the daemon does that (Prompt 27). Two at once would share the
/// temporary file.
pub fn atomic_write(fs: &dyn FileSystem, path: &str, bytes: &[u8]) -> Result<(), FsError> {
    check_path(path, false)?;
    let (dir, _) = split(path);
    if !dir.is_empty() {
        fs.create_dir_all(dir)?;
    }
    let temp = temp_path(path);
    let result = (|| {
        fs.write(&temp, bytes)?;
        fs.sync_file(&temp)?;
        fs.rename(&temp, path)?;
        fs.sync_dir(dir)
    })();
    if result.is_err() {
        // The file system may be gone; the leftover is harmless and `recover_temps` finds it.
        let _ = fs.remove_file(&temp);
    }
    result
}

/// Removes temporary files that an interrupted write left in `dir`, and returns their names. A missing folder
/// has none.
pub fn recover_temps(fs: &dyn FileSystem, dir: &str) -> Result<Vec<String>, FsError> {
    let names = match fs.list(dir) {
        Ok(names) => names,
        Err(FsError::NotFound(_)) => return Ok(Vec::new()),
        Err(other) => return Err(other),
    };
    let mut removed = Vec::new();
    for name in names {
        if name.starts_with('.') && name.ends_with(TEMP_SUFFIX) {
            let path = if dir.is_empty() {
                name.clone()
            } else {
                format!("{dir}/{name}")
            };
            fs.remove_file(&path)?;
            removed.push(name);
        }
    }
    Ok(removed)
}
