use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use vellum_core::ports::{FileSystem, FsError, check_path};

#[derive(Debug, Clone)]
struct File {
    bytes: Vec<u8>,
    /// Whether these bytes would survive a crash.
    synced: bool,
}

#[derive(Debug, Default)]
struct State {
    files: BTreeMap<String, File>,
    dirs: BTreeSet<String>,
    mutations: usize,
    crash_after: Option<usize>,
}

/// A file system in memory that can be crashed on purpose.
///
/// `crash_after(n)` lets `n` changing calls through (`write`, `sync_file`, `rename`, `remove_file`,
/// `create_dir_all`, `sync_dir`) and then fails every later one, as a disk that went away would. `after_crash`
/// is what a restart finds: files whose bytes were written but never synced come back with half of them (the
/// worst a real disk does), synced files come back whole, and a rename is either done or not.
///
/// What it does not model: a rename lost because its folder was not synced. Renames are all or nothing here, so
/// a missing `sync_dir` is not caught.
#[derive(Debug, Default)]
pub struct MemFs {
    state: Mutex<State>,
}

impl MemFs {
    /// An empty disk.
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts a file (and the folders above it) on the disk as if it had always been there, whole and synced.
    /// Not counted as a change.
    pub fn seed(&self, path: &str, bytes: &[u8]) {
        let mut state = self.state.lock().expect("lock");
        let mut dir = String::new();
        for part in path.split('/').take(path.split('/').count() - 1) {
            if !dir.is_empty() {
                dir.push('/');
            }
            dir.push_str(part);
            state.dirs.insert(dir.clone());
        }
        state.files.insert(
            path.to_owned(),
            File {
                bytes: bytes.to_vec(),
                synced: true,
            },
        );
    }

    /// How many changing calls have gone through.
    pub fn mutations(&self) -> usize {
        self.state.lock().expect("lock").mutations
    }

    /// After `n` more changing calls (counting those already made from now), every changing call fails.
    pub fn crash_after(&self, n: usize) {
        let mut state = self.state.lock().expect("lock");
        state.crash_after = Some(state.mutations + n);
    }

    /// The disk a restart would find.
    pub fn after_crash(&self) -> MemFs {
        let state = self.state.lock().expect("lock");
        let files = state
            .files
            .iter()
            .map(|(path, file)| {
                let kept = if file.synced {
                    file.bytes.clone()
                } else {
                    file.bytes[..file.bytes.len() / 2].to_vec()
                };
                (
                    path.clone(),
                    File {
                        bytes: kept,
                        synced: true,
                    },
                )
            })
            .collect();
        MemFs {
            state: Mutex::new(State {
                files,
                dirs: state.dirs.clone(),
                mutations: 0,
                crash_after: None,
            }),
        }
    }

    fn change(&self) -> Result<std::sync::MutexGuard<'_, State>, FsError> {
        let mut state = self.state.lock().expect("lock");
        if state
            .crash_after
            .is_some_and(|limit| state.mutations >= limit)
        {
            return Err(FsError::Unavailable("simulated crash".into()));
        }
        state.mutations += 1;
        Ok(state)
    }
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

impl State {
    fn dir_exists(&self, dir: &str) -> bool {
        dir.is_empty() || self.dirs.contains(dir)
    }
}

impl FileSystem for MemFs {
    fn read(&self, path: &str) -> Result<Vec<u8>, FsError> {
        check_path(path, false)?;
        let state = self.state.lock().expect("lock");
        state
            .files
            .get(path)
            .map(|f| f.bytes.clone())
            .ok_or_else(|| FsError::NotFound(path.to_owned()))
    }

    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), FsError> {
        check_path(path, false)?;
        let mut state = self.change()?;
        if !state.dir_exists(parent(path)) {
            return Err(FsError::NotFound(parent(path).to_owned()));
        }
        if state.dirs.contains(path) {
            return Err(FsError::Unavailable(format!("{path:?} is a folder")));
        }
        state.files.insert(
            path.to_owned(),
            File {
                bytes: bytes.to_vec(),
                synced: false,
            },
        );
        Ok(())
    }

    fn sync_file(&self, path: &str) -> Result<(), FsError> {
        check_path(path, false)?;
        let mut state = self.change()?;
        match state.files.get_mut(path) {
            Some(file) => {
                file.synced = true;
                Ok(())
            }
            None => Err(FsError::NotFound(path.to_owned())),
        }
    }

    fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        check_path(from, false)?;
        check_path(to, false)?;
        let mut state = self.change()?;
        if !state.dir_exists(parent(to)) {
            return Err(FsError::NotFound(parent(to).to_owned()));
        }
        let file = state
            .files
            .remove(from)
            .ok_or_else(|| FsError::NotFound(from.to_owned()))?;
        state.files.insert(to.to_owned(), file);
        Ok(())
    }

    fn remove_file(&self, path: &str) -> Result<(), FsError> {
        check_path(path, false)?;
        let mut state = self.change()?;
        state
            .files
            .remove(path)
            .map(|_| ())
            .ok_or_else(|| FsError::NotFound(path.to_owned()))
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, FsError> {
        check_path(dir, true)?;
        let state = self.state.lock().expect("lock");
        if !state.dir_exists(dir) {
            return Err(FsError::NotFound(dir.to_owned()));
        }
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let names: BTreeSet<String> = state
            .files
            .keys()
            .map(String::as_str)
            .chain(state.dirs.iter().map(String::as_str))
            .filter_map(|path| path.strip_prefix(&prefix))
            .filter(|rest| !rest.is_empty() && !rest.contains('/'))
            .map(str::to_owned)
            .collect();
        Ok(names.into_iter().collect())
    }

    fn create_dir_all(&self, dir: &str) -> Result<(), FsError> {
        check_path(dir, true)?;
        let mut state = self.change()?;
        let mut path = String::new();
        for part in dir.split('/').filter(|p| !p.is_empty()) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            if state.files.contains_key(&path) {
                return Err(FsError::Unavailable(format!("{path:?} is a file")));
            }
            state.dirs.insert(path.clone());
        }
        Ok(())
    }

    fn sync_dir(&self, dir: &str) -> Result<(), FsError> {
        check_path(dir, true)?;
        let state = self.change()?;
        if state.dir_exists(dir) {
            Ok(())
        } else {
            Err(FsError::NotFound(dir.to_owned()))
        }
    }

    fn exists(&self, path: &str) -> Result<bool, FsError> {
        check_path(path, true)?;
        let state = self.state.lock().expect("lock");
        Ok(state.files.contains_key(path) || state.dir_exists(path))
    }
}
