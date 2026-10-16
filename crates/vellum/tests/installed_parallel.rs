//! TEST DATA. Parallel lanes on real installed engines; opt-in engines lane only (guide Prompt 39 FINAL). Two
//! Claude Code engines answer one request Side by side, each in its own reserved frame, through the orchestrator,
//! the real tool registry and the daemon's documents with the scene (find_space places the areas).
mod common;
use std::{path::PathBuf, sync::Arc, time::Duration};

use vellum::orchestrate::{Area, LaneSpec, Orchestrator, TurnSpec};
use vellum::sessions::{SessionLog, pricing};
use vellum::tools::Registry;
use vellum_contracts::fakes::MemorySessions;
use vellum_contracts::suites::model::block_on;
use vellum_core::parallel::LaneEnd;
use vellum_core::ports::*;
use vellum_core::turns::TurnState;
use vellum_types::{Actor, AgentId, DeviceId, PageId, SessionId, TurnId};

const PATH: &str = "design/app.vellum";

fn run_dir(name: &str) -> PathBuf {
    (if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    })
    .join(format!("vellum-{name}-{}", std::process::id()))
}

#[test]
#[ignore = "engines: needs installed, signed-in Claude Code, local IPC, the scene build and model quota; nightly trusted runner only"]
fn engines_lane_real_claude_side_by_side_builds_one_frame_per_lane() {
    let program = std::env::var_os("VELLUM_CLAUDE_CODE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("claude"));
    let dirs = [run_dir("parallel-a"), run_dir("parallel-b")];
    let engines: Vec<Arc<_>> = dirs
        .iter()
        .map(|dir| {
            Arc::new(vellum::installed_engines::claude(
                program.clone(),
                std::env::current_dir().unwrap(),
                PathBuf::from(env!("CARGO_BIN_EXE_vellum")),
                dir.clone(),
            ))
        })
        .collect();
    for engine in &engines {
        assert_eq!(
            engine.availability(),
            EngineStatus::Ready,
            "engines lane requires sign-in; run claude in your terminal, then re-check"
        );
    }
    let mut daemon = common::Daemon::with_scene(&common::design(), &[PATH]);
    let log = SessionLog::new(Box::new(MemorySessions::new()), pricing());
    let session = SessionId::new("01J00000000000000000000P01").unwrap();
    let teal = AgentId::new("01J00000000000000000000P02").unwrap();
    let saffron = AgentId::new("01J00000000000000000000P03").unwrap();
    log.store()
        .create(NewSession {
            id: session.clone(),
            doc: DocKey::new("/repo", PATH).unwrap(),
            kind: SessionKind::Engine {
                engine: "claude-code".into(),
            },
            agent: teal.clone(),
            title: "TEST DATA: parallel".into(),
            created_at: Timestamp(0),
        })
        .unwrap();
    let turn = TurnId::new("01J00000000000000000000P04").unwrap();
    let spec = TurnSpec {
        session,
        turn: turn.clone(),
        person: Actor::Person {
            device: DeviceId::new("01J00000000000000000000P05").unwrap(),
        },
        document: PATH.into(),
    };
    let registry = Registry::standard();
    let orchestrator = Orchestrator::new(&*engines[0], &registry, &log)
        .with_helpers(vec![&*engines[1]], vec!["Saffron".into()]);
    let mut request = TurnRequest::new(
        turn.clone(),
        "sonnet",
        "TEST DATA: Create exactly one frame with insert_nodes: the reserved frame described below, with its id, \
         page, position and size, named as given. Add nothing inside it. Then reply 'Fixture complete'. Do not \
         read files, run commands or make any other change.",
    );
    request.limits.max_rounds = 6;

    // A run that hangs is interrupted, so the lane fails instead of the runner.
    let watchdog_engines = engines.clone();
    let (done, wait) = std::sync::mpsc::channel::<()>();
    let watchdog = std::thread::spawn(move || {
        if wait.recv_timeout(Duration::from_secs(240)).is_err() {
            for engine in &watchdog_engines {
                engine.interrupt(&turn);
            }
        }
    });
    let outcome = block_on(orchestrator.side_by_side(
        &mut daemon.docs,
        &spec,
        request,
        &[
            LaneSpec {
                agent: teal.clone(),
                name: "Teal".into(),
                model: "sonnet".into(),
            },
            LaneSpec {
                agent: saffron.clone(),
                name: "Saffron".into(),
                model: "sonnet".into(),
            },
        ],
        Area {
            page: PageId::new("pageHome01").unwrap(),
            width: 390.0,
            height: 844.0,
        },
    ))
    .unwrap();
    let _ = done.send(());
    watchdog.join().unwrap();
    drop(orchestrator);

    assert_eq!(outcome.state, TurnState::Done, "{outcome:?}");
    assert!(outcome.lanes.iter().all(|l| l.end == LaneEnd::Done));
    assert_eq!(outcome.options.len(), 2, "one frame per lane: {outcome:?}");
    assert_eq!(outcome.options[0].agent, teal);
    assert_eq!(outcome.options[1].agent, saffron);
    let doc = daemon.docs.target(PATH).unwrap().snapshot().unwrap();
    for option in &outcome.options {
        assert!(vellum_core::ops::find(&doc, &option.frame).is_some());
    }
    for dir in dirs {
        let _ = std::fs::remove_dir_all(dir);
    }
}
