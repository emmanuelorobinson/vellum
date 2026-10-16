//! IPC lane: real processes and local sockets/pipes; no renderer, account or external network.
use serde_json::Value;
#[cfg(unix)]
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Daemon {
    dir: PathBuf,
    child: Option<Child>,
}
impl Daemon {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "v-ipc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        Self { dir, child: None }
    }
    fn command(&self) -> Command {
        let mut c = Command::new(
            std::env::var_os("VELLUM_IPC_BINARY")
                .expect("ipc lane requires VELLUM_IPC_BINARY; run node scripts/ipc.mjs"),
        );
        c.arg("--run-dir").arg(&self.dir);
        c
    }
    fn start(&mut self, tcp: bool) {
        let mut c = self.command();
        c.arg("daemon");
        if tcp {
            c.args([
                "--listen",
                "127.0.0.1:0",
                "--allow-origin",
                "https://editor.example",
            ]);
        }
        self.child = Some(
            c.stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        self.wait_ready();
    }
    fn wait_ready(&self) {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if self
                .command()
                .arg("status")
                .output()
                .unwrap()
                .status
                .success()
            {
                break;
            }
            assert!(Instant::now() < until, "daemon did not become ready");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn metadata(&self) -> Value {
        serde_json::from_slice(&fs::read(self.dir.join("daemon.json")).unwrap()).unwrap()
    }
    fn bridge(&self, input: &[u8]) -> std::process::Output {
        let mut c = self
            .command()
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin.take().unwrap().write_all(input).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while c.try_wait().unwrap().is_none() {
            if Instant::now() > until {
                let _ = c.kill();
                let _ = c.wait();
                panic!("bridge timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        c.wait_with_output().unwrap()
    }
}
static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.command().args(["daemon", "stop"]).output();
        if let Some(c) = &mut self.child {
            let _ = c.kill();
            let _ = c.wait();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_lifecycle_single_instance_and_cleanup() {
    let mut d = Daemon::new();
    d.start(false);
    let first = d.metadata();
    assert!(first["tcp"].is_null());
    let second = d.command().arg("daemon").output().unwrap();
    assert!(!second.status.success());
    assert!(String::from_utf8_lossy(&second.stderr).contains("already running"));
    assert!(d.metadata() == first, "discovery changed unexpectedly");
    let status = d.command().arg("status").output().unwrap();
    assert!(status.status.success());
    assert!(!String::from_utf8_lossy(&status.stdout).contains(first["token"].as_str().unwrap()));
    assert!(
        d.command()
            .args(["daemon", "stop"])
            .status()
            .unwrap()
            .success()
    );
    assert!(d.child.as_mut().unwrap().wait().unwrap().success());
    assert!(!d.dir.join("daemon.json").exists());
    #[cfg(unix)]
    assert!(!d.dir.join("vellum.sock").exists());
}

#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_bridge_autostarts_reuses_and_restarts_after_crash() {
    let d = Daemon::new();
    let request = b"{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"tools/list\"}\n";
    let out = d.bridge(request);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(reply["id"], 42);
    assert_eq!(reply["error"]["code"], -32601); // Registry arrives in Prompt 29.
    let first = d.metadata();
    assert!(d.bridge(request).status.success());
    assert!(d.metadata() == first, "discovery changed unexpectedly");
    assert!(
        d.command()
            .args(["daemon", "stop"])
            .status()
            .unwrap()
            .success()
    );
    d.wait_ready_after_restart(request);
}
impl Daemon {
    fn wait_ready_after_restart(&self, request: &[u8]) {
        let until = Instant::now() + Duration::from_secs(5);
        while self.dir.join("daemon.json").exists() {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(self.bridge(request).status.success());
    }
}

#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_crash_releases_lock_and_rotates_token() {
    let mut d = Daemon::new();
    d.start(false);
    let old = d.metadata();
    d.child.as_mut().unwrap().kill().unwrap();
    d.child.as_mut().unwrap().wait().unwrap();
    assert!(
        d.bridge(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
            .status
            .success()
    );
    let new = d.metadata();
    assert!(old["token"] != new["token"], "restart reused a token");
    assert_ne!(old["pid"], new["pid"]);
}

#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_concurrent_bridges_start_one_daemon() {
    let d = Daemon::new();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| d.bridge(b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"ping\"}\n"))
            })
            .collect();
        for h in handles {
            let out = h.join().unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    });
    assert!(d.command().arg("status").output().unwrap().status.success());
}

#[cfg(unix)]
#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_private_files_authentication_version_and_partial_frames() {
    use std::{
        io::{BufRead, BufReader},
        os::unix::{fs::PermissionsExt, net::UnixStream},
    };
    let mut d = Daemon::new();
    d.start(false);
    for (name, mode) in [
        ("", 0o700),
        ("daemon.json", 0o600),
        ("vellum.sock", 0o600),
        ("daemon.lock", 0o600),
    ] {
        assert_eq!(
            fs::metadata(d.dir.join(name)).unwrap().permissions().mode() & 0o777,
            mode
        );
    }
    let meta = d.metadata();
    for (token, version, expected) in [
        ("wrong", 1, "unauthorized"),
        (meta["token"].as_str().unwrap(), 99, "version"),
    ] {
        let mut s = UnixStream::connect(d.dir.join("vellum.sock")).unwrap();
        writeln!(
            s,
            "{}",
            json!({"v":version,"token":token,"channel":"control"})
        )
        .unwrap();
        let mut reply = String::new();
        BufReader::new(s).read_line(&mut reply).unwrap();
        assert!(reply.contains(expected));
        assert!(!reply.contains(meta["token"].as_str().unwrap()));
    }
    let mut s = UnixStream::connect(d.dir.join("vellum.sock")).unwrap();
    writeln!(
        s,
        "{}",
        json!({"v":1,"token":meta["token"],"channel":"mcp"})
    )
    .unwrap();
    let mut reader = BufReader::new(s.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("accepted"));
    s.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":").unwrap();
    s.write_all(b"7,\"method\":\"missing\"}\n").unwrap();
    s.shutdown(std::net::Shutdown::Write).unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["id"], 7);
}

