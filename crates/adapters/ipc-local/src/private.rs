use crate::invalid;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

#[cfg(windows)]
#[path = "windows_security.rs"]
mod windows_security;
#[cfg(windows)]
pub(crate) use windows_security::{pipe, sid};

#[derive(Clone)]
pub struct RunDir {
    path: PathBuf,
}
impl RunDir {
    /// Reject unsafe preexisting paths. Never follows a symlink to discovery or a lock file.
    pub fn new(path: PathBuf) -> io::Result<Self> {
        prepare_dir(&path)?;
        Ok(Self {
            path: fs::canonicalize(path)?,
        })
    }
    pub fn for_user() -> io::Result<Self> {
        #[cfg(unix)]
        let home = std::env::var_os("HOME");
        #[cfg(windows)]
        let home = std::env::var_os("USERPROFILE");
        let root = PathBuf::from(home.ok_or_else(|| invalid("home directory is unavailable"))?)
            .join(".vellum");
        prepare_dir(&root)?;
        Self::new(root.join("run"))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn file(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
    pub(crate) fn lock(&self, name: &str) -> io::Result<File> {
        let path = self.file(name);
        open_private(&path, true, false, true)
    }
    pub(crate) fn metadata(&self) -> io::Result<Metadata> {
        let path = self.file("daemon.json");
        check_file(&path)?;
        let mut bytes = Vec::new();
        open_private(&path, false, false, false)?
            .take(8193)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 8192 {
            return Err(invalid("invalid daemon discovery"));
        }
        let m: Metadata =
            serde_json::from_slice(&bytes).map_err(|_| invalid("invalid daemon discovery"))?;
        // Never trust an arbitrary endpoint from disk, even if another same-user program wrote it.
        if m.endpoint != self.endpoint()?
            || m.token.len() != 64
            || !m.token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid("invalid daemon discovery"));
        }
        Ok(m)
    }
    pub(crate) fn publish(&self, metadata: &Metadata) -> io::Result<()> {
        let temp = self.file("daemon.json.new");
        if temp.exists() {
            check_file(&temp)?;
            fs::remove_file(&temp)?;
        }
        let mut file = open_private(&temp, true, true, true)?;
        file.write_all(
            &serde_json::to_vec(metadata).map_err(|_| invalid("cannot encode discovery"))?,
        )?;
        file.sync_all()?;
        #[cfg(windows)]
        {
            // Windows rename cannot replace an open/existing file. Lock excludes other publishers;
            // readers retry across this short discovery gap.
            if self.file("daemon.json").exists() {
                check_file(&self.file("daemon.json"))?;
                fs::remove_file(self.file("daemon.json"))?;
            }
        }
        fs::rename(temp, self.file("daemon.json"))?;
        Ok(())
    }
    pub(crate) fn endpoint(&self) -> io::Result<String> {
        #[cfg(unix)]
        {
            self.file("vellum.sock")
                .to_str()
                .map(String::from)
                .ok_or_else(|| invalid("socket path is not UTF-8"))
        }
        #[cfg(windows)]
        {
            // SID prevents collisions between users; suffix isolates explicit run directories.
            let mut hash = 0xcbf29ce484222325_u64;
            for byte in self.path.to_string_lossy().as_bytes() {
                hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
            }
            Ok(format!(r"\\.\pipe\vellum-{}-{hash:016x}", sid()?))
        }
    }
}

// No Debug implementation: the discovery token is a local rendezvous credential, never a log field.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Metadata {
    pub v: u32,
    pub pid: u32,
    pub endpoint: String,
    pub token: String,
    pub tcp: Option<String>,
}

pub(crate) fn token() -> io::Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("OS entropy unavailable"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(unix)]
fn open_private(path: &Path, create: bool, exclusive: bool, write: bool) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(write)
        .create(create)
        .create_new(exclusive)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    check_file(path)?;
    Ok(file)
}
#[cfg(windows)]
fn open_private(path: &Path, create: bool, exclusive: bool, write: bool) -> io::Result<File> {
    let file = windows_security::open_file(path, create, exclusive, write)?;
    check_file(path)?;
    Ok(file)
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn owned(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid takes no pointers and cannot fail.
    metadata.uid() == unsafe { libc::geteuid() }
}
#[cfg(unix)]
fn prepare_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e),
    }
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_dir() || !owned(&meta) || meta.permissions().mode() & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "run directory must be owned by you with mode 0700",
        ));
    }
    Ok(())
}
#[cfg(unix)]
pub(crate) fn check_file(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file()
        || !owned(&meta)
        || meta.permissions().mode() & 0o777 != 0o600
        || meta.nlink() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "daemon file must be private, regular and owned by you",
        ));
    }
    Ok(())
}
#[cfg(windows)]
fn prepare_dir(path: &Path) -> io::Result<()> {
    windows_security::prepare_dir(path)
}
#[cfg(windows)]
pub(crate) fn check_file(path: &Path) -> io::Result<()> {
    windows_security::check_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "v-private-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }
    #[test]
    fn discovery_is_private_and_a_symlink_is_never_a_runtime_directory() {
        let path = path();
        let dir = RunDir::new(path.clone()).unwrap();
        let m = Metadata {
            v: 1,
            pid: 1,
            endpoint: dir.endpoint().unwrap(),
            token: token().unwrap(),
            tcp: None,
        };
        dir.publish(&m).unwrap();
        check_file(&dir.file("daemon.json")).unwrap();
        assert!(
            dir.metadata().unwrap().token == m.token,
            "discovery token changed"
        );
        // Validating the files on Windows inspects the owner and every ACE, not file modes.
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt, symlink};
            let link = path.with_extension("link");
            symlink(&path, &link).unwrap();
            assert!(RunDir::new(link.clone()).is_err());
            fs::remove_file(link).unwrap();
            fs::set_permissions(dir.file("daemon.json"), fs::Permissions::from_mode(0o644))
                .unwrap();
            assert!(dir.metadata().is_err());
        }
        fs::remove_dir_all(path).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn locks_and_discovery_refuse_symlinks_and_hardlinks() {
        use std::os::unix::fs::symlink;
        let path = path();
        let dir = RunDir::new(path.clone()).unwrap();
        let file = dir.lock("source").unwrap();
        drop(file);
        symlink(dir.file("source"), dir.file("daemon.lock")).unwrap();
        assert!(dir.lock("daemon.lock").is_err());
        fs::hard_link(dir.file("source"), dir.file("daemon.json")).unwrap();
        assert!(dir.metadata().is_err());
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn token_is_os_random_and_fixed_length() {
        let a = token().unwrap();
        let b = token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a != b, "OS entropy repeated a token");
    }
}
