//! TEST DATA. A repo with a design in it and the daemon that serves it, for tests that run the real binary.
#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::{design, encode};

static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A repo with `design/app.vellum` in it and a daemon, both removed when the test ends. The daemon's own output
/// is kept and shown when the test fails.
pub struct Workspace {
    pub root: PathBuf,
    daemon: Child,
}

impl Workspace {
    pub fn new() -> Self {
        Self::with(&encode(&design()))
    }

    /// As [`Workspace::new`], with `bytes` as the design file.
    pub fn with(bytes: &[u8]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "v-mcp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("repo/.git")).unwrap();
        std::fs::create_dir_all(root.join("repo/design")).unwrap();
        std::fs::create_dir_all(root.join("state")).unwrap();
        std::fs::write(root.join("repo/design/app.vellum"), bytes).unwrap();
        let log = std::fs::File::create(root.join("daemon.log")).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_vellum"));
        command
            .arg("--run-dir")
            .arg(root.join("state/run"))
            .arg("daemon")
            .env("RUST_BACKTRACE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        let daemon = command.spawn().expect("start the daemon");
        let space = Self { root, daemon };
        let until = Instant::now() + Duration::from_secs(10);
        while !space
            .vellum()
            .arg("status")
            .output()
            .unwrap()
            .status
            .success()
        {
            assert!(Instant::now() < until, "the daemon did not start");
            std::thread::sleep(Duration::from_millis(20));
        }
        space
    }

    pub fn repo(&self) -> PathBuf {
        self.root.join("repo")
    }

    pub fn run_dir(&self) -> PathBuf {
        self.root.join("state/run")
    }

    /// The binary, pointed at this workspace's daemon.
    pub fn vellum(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_vellum"));
        command.arg("--run-dir").arg(self.run_dir());
        command
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = self.vellum().args(["daemon", "stop"]).output();
        let _ = self.daemon.wait();
        if std::thread::panicking()
            && let Ok(log) = std::fs::read_to_string(self.root.join("daemon.log"))
            && !log.is_empty()
        {
            eprintln!("---- daemon output ----\n{log}");
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
