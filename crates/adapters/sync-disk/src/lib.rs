//! Immutable SHA-256 addressed `.vellum` snapshots. Only keys, never caller paths, cross this boundary.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use vellum_core::ports::sync::{Error, SnapshotStore};
use vellum_types::sync::MAX_SNAPSHOT_BYTES;
static STAGING_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub struct DiskSnapshots {
    root: PathBuf,
}
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn key_ok(key: &str) -> bool {
    key.len() == 64
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl DiskSnapshots {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, Error> {
        let root = root.as_ref();
        fs::create_dir_all(root).map_err(|_| Error::Unavailable)?;
        let meta = fs::symlink_metadata(root).map_err(|_| Error::Unavailable)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(Error::Unavailable);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(root, fs::Permissions::from_mode(0o700))
                .map_err(|_| Error::Unavailable)?;
        }
        Ok(Self {
            root: fs::canonicalize(root).map_err(|_| Error::Unavailable)?,
        })
    }
}
impl SnapshotStore for DiskSnapshots {
    fn put(&self, bytes: &[u8]) -> Result<String, Error> {
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(Error::Limit);
        }
        let key = digest(bytes);
        let target = self.root.join(format!("{key}.vellum"));
        if target.try_exists().map_err(|_| Error::Unavailable)? {
            self.get(&key)?;
            return Ok(key);
        }
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut staged = None;
        for _ in 0..256 {
            let sequence = STAGING_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = self
                .root
                .join(format!("{key}.pending-{}-{sequence}", std::process::id()));
            match options.open(&path) {
                Ok(file) => {
                    staged = Some((path, file));
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(Error::Unavailable),
            }
        }
        let (staging, mut file) = staged.ok_or(Error::Unavailable)?;
        let result = (|| {
            file.write_all(bytes).map_err(|_| Error::Unavailable)?;
            file.sync_all().map_err(|_| Error::Unavailable)?;
            // Publishing by hard link never replaces an existing snapshot, including a symlink.
            match fs::hard_link(&staging, &target) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    self.get(&key)?;
                }
                Err(_) => return Err(Error::Unavailable),
            }
            #[cfg(unix)]
            File::open(&self.root)
                .and_then(|f| f.sync_all())
                .map_err(|_| Error::Unavailable)?;
            Ok(key)
        })();
        let _ = fs::remove_file(staging);
        result
    }
    fn get(&self, key: &str) -> Result<Vec<u8>, Error> {
        if !key_ok(key) {
            return Err(Error::NotFound);
        }
        let path = self.root.join(format!("{key}.vellum"));
        let meta = fs::symlink_metadata(&path).map_err(|_| Error::NotFound)?;
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.len() > MAX_SNAPSHOT_BYTES as u64
        {
            return Err(Error::InvalidUpdate);
        }
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|_| Error::Unavailable)?
            .take(MAX_SNAPSHOT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Unavailable)?;
        if bytes.len() > MAX_SNAPSHOT_BYTES || digest(&bytes) != key {
            return Err(Error::InvalidUpdate);
        }
        Ok(bytes)
    }
}
