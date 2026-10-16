//! The daemon's documents (guide Prompt 28): live and headless targets, the file watcher and recovery copies.
//! Transport and lifecycle are in `vellum-ipc-local` (Prompt 27); tools reach these targets from Prompt 29.

mod documents;
mod headless;
mod live;
mod recovery;

pub use documents::{Documents, Env, External, Opened};
pub use headless::{DocFile, Engine, HeadlessTarget};
pub use live::{EditorId, EditorLink, LiveTarget, Role};
pub use recovery::{DamagedCopy, RECOVERY_INTERVAL_MS, Recovery, RecoveryCopy, recovery_dir};
