//! What a file-backed document needs beyond the codec: durable writes (this module), the assets a document
//! points at, libraries on disk and the large-file suggestion (guide Prompt 24).
//!
//! Everything here works through ports (`FileSystem`, `AssetStore`, `Codec`), so it is pure and can be crashed on
//! purpose in tests.

mod assets;
mod atomic;
mod library;
mod single;
mod threshold;

pub use assets::{GcReport, SaveKind, asset_in_path, collect_on_save, referenced_assets};
pub use atomic::{TEMP_SUFFIX, atomic_write, recover_temps};
pub use library::{
    CopyReport, FileLibrarySource, LibraryCache, LibraryError, PersistReport, WarmReport,
    copy_dependencies, library_path_for, turn_into_library,
};
pub use single::SingleFile;
pub use threshold::{LARGE_FILE_BYTES, LARGE_FILE_NODES, count_nodes, suggest_multi_file};
