//! Native process boundary: bounded pipes, private environment and owned process trees.
use crate::{Installation, Launch, Launcher, Process};
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
        "CLAUDE_CONFIG_DIR",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "ANTHROPIC_BASE_URL",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
        "AWS_PROFILE",
        "AWS_REGION",
        "AWS_DEFAULT_REGION",
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "AWS_SESSION_TOKEN",
        "AWS_CONFIG_FILE",
        "AWS_SHARED_CREDENTIALS_FILE",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "ANTHROPIC_VERTEX_PROJECT_ID",
        "CLOUD_ML_REGION",
        "ANTHROPIC_FOUNDRY_RESOURCE",
        "ANTHROPIC_FOUNDRY_API_KEY",
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
            .env("DISABLE_TELEMETRY", "1")
            .env("DISABLE_ERROR_REPORTING", "1")
            .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
            .env("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1")
            .env("CLAUDE_CODE_ENABLE_TELEMETRY", "0")
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
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        runtime().spawn(async move {
            let result = async {
                let mut child = cmd.spawn().map_err(|e| e.kind())?;
                let _tree =
                    process_tree::Tree::attach(child.id().ok_or(std::io::ErrorKind::Other)?)
                        .map_err(|e| e.kind())?;
                let mut bytes = vec![];
                child
                    .stdout
                    .take()
                    .ok_or(std::io::ErrorKind::Other)?
                    .take(128 * 1024 + 1)
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
                };
            }
            _ => {
                return Installation {
                    version: None,
                    status: EngineStatus::Failed,
                };
            }
        };
        let help = self.probe(&["--help"]);
        let auth = self.probe(&["auth", "status", "--json"]);
        match (help, auth) {
            (Ok((true, h)), Ok((_, a))) => Installation::from_probe(&version, &h, &a),
            _ => Installation {
                version: Some(version.split_whitespace().next().unwrap_or("").into()),
                status: EngineStatus::Failed,
            },
        }
    }
    fn start(&self, launch: Launch, cancel: Cancellation) -> Result<Box<dyn Process>, EngineError> {
        let mut cmd = self.command();
        cmd.args(launch.args).env(
            "CLAUDE_CODE_MAX_OUTPUT_TOKENS",
            launch.max_output_tokens.to_string(),
        );
        let mut input =
            serde_json::to_vec(&launch.input).map_err(|_| EngineError::InvalidRequest)?;
        input.push(b'\n');
        if input.len() > 8 * 1024 * 1024 {
            return Err(EngineError::InvalidRequest);
        }
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
                stdin
                    .write_all(&input)
                    .await
                    .map_err(|_| EngineError::Unavailable)?;
                stdin
                    .shutdown()
                    .await
                    .map_err(|_| EngineError::Unavailable)?;
                drop(stdin);
                let mut reader =
                    BufReader::new(child.stdout.take().ok_or(EngineError::Unavailable)?);
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
            tokio::select! {
                biased;
                _=std::future::poll_fn(|cx|worker_cancel.poll_cancelled(cx))=>{},
                result=result=>{if let Err(error)=result {let _=tx.send(Err(error)).await;}},
            }
        });
        Ok(Box::new(NativeProcess { rx, cancel }))
    }
}
struct NativeProcess {
    rx: mpsc::Receiver<Result<String, EngineError>>,
    cancel: Cancellation,
}
impl Process for NativeProcess {
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
