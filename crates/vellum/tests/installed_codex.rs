//! TEST DATA. Real installed Codex acceptance; opt-in engines lane only.
mod common;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use vellum::tools::{Registry, RegistryBridge, Session};
use vellum_contracts::suites::model::block_on;
use vellum_core::ports::*;
#[test]
#[ignore = "engines: needs installed, signed-in Codex, local IPC and model quota; nightly trusted runner only"]
fn engines_lane_real_codex_calls_canvas_and_reports_usage() {
    let base = (if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    })
    .join(format!("vellum-codex-live-{}", std::process::id()));
    vellum_ipc_local::RunDir::new(base.clone()).unwrap();
    let program = std::env::var_os("VELLUM_CODEX")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let engine = Arc::new(vellum::installed_engines::codex(
        program,
        std::env::current_dir().unwrap(),
        PathBuf::from(env!("CARGO_BIN_EXE_vellum")),
        base.clone(),
    ));
    assert_eq!(
        engine.availability(),
        EngineStatus::Ready,
        "engines lane requires sign-in; run codex in your terminal, then re-check"
    );
    let mut daemon = common::Daemon::new(&common::design(), &["design/app.vellum"]);
    let mut session = Session::default();
    let turn = vellum_types::TurnId::new("01J00000000000000000000E01").unwrap();
    session.set_turn(Some(turn.clone()));
    let registry = Registry::standard();
    let bridge = RegistryBridge::new(&registry, &mut daemon.docs, &mut session);
    let mut request = TurnRequest::new(
        turn.clone(),
        "gpt-6-luna",
        "TEST DATA: Call get_app_state once using the attached canvas tool. Then reply 'Fixture complete'. Do not read files, run commands or make any changes.",
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
#[ignore = "engines: needs installed, signed-in Codex, local IPC and model quota; nightly trusted runner only"]
fn engines_lane_real_codex_stop_interrupts_a_working_turn() {
    let base = (if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    })
    .join(format!("vellum-codex-stop-{}", std::process::id()));
    vellum_ipc_local::RunDir::new(base.clone()).unwrap();
    let program = std::env::var_os("VELLUM_CODEX")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let engine = Arc::new(vellum::installed_engines::codex(
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
                "gpt-6-luna",
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

struct ExecOnly(vellum_engine_codex::NativeLauncher);
impl vellum_engine_codex::Launcher for ExecOnly {
    fn inspect(&self) -> vellum_engine_codex::Installation {
        let mut i = self.0.inspect();
        i.mode = Some(vellum_engine_codex::Mode::Exec);
        i
    }
    fn start(
        &self,
        l: vellum_engine_codex::Launch,
        c: Cancellation,
    ) -> Result<Box<dyn vellum_engine_codex::Process>, EngineError> {
        self.0.start(l, c)
    }
}
#[test]
#[ignore = "engines: needs installed, signed-in Codex, local IPC and model quota; trusted runner only"]
fn engines_lane_real_codex_exec_stop_interrupts_the_fallback() {
    let base = (if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    })
    .join(format!("vellum-codex-exec-stop-{}", std::process::id()));
    vellum_ipc_local::RunDir::new(base.clone()).unwrap();
    let program = std::env::var_os("VELLUM_CODEX")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let engine = Arc::new(vellum_engine_codex::CodexEngine::new(
        Arc::new(ExecOnly(vellum_engine_codex::NativeLauncher::new(
            program,
            std::env::current_dir().unwrap(),
        ))),
        Arc::new(vellum::installed_engines::LocalMcpHost::new(
            PathBuf::from(env!("CARGO_BIN_EXE_vellum")),
            base.clone(),
        )),
    ));
    assert_eq!(engine.availability(), EngineStatus::Ready);
    let bridge = vellum_contracts::fakes::RecordingBridge::probe();
    let turn = vellum_types::TurnId::new("01J00000000000000000000E03").unwrap();
    let mut stream = engine
        .run_turn(
            TurnRequest::new(
                turn.clone(),
                "gpt-6-luna",
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
    assert_eq!(end, Some(TurnEnd::Stopped));
    assert!(stopper.join().unwrap().elapsed() < Duration::from_secs(3));
    drop(stream);
    assert!(std::fs::read_dir(&base).unwrap().next().is_none());
    std::fs::remove_dir_all(base).unwrap();
}
