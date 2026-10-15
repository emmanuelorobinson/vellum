//! The local file system (guide Prompt 24): the `FileSystem` port over `std::fs`, and the `AssetStore` that keeps
//! images, fonts and shader files by content hash in `<name>.assets/` beside a document (ARCHITECTURE §4.3).
//!
//! An asset is one file named by the SHA-256 of its bytes, lower-case hex, with no extension (the media type is
//! not part of an asset's identity, so it is not stored). Every write goes through `atomic_write`, so a crash
//! leaves the whole asset or none of it. A file whose bytes no longer match its name is reported as damaged,
//! never handed out.

mod std_fs;

use std::sync::Arc;

use sha2::{Digest, Sha256};
use vellum_core::files::atomic_write;
use vellum_core::ports::{
    AssetCollect, AssetError, AssetHash, AssetStore, FileSystem, FsError, check_mime, check_path,
};

pub use std_fs::StdFs;

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The assets folder for a document: `designs/app.vellum` (a file, or a folder in the directory format) has
/// `designs/app.assets`. Only a trailing `.vellum` goes, so `brand.lib.vellum` has `brand.lib.assets`.
pub fn assets_dir_for(document: &str) -> String {
    let document = document.trim_end_matches('/');
    let (dir, name) = match document.rsplit_once('/') {
        Some((dir, name)) => (Some(dir), name),
        None => (None, document),
    };
    let stem = name.strip_suffix(".vellum").unwrap_or(name);
    match dir {
        Some(dir) => format!("{dir}/{stem}.assets"),
        None => format!("{stem}.assets"),
    }
}

/// The SHA-256 of `bytes`, which is an asset's hash.
fn hash_of(bytes: &[u8]) -> AssetHash {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    AssetHash::new(hex).expect("hex is a valid hash")
}

fn unavailable(error: FsError) -> AssetError {
    AssetError::Unavailable(error.to_string())
}

/// Assets by content hash in one folder of a [`FileSystem`].
pub struct FsAssets {
    fs: Arc<dyn FileSystem>,
    dir: String,
}

impl FsAssets {
    /// Assets kept in `dir`, a path inside the file system's root such as `app.assets`.
    pub fn new(fs: Arc<dyn FileSystem>, dir: impl Into<String>) -> Result<Self, FsError> {
        let dir = dir.into();
        check_path(&dir, false)?;
        Ok(Self { fs, dir })
    }

    fn path_of(&self, hash: &AssetHash) -> String {
        format!("{}/{hash}", self.dir)
    }
}

impl AssetStore for FsAssets {
    fn get(&self, hash: &AssetHash) -> Result<Vec<u8>, AssetError> {
        let bytes = match self.fs.read(&self.path_of(hash)) {
            Ok(bytes) => bytes,
            Err(FsError::NotFound(_)) => return Err(AssetError::NotFound(hash.clone())),
            Err(other) => return Err(unavailable(other)),
        };
        if &hash_of(&bytes) != hash {
            return Err(AssetError::Unavailable(format!(
                "asset {hash} is damaged: its bytes do not match its name"
            )));
        }
        Ok(bytes)
    }

    fn put(&self, bytes: &[u8], mime: &str) -> Result<AssetHash, AssetError> {
        check_mime(mime)?;
        let hash = hash_of(bytes);
        let path = self.path_of(&hash);
        // The same bytes are stored once. A file that is there but wrong (a copy that was cut short, an
        // edit) is written again rather than trusted.
        if let Ok(existing) = self.fs.read(&path)
            && existing == bytes
        {
            return Ok(hash);
        }
        atomic_write(self.fs.as_ref(), &path, bytes).map_err(unavailable)?;
        Ok(hash)
    }
}

impl AssetCollect for FsAssets {
    fn list(&self) -> Result<Vec<AssetHash>, AssetError> {
        let names = match self.fs.list(&self.dir) {
            Ok(names) => names,
            Err(FsError::NotFound(_)) => return Ok(Vec::new()),
            Err(other) => return Err(unavailable(other)),
        };
        // Temporary files, `.DS_Store`, notes a person dropped in: not ours, so not listed and never removed.
        let mut hashes: Vec<AssetHash> = names
            .iter()
            .filter(|name| name.len() == 64)
            .filter_map(|name| AssetHash::new(name.as_str()).ok())
            .collect();
        hashes.sort();
        Ok(hashes)
    }

    fn remove(&self, hash: &AssetHash) -> Result<(), AssetError> {
        match self.fs.remove_file(&self.path_of(hash)) {
            Ok(()) => Ok(()),
            Err(FsError::NotFound(_)) => Err(AssetError::NotFound(hash.clone())),
            Err(other) => Err(unavailable(other)),
        }
    }
}
