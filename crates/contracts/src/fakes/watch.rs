use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use vellum_core::ports::{FileWatcher, FsError};

#[derive(Debug, Default)]
struct State {
    watched: BTreeSet<String>,
    changed: BTreeSet<String>,
}

/// A watcher that hears only what it is told. Clones share one state, so a test keeps a clone and calls
/// [`ManualWatcher::touch`] where a real disk would have written. No debounce: a touch is reported on the next
/// `changed`.
#[derive(Debug, Clone, Default)]
pub struct ManualWatcher(Arc<Mutex<State>>);

impl ManualWatcher {
    /// Watching nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Something at `path` changed: a watched document, or a file inside a watched folder document.
    pub fn touch(&self, path: &str) {
        let mut state = self.0.lock().expect("lock");
        let hit = state
            .watched
            .iter()
            .find(|doc| {
                path == doc.as_str()
                    || path
                        .strip_prefix(doc.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .cloned();
        if let Some(doc) = hit {
            state.changed.insert(doc);
        }
    }

    /// The documents being watched.
    pub fn watched(&self) -> Vec<String> {
        self.0
            .lock()
            .expect("lock")
            .watched
            .iter()
            .cloned()
            .collect()
    }
}

impl FileWatcher for ManualWatcher {
    fn id(&self) -> &'static str {
        "manual"
    }

    fn watch(&mut self, path: &str) -> Result<(), FsError> {
        self.0.lock().expect("lock").watched.insert(path.to_owned());
        Ok(())
    }

    fn unwatch(&mut self, path: &str) {
        let mut state = self.0.lock().expect("lock");
        state.watched.remove(path);
        state.changed.remove(path);
    }

    fn changed(&mut self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().expect("lock").changed)
            .into_iter()
            .collect()
    }
}
