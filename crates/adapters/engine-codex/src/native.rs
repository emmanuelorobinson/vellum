//! Native process boundary: bounded pipes, private environment and owned process trees.
use crate::{Installation, Launch, Launcher, Process};
use serde_json::Value;
use std::{ffi::OsString, path::PathBuf, sync::OnceLock, time::Duration};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc,
};
use vellum_core::ports::*;
#[path = "process_tree.rs"]
mod process_tree;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("installed-engine runtime")
    })
}
/// Only the installed tool's own sign-in/configuration and OS basics are inherited. Vellum keys are never read.
pub fn inherited_environment() -> Vec<(OsString, OsString)> {
    const KEYS: &[&str] = &[
        "PATH",
        "HOME",
        "USER",
        "LOGNAME",
        "TMPDIR",
        "TMP",
        "TEMP",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "SystemRoot",
        "SYSTEMROOT",
        "COMSPEC",
        "PATHEXT",
        "LANG",
        "LC_ALL",
        "CODEX_HOME",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "OPENAI_BASE_URL",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
    ];
    KEYS.iter()
        .filter_map(|k| std::env::var_os(k).map(|v| (OsString::from(k), v)))
        .collect()
}
pub struct NativeLauncher {
    program: PathBuf,
    cwd: PathBuf,
}
impl NativeLauncher {
    pub fn new(program: PathBuf, cwd: PathBuf) -> Self {
        Self { program, cwd }
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.current_dir(&self.cwd)
            .env_clear()
            .envs(inherited_environment())
            .env("DO_NOT_TRACK", "1")
            .env("OTEL_SDK_DISABLED", "true")
            .env("TURBO_TELEMETRY_DISABLED", "1")
            .env("NEXT_TELEMETRY_DISABLED", "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        process_tree::configure(&mut cmd);
        cmd
    }
    fn probe(&self, args: &[&str]) -> Result<(bool, String), std::io::ErrorKind> {
        let mut cmd = self.command();
        cmd.args(args).stdin(std::process::Stdio::null());
        let auth = args == ["login", "status"];
        if auth {
            cmd.stderr(std::process::Stdio::piped());
        }
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        runtime().spawn(async move {
            let result = async {
                let mut child = cmd.spawn().map_err(|e| e.kind())?;
                let _tree =
                    process_tree::Tree::attach(child.id().ok_or(std::io::ErrorKind::Other)?)
                        .map_err(|e| e.kind())?;
                let mut bytes = vec![];
                let pipe: Box<dyn tokio::io::AsyncRead + Send + Unpin> = if auth {
                    Box::new(child.stderr.take().ok_or(std::io::ErrorKind::Other)?)
                } else {
                    Box::new(child.stdout.take().ok_or(std::io::ErrorKind::Other)?)
                };
                pipe.take(128 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .await
                    .map_err(|e| e.kind())?;
                if bytes.len() > 128 * 1024 {
                    return Err(std::io::ErrorKind::InvalidData);
                }
                let status = child.wait().await.map_err(|e| e.kind())?;
                let text = String::from_utf8(bytes).map_err(|_| std::io::ErrorKind::InvalidData)?;
                Ok((status.success(), text))
            };
            let result = tokio::time::timeout(Duration::from_secs(5), result)
                .await
                .unwrap_or(Err(std::io::ErrorKind::TimedOut));
            let _ = tx.send(result);
        });
        rx.recv_timeout(Duration::from_secs(6))
            .unwrap_or(Err(std::io::ErrorKind::TimedOut))
    }
}
impl Launcher for NativeLauncher {
    fn inspect(&self) -> Installation {
        let version = match self.probe(&["--version"]) {
            Ok((true, v)) => v,
            Err(std::io::ErrorKind::NotFound) => {
                return Installation {
                    version: None,
                    status: EngineStatus::Missing,
                    mode: None,
                };
            }
            _ => {
                return Installation {
                    version: None,
                    status: EngineStatus::Failed,
                    mode: None,
                };
            }
        };
        let root = self.probe(&["--help"]);
        let exec = self.probe(&["exec", "--help"]);
        let auth = self.probe(&["login", "status"]);
        let app = if root
            .as_ref()
            .is_ok_and(|(ok, h)| *ok && h.contains("app-server"))
        {
            self.probe(&["app-server", "--help"])
                .ok()
                .filter(|(ok, _)| *ok)
                .map(|(_, h)| h)
        } else {
            None
        };
        match (root, exec, auth) {
            (Ok((true, root)), Ok((true, exec)), Ok((ok, auth))) => {
                Installation::from_probe(&version, &root, app.as_deref(), &exec, ok, &auth)
            }
            _ => Installation {
                version: None,
                status: EngineStatus::Failed,
                mode: None,
            },
        }
    }
    fn start(&self, launch: Launch, cancel: Cancellation) -> Result<Box<dyn Process>, EngineError> {
        let mut cmd = self.command();
        cmd.args(launch.args);
        let input = launch.input;
        if input.as_ref().is_some_and(|s| s.len() > 8 * 1024 * 1024) {
            return Err(EngineError::InvalidRequest);
        }
        let (outgoing, mut incoming) = mpsc::channel::<Value>(32);
        let (tx, rx) = mpsc::channel(32);
        let worker_cancel = cancel.clone();
        runtime().spawn(async move {
            let result = async {
                if worker_cancel.is_cancelled() {
                    return Ok(());
                }
                let mut child = cmd.spawn().map_err(|_| EngineError::Unavailable)?;
                let _tree = process_tree::Tree::attach(child.id().ok_or(EngineError::Unavailable)?)
                    .map_err(|_| EngineError::Unavailable)?;
                let mut stdin = child.stdin.take().ok_or(EngineError::Unavailable)?;
                let writer = async move {
                    if let Some(input) = input {
                        stdin
                            .write_all(input.as_bytes())
                            .await
                            .map_err(|_| EngineError::Unavailable)?;
                        stdin
                            .shutdown()
                            .await
                            .map_err(|_| EngineError::Unavailable)?;
                        drop(stdin);
                        std::future::pending::<Result<(), EngineError>>().await
                    } else {
                        while let Some(value) = incoming.recv().await {
                            let mut bytes = serde_json::to_vec(&value)
                                .map_err(|_| EngineError::InvalidRequest)?;
                            if bytes.len() > 8 * 1024 * 1024 {
                                return Err(EngineError::InvalidRequest);
                            }
                            bytes.push(b'\n');
                            stdin
                                .write_all(&bytes)
                                .await
                                .map_err(|_| EngineError::Unavailable)?;
                        }
                        Ok(())
                    }
                };
                let mut reader =
                    BufReader::new(child.stdout.take().ok_or(EngineError::Unavailable)?);
                let reader = async {
                    while let Some(line) = record(&mut reader).await? {
                        if tx.send(Ok(line)).await.is_err() {
                            return Ok(());
                        }
                    }
                    let status = child.wait().await.map_err(|_| EngineError::Unavailable)?;
                    if !status.success() {
                        return Err(EngineError::Model(ModelError::Unavailable));
                    }
                    Ok(())
                };
                tokio::select! {r=reader=>r,r=writer=>r}
            };
            tokio::select! {
                biased;
                _=std::future::poll_fn(|cx|worker_cancel.poll_cancelled(cx))=>{},
                result=result=>{if let Err(error)=result {let _=tx.send(Err(error)).await;}},
            }
        });
        Ok(Box::new(NativeProcess {
            rx,
            cancel,
            outgoing,
        }))
    }
}
struct NativeProcess {
    rx: mpsc::Receiver<Result<String, EngineError>>,
    cancel: Cancellation,
    outgoing: mpsc::Sender<Value>,
}
impl Process for NativeProcess {
    fn send(&mut self, value: Value) -> EngineFuture<'_, Result<(), EngineError>> {
        let result = self
            .outgoing
            .try_send(value)
            .map_err(|_| EngineError::Unavailable);
        Box::pin(async move { result })
    }
    fn next(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>> {
        Box::pin(async { self.rx.recv().await.transpose() })
    }
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
async fn record(reader: &mut (impl AsyncBufRead + Unpin)) -> Result<Option<String>, EngineError> {
    let mut bytes = vec![];
    loop {
        let buf = reader
            .fill_buf()
            .await
            .map_err(|_| EngineError::Unavailable)?;
        if buf.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                Err(EngineError::Model(ModelError::Truncated))
            };
        }
        let end = buf.iter().position(|b| *b == b'\n').map(|n| n + 1);
        let count = end.unwrap_or(buf.len());
        if bytes.len() + count > 4 * 1024 * 1024 {
            return Err(EngineError::Model(ModelError::Limit));
        }
        bytes.extend_from_slice(&buf[..count]);
        reader.consume(count);
        if end.is_some() {
            return String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| EngineError::Model(ModelError::Protocol));
        }
    }
}