#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_tcp_is_explicit_and_checks_token_host_origin() {
    use std::io::Read;
    let mut d = Daemon::new();
    d.start(true);
    let m = d.metadata();
    let address = m["tcp"].as_str().unwrap();
    for (token, host, origin, code) in [
        ("bad", address, "https://editor.example", 401),
        (
            m["token"].as_str().unwrap(),
            "evil.example",
            "https://editor.example",
            403,
        ),
        (
            m["token"].as_str().unwrap(),
            address,
            "https://evil.example",
            403,
        ),
        (m["token"].as_str().unwrap(), address, "null", 403),
        (
            m["token"].as_str().unwrap(),
            address,
            "https://editor.example",
            200,
        ),
    ] {
        let mut s = std::net::TcpStream::connect(address).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let body = "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"ping\"}";
        write!(s,"POST /rpc HTTP/1.1\r\nHost: {host}\r\nOrigin: {origin}\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
        let mut reply = String::new();
        s.read_to_string(&mut reply).unwrap();
        assert!(
            reply.starts_with(&format!("HTTP/1.1 {code}")),
            "unexpected response status"
        );
        assert!(!reply.contains(m["token"].as_str().unwrap()));
    }
}

#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_default_process_has_no_tcp_listener() {
    let mut d = Daemon::new();
    d.start(false);
    let pid = d.child.as_ref().unwrap().id();
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("lsof")
            .args([
                "-nP",
                "-a",
                "-p",
                &pid.to_string(),
                "-iTCP",
                "-sTCP:LISTEN",
                "-Fn",
            ])
            .output()
            .expect("ipc lane needs lsof");
        assert!(matches!(output.status.code(), Some(0 | 1)));
        assert!(output.stdout.is_empty(), "default daemon opened TCP");
    }
    #[cfg(target_os = "linux")]
    {
        let sockets: Vec<_> = fs::read_dir(format!("/proc/{pid}/fd"))
            .unwrap()
            .filter_map(|e| fs::read_link(e.ok()?.path()).ok())
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        for table in ["tcp", "tcp6"] {
            for line in fs::read_to_string(format!("/proc/{pid}/net/{table}"))
                .unwrap()
                .lines()
                .skip(1)
            {
                let columns: Vec<_> = line.split_whitespace().collect();
                if columns[3] == "0A" {
                    assert!(
                        !sockets.contains(&format!("socket:[{}]", columns[9])),
                        "default daemon opened TCP"
                    );
                }
            }
        }
    }
    #[cfg(windows)]
    {
        let output=Command::new("powershell").args(["-NoProfile","-Command",&format!("@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue | Where-Object OwningProcess -eq {pid}).Count")]).output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
    }
}

#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_disconnect_exits_bridge_with_stdin_still_open() {
    let mut d = Daemon::new();
    d.start(false);
    let mut bridge = d
        .command()
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = bridge.stdin.take().unwrap();
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n")
        .unwrap();
    // Reading the response proves the bridge authenticated and started its stdin reader.
    use std::io::{BufRead, BufReader};
    let mut output = BufReader::new(bridge.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert!(!line.is_empty());
    d.child.as_mut().unwrap().kill().unwrap();
    d.child.as_mut().unwrap().wait().unwrap();
    let until = Instant::now() + Duration::from_secs(4);
    while bridge.try_wait().unwrap().is_none() {
        if Instant::now() > until {
            let _ = bridge.kill();
            let _ = bridge.wait();
            panic!("bridge did not exit on daemon disconnect with stdin open");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(input);
}

#[cfg(unix)]
#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes, lsof on macOS, PowerShell on Windows)"]
fn ipc_sigterm_cleans_up_active_and_unauthenticated_connections() {
    use std::os::unix::net::UnixStream;
    let mut d = Daemon::new();
    d.start(false);
    let _idle = UnixStream::connect(d.dir.join("vellum.sock")).unwrap();
    let result = Command::new("kill")
        .args(["-TERM", &d.child.as_ref().unwrap().id().to_string()])
        .status()
        .unwrap();
    assert!(result.success());
    let until = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(exit) = d.child.as_mut().unwrap().try_wait().unwrap() {
            assert!(exit.success());
            break;
        }
        assert!(Instant::now() < until, "shutdown did not complete");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!d.dir.join("daemon.json").exists());
    assert!(!d.dir.join("vellum.sock").exists());
}

#[test]
#[ignore = "ipc lane: node scripts/ipc.mjs (real local sockets/pipes)"]
fn ipc_restart_waits_for_graceful_shutdown_to_release_instance_lock() {
    let mut d = Daemon::new();
    d.start(true);
    let m = d.metadata();
    let mut slow = std::net::TcpStream::connect(m["tcp"].as_str().unwrap()).unwrap();
    slow.write_all(b"POST /rpc HTTP/1.1\r\nHost:").unwrap();
    // Keep an accepted HTTP request draining while the control connection acknowledges stop.
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        d.command()
            .args(["daemon", "stop"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        d.child.as_mut().unwrap().try_wait().unwrap().is_none(),
        "HTTP drain did not exercise the instance-lock overlap"
    );
    let out = d.bridge(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
