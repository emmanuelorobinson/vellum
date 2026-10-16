//! TEST DATA. A local MCP relay and deterministic process; no model, installed engine or network.
mod common;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use vellum::{
    installed_engines::LocalMcpHost,
    tools::{Registry, RegistryBridge, Session},
};
use vellum_contracts::suites::model::block_on;
use vellum_core::ports::*;
use vellum_engine_claude_code::{ClaudeEngine, Installation, Launch, Launcher, Process};

struct Client {
    binary: PathBuf,
}
impl Launcher for Client {
    fn inspect(&self) -> Installation {
        Installation {
            version: Some("2.1.301".into()),
            status: EngineStatus::Ready,
        }
    }
    fn start(&self, launch: Launch, _: Cancellation) -> Result<Box<dyn Process>, EngineError> {
        let config: Value = serde_json::from_str(
            &launch.args[launch
                .args
                .iter()
                .position(|a| a == "--mcp-config")
                .unwrap()
                + 1],
        )
        .unwrap();
        let args: Vec<String> = config["mcpServers"]["vellum"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect();
        let binary = self.binary.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            let mut child = std::process::Command::new(binary)
                .args(args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let mut input = child.stdin.take().unwrap();
            let mut out = BufReader::new(child.stdout.take().unwrap());
            for msg in [
                json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"TEST DATA"}}}),
                json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_app_state","arguments":{}}}),
            ] {
                writeln!(input, "{msg}").unwrap();
                input.flush().unwrap();
                let mut line = String::new();
                out.read_line(&mut line).unwrap();
                let reply: Value = serde_json::from_str(&line).unwrap();
                assert!(reply.get("error").is_none(), "{reply}");
            }
            drop(input);
            let _ = child.wait();
            let _ = tx.send(());
        });
        Ok(Box::new(Wait {
            rx,
            at: Instant::now(),
        }))
    }
}
struct Wait {
    rx: std::sync::mpsc::Receiver<()>,
    at: Instant,
}
impl Process for Wait {
    fn next(&mut self) -> EngineFuture<'_, Result<Option<String>, EngineError>> {
        Box::pin(std::future::poll_fn(|cx| {
            if self.rx.try_recv().is_ok() {
                std::task::Poll::Ready(Ok(Some(
                    r#"{"type":"result","subtype":"success","is_error":false,"result":"Done"}"#
                        .into(),
                )))
            } else if self.at.elapsed() > Duration::from_secs(10) {
                std::task::Poll::Ready(Err(EngineError::Unavailable))
            } else {
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }))
    }
}
#[test]
#[ignore = "ipc: starts the real vellum mcp relay over a private local socket/pipe"]
fn turn_mcp_relay_calls_the_same_registry_bridge_and_cleans_up() {
    let base = (if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    })
    .join(format!("vellum-engine-host-{}", std::process::id()));
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_vellum"));
    let host = Arc::new(LocalMcpHost::new(binary.clone(), base.clone()));
    let engine = ClaudeEngine::new(Arc::new(Client { binary }), host);
    let mut daemon = common::Daemon::new(&common::design(), &["design/app.vellum"]);
    let docs = &mut daemon.docs;
    let mut session = Session::default();
    let turn = vellum_types::TurnId::new("01J00000000000000000000T01").unwrap();
    session.set_turn(Some(turn.clone()));
    let registry = Registry::standard();
    let bridge = RegistryBridge::new(&registry, docs, &mut session);
    let mut stream = engine
        .run_turn(
            TurnRequest::new(turn, "sonnet", "TEST DATA inspect"),
            &bridge,
        )
        .unwrap();
    let mut events = vec![];
    while let Some(e) = block_on(stream.next()) {
        events.push(e);
    }
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    drop(stream);
    assert_eq!(session.steps().len(), 1);
    assert_eq!(session.steps()[0].tool, "get_app_state");
    assert!(std::fs::read_dir(&base).unwrap().next().is_none());
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
#[ignore = "engines: needs installed, signed-in Claude Code, local IPC and model quota; nightly trusted runner only"]
fn engines_lane_real_claude_calls_mcp_and_reports_usage() {
    let base = (if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    })
    .join(format!("vellum-live-{}", std::process::id()));
    let program = std::env::var_os("VELLUM_CLAUDE_CODE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("claude"));
    let engine = Arc::new(vellum::installed_engines::claude(
        program,
        std::env::current_dir().unwrap(),
        PathBuf::from(env!("CARGO_BIN_EXE_vellum")),
        base.clone(),
    ));
    assert_eq!(
        engine.availability(),
        EngineStatus::Ready,
        "engines lane requires sign-in; run claude in your terminal, then re-check"
    );
    let mut daemon = common::Daemon::new(&common::design(), &["design/app.vellum"]);
    let mut session = Session::default();
    let turn = vellum_types::TurnId::new("01J00000000000000000000E01").unwrap();
    session.set_turn(Some(turn.clone()));
    let registry = Registry::standard();
    let bridge = RegistryBridge::new(&registry, &mut daemon.docs, &mut session);
    let mut request = TurnRequest::new(
        turn.clone(),
        "sonnet",
        "TEST DATA: Call get_app_state once via the vellum MCP server. Then reply 'Fixture complete'. Do not read files, run commands or make any changes.",
    );
    request.limits.max_rounds = 3;
    let mut stream = engine.run_turn(request, &bridge).unwrap();
    let stop = engine.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if rx.recv_timeout(Duration::from_secs(90)).is_err() {
            stop.interrupt(&turn);
        }
    });
    let mut events = vec![];
    while let Some(e) = block_on(stream.next()) {
        events.push(e);
    }
    let _ = tx.send(());
    watchdog.join().unwrap();
    assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
    assert!(events.iter().any(
        |e| matches!(e,TurnEvent::Usage(u) if u.input_tokens.is_some() && u.output_tokens.is_some())
    ));
    assert!(events.iter().any(
        |e| matches!(e,TurnEvent::ToolFinished {name,failed:false,..} if name=="get_app_state")
    ));
    drop(stream);
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
#[ignore = "engines: needs installed, signed-in Claude Code, local IPC and model quota; nightly trusted runner only"]
fn engines_lane_real_claude_stop_interrupts_a_working_turn() {
    let base = (if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    })
    .join(format!("vellum-stop-{}", std::process::id()));
    let program = std::env::var_os("VELLUM_CLAUDE_CODE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("claude"));
    let engine = Arc::new(vellum::installed_engines::claude(
        program,
        std::env::current_dir().unwrap(),
        PathBuf::from(env!("CARGO_BIN_EXE_vellum")),
        base.clone(),
    ));
    assert_eq!(
        engine.availability(),
        EngineStatus::Ready,
        "engines lane requires sign-in"
    );
    let bridge = vellum_contracts::fakes::RecordingBridge::probe();
    let turn = vellum_types::TurnId::new("01J00000000000000000000E02").unwrap();
    let mut stream = engine
        .run_turn(
            TurnRequest::new(
                turn.clone(),
                "sonnet",
                "TEST DATA: Explain design systems in 200 paragraphs. Do not use tools.",
            ),
            &bridge,
        )
        .unwrap();
    let stop = engine.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        let at = Instant::now();
        stop.interrupt(&turn);
        at
    });
    let mut end = None;
    while let Some(e) = block_on(stream.next()) {
        if let TurnEvent::Ended(e) = e {
            end = Some(e);
        }
    }
    let at = stopper.join().unwrap();
    assert_eq!(end, Some(TurnEnd::Stopped));
    assert!(at.elapsed() < Duration::from_secs(3));
    drop(stream);
    std::fs::remove_dir_all(base).unwrap();
}
