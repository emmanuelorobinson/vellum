use crate::{
    DEADLINE, MAX_FRAME, frame, invalid,
    private::RunDir,
    runtime, send,
    transport::{self, Stream},
};
use std::{
    io,
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::io::{AsyncWriteExt, BufReader};
use vellum_types::daemon::{DAEMON_PROTOCOL_VERSION, DaemonChannel, DaemonHello, DaemonWelcome};

async fn connect(dir: &RunDir, channel: DaemonChannel) -> io::Result<BufReader<Stream>> {
    let metadata = dir.metadata()?;
    let stream = transport::connect(&metadata.endpoint).await?;
    let mut stream = BufReader::new(stream);
    send(
        stream.get_mut(),
        &DaemonHello {
            v: DAEMON_PROTOCOL_VERSION,
            token: metadata.token,
            channel,
        },
    )
    .await?;
    let bytes = frame(&mut stream, 4096)
        .await?
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "missing daemon handshake"))?;
    let welcome: DaemonWelcome =
        serde_json::from_slice(&bytes).map_err(|_| invalid("invalid daemon handshake"))?;
    match welcome {
        DaemonWelcome::Accepted { v } if v == DAEMON_PROTOCOL_VERSION => Ok(stream),
        DaemonWelcome::Version { .. } => Err(invalid(
            "daemon protocol version mismatch; restart or upgrade the daemon",
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "daemon authentication refused",
        )),
    }
}
async fn control(dir: &RunDir, method: &str) -> io::Result<serde_json::Value> {
    let result = async {
        let mut stream = connect(dir, DaemonChannel::Control).await?;
        send(stream.get_mut(), &serde_json::json!({"method":method})).await?;
        let bytes = frame(&mut stream, 4096).await?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "missing daemon response")
        })?;
        serde_json::from_slice(&bytes).map_err(|_| invalid("invalid daemon response"))
    };
    tokio::time::timeout(DEADLINE, result)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "daemon timed out"))?
}
pub fn status(dir: &RunDir) -> io::Result<serde_json::Value> {
    runtime()?.block_on(control(dir, "status"))
}
pub fn stop(dir: &RunDir) -> io::Result<()> {
    runtime()?.block_on(control(dir, "shutdown")).map(|_| ())
}

/// Hosts call this after a disconnect to restart a crashed daemon. Never replays a request.
pub fn ensure_daemon(dir: &RunDir, executable: &Path) -> io::Result<()> {
    runtime()?.block_on(ensure(dir, executable))
}
fn unavailable(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::NotFound
            | io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::UnexpectedEof
    )
}
async fn ensure(dir: &RunDir, executable: &Path) -> io::Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let lock = dir.lock("startup.lock")?;
    loop {
        match control(dir, "status").await {
            Ok(_) => return Ok(()),
            Err(e) if unavailable(&e) => (),
            Err(e) => return Err(e),
        }
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => (),
            Err(std::fs::TryLockError::Error(e)) => return Err(e),
        }
        if tokio::time::Instant::now() > deadline {
            return Err(invalid("daemon startup timed out"));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    // A graceful stop acknowledges before its bounded HTTP drain finishes. Wait for
    // the instance lock instead of launching a child that must immediately fail.
    let instance = dir.lock("daemon.lock")?;
    loop {
        match control(dir, "status").await {
            Ok(_) => return Ok(()),
            Err(e) if unavailable(&e) => (),
            Err(e) => return Err(e),
        }
        match instance.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => (),
            Err(std::fs::TryLockError::Error(e)) => return Err(e),
        }
        if tokio::time::Instant::now() > deadline {
            return Err(invalid("daemon shutdown/startup timed out"));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    drop(instance);
    let mut command = Command::new(executable);
    command
        .arg("--run-dir")
        .arg(dir.path())
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Detached sidecar: no console window, and it outlives the short stdio bridge.
        command.creation_flags(0x00000008 | 0x08000000);
    }
    configure_spawn(&mut command);
    let mut child = command.spawn()?;
    loop {
        match control(dir, "status").await {
            Ok(_) => {
                reap(child);
                return Ok(());
            }
            Err(e) if unavailable(&e) => (),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        }
        if let Some(exit) = child.try_wait()? {
            // A manually-started daemon may have won the process lock after our probe.
            if control(dir, "status").await.is_ok() {
                return Ok(());
            }
            return Err(io::Error::other(format!("daemon failed to start ({exit})")));
        }
        if tokio::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(invalid("daemon startup timed out"));
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
fn reap(mut child: Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

pub fn bridge(dir: &RunDir, executable: &Path) -> io::Result<()> {
    let runtime = runtime()?;
    let result = runtime.block_on(async {
        ensure(dir, executable).await?;
        let stream = tokio::time::timeout(DEADLINE, connect(dir, DaemonChannel::Mcp))
            .await
            .map_err(|_| invalid("daemon handshake timed out"))??;
        let (reader, mut writer) = tokio::io::split(stream);
        let upload = async {
            let mut input = BufReader::new(tokio::io::stdin());
            while let Some(bytes) = frame(&mut input, MAX_FRAME).await? {
                // MCP is newline-delimited JSON; empty lines are not messages.
                if bytes.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                writer.write_all(&bytes).await?;
                writer.flush().await?;
            }
            writer.write_all(b"\n").await?;
            writer.flush().await?;
            Ok::<_, io::Error>(())
        };
        let download = async {
            let mut reader = BufReader::new(reader);
            let mut output = tokio::io::stdout();
            loop {
                let bytes = frame(&mut reader, MAX_FRAME).await?.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::UnexpectedEof, "daemon disconnected")
                })?;
                if bytes == b"\n" {
                    output.flush().await?;
                    return Ok::<_, io::Error>(());
                }
                output.write_all(&bytes).await?;
                output.flush().await?;
            }
        };
        tokio::pin!(upload, download);
        tokio::select! {
            result=&mut download=>result,
            result=&mut upload=> { result?; download.await }
        }
    });
    // Tokio stdin uses an uncancellable blocking read. A crashed daemon must still let
    // this process exit while its MCP client keeps stdin open. Do not await that thread.
    runtime.shutdown_timeout(Duration::from_millis(100));
    result
}

// macOS std creates pipes using pipe + fcntl, so a concurrently spawning host can
// accidentally pass a pipe before CLOEXEC is set. Ensure the long-lived daemon never
// retains that pipe. CLOEXEC preserves std's exec-error pipe until a successful exec.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
fn configure_spawn(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    // SAFETY: sysconf has no pointers. The pre-exec callback uses only async-signal-safe
    // fcntl; no allocation, locks or Rust I/O, and it does not close the exec-error pipe.
    let max = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) }.max(256) as i32;
    unsafe {
        command.pre_exec(move || {
            for fd in 3..max {
                libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            }
            Ok(())
        });
    }
}
#[cfg(all(unix, not(target_os = "macos")))]
fn configure_spawn(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}
#[cfg(windows)]
fn configure_spawn(_: &mut Command) {}
