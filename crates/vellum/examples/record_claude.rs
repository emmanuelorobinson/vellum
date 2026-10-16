//! TEST DATA. Opt-in engines lane recorder; uses the real installed version and a fixed MCP-only prompt.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use vellum::{
    installed_engines::LocalMcpHost,
    tools::{Client, Registry, RegistryBridge, Session},
};
use vellum_contracts::suites::model::block_on;
use vellum_core::ports::*;
use vellum_engine_claude_code::{
    ClaudeEngine, Installation, Launch, Launcher, NativeLauncher, Process, recording,
};
struct Recorder {
    native: NativeLauncher,
    installation: Installation,
    negative: bool,
    lines: Arc<Mutex<Vec<Value>>>,
}
impl Launcher for Recorder {
    fn inspect(&self) -> Installation {
        let mut i = self.installation.clone();
        if self.negative && i.status == EngineStatus::SignedOut {
            i.status = EngineStatus::Ready;
        }
        i
    }
    fn start(&self, launch: Launch, cancel: Cancellation) -> Result<Box<dyn Process>, EngineError> {
        Ok(Box::new(Recording {
            inner: self.native.start(launch, cancel)?,
            lines: self.lines.clone(),
        }))
    }
}
struct Recording {
    inner: Box<dyn Process>,
    lines: Arc<Mutex<Vec<Value>>>,
}
impl Process for Recording {
    fn next(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>> {
        Box::pin(async {
            let next = self.inner.next().await?;
            if let Some(line) = &next {
                let value: Value = serde_json::from_str(line)
                    .map_err(|_| EngineError::Model(ModelError::Protocol))?;
                if let Some(value) = recording::record(&value) {
                    let mut lines = self.lines.lock().unwrap();
                    if lines.len() >= 10000 {
                        return Err(EngineError::RoundLimit);
                    }
                    lines.push(value);
                }
            }
            Ok(next)
        })
    }
}
struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let option = |name: &str, default: &str| {
        args.windows(2)
            .find(|p| p[0] == name)
            .map_or_else(|| default.to_owned(), |p| p[1].clone())
    };
    let negative = option("--case", "success") == "signed-out";
    let program = PathBuf::from(option("--claude", "claude"));
    let repo = std::env::current_dir()?;
    let native = NativeLauncher::new(program, repo.clone());
    let installation = native.inspect();
    let version = installation
        .version
        .clone()
        .ok_or("Claude Code not installed or unsupported")?;
    if (!negative && installation.status != EngineStatus::Ready)
        || (negative && installation.status != EngineStatus::SignedOut)
    {
        return Err(format!(
            "Claude Code preflight: {:?}; sign in in your terminal for the success case",
            installation.status
        )
        .into());
    }
    let home = if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    };
    let runtime = home.join(format!("vellum-rec-{}", std::process::id()));
    let _cleanup = Scratch(runtime.clone());
    let binary = PathBuf::from(option("--vellum", "target/debug/vellum"));
    let binary = std::fs::canonicalize(binary)?;
    let mut docs = vellum::daemon::production::documents(&runtime)?;
    let mut session = Session::in_dir(repo);
    session.client = Some(Client {
        name: "Claude Code".into(),
        version: version.clone(),
    });
    let turn = vellum_types::TurnId::new("01J00000000000000000000REC")?;
    session.set_turn(Some(turn.clone()));
    let registry = Registry::standard();
    let bridge = RegistryBridge::new(&registry, &mut docs, &mut session);
    let lines = Arc::new(Mutex::new(vec![]));
    let engine = Arc::new(ClaudeEngine::new(
        Arc::new(Recorder {
            native,
            installation,
            negative,
            lines: lines.clone(),
        }),
        Arc::new(LocalMcpHost::new(binary, runtime.join("run"))),
    ));
    let mut request = TurnRequest::new(
        turn.clone(),
        option("--model", "sonnet"),
        "TEST DATA: Call get_app_state exactly once using the attached vellum MCP server, then reply 'Fixture complete'. Do not read files, run commands or make any changes.",
    );
    request.limits.max_rounds = 3;
    let mut stream = engine.run_turn(request, &bridge)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = engine.clone();
    let watchdog = std::thread::spawn(move || {
        if rx.recv_timeout(Duration::from_secs(90)).is_err() {
            stop.interrupt(&turn);
        }
    });
    let mut end = None;
    let mut calls = 0;
    while let Some(event) = block_on(stream.next()) {
        match event {
            TurnEvent::Ended(e) => end = Some(e),
            TurnEvent::ToolFinished { failed: false, .. } => calls += 1,
            _ => {}
        }
    }
    let _ = tx.send(());
    watchdog.join().map_err(|_| "recorder watchdog failed")?;
    drop(stream);
    let expected = if negative {
        end == Some(TurnEnd::Failed(EngineError::Model(
            ModelError::Authentication,
        )))
    } else {
        end == Some(TurnEnd::Done) && calls > 0
    };
    if !expected {
        return Err(format!(
            "fixture did not satisfy its case: {end:?}, {calls} successful MCP calls"
        )
        .into());
    }
    let out = PathBuf::from(option("--out", "fixtures/engines/claude-code")).join(&version);
    std::fs::create_dir_all(&out)?;
    let case = if negative { "signed-out" } else { "success" };
    let text = lines
        .lock()
        .unwrap()
        .iter()
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    std::fs::write(out.join(format!("{case}.recorded.ndjson")), text)?;
    std::fs::write(
        out.join(format!("{case}.recorded.json")),
        format!(
            "{}\n",
            serde_json::to_string_pretty(
                &json!({"testData":true,"kind":"recorded","version":version,"case":case,"projection":"decoder fields only; no raw diagnostics/auth/init metadata","mcpCalls":calls})
            )?
        ),
    )?;
    drop(docs);
    std::fs::remove_dir_all(runtime)?;
    println!("Recorded Claude Code {version} {case}: {calls} successful MCP calls");
    Ok(())
}
