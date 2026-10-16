//! TEST DATA. Native process lane: Python 3 fixture, no installed engine/account/model/network.
#![cfg(unix)]
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use vellum_contracts::{fakes::RecordingBridge, suites::model::block_on};
use vellum_core::ports::*;
use vellum_engine_claude_code::*;
struct Host;
impl McpHost for Host {
    fn open(&self, _: &[BridgeTool]) -> Result<Box<dyn McpChannel>, EngineError> {
        Ok(Box::new(Host))
    }
}
impl McpChannel for Host {
    fn config(&self) -> Value {
        json!({"mcpServers":{}})
    }
    fn poll_call(&mut self, _: &mut std::task::Context<'_>) -> std::task::Poll<Option<McpCall>> {
        std::task::Poll::Pending
    }
}
fn engine(dir: &std::path::Path) -> ClaudeEngine {
    ClaudeEngine::new(
        Arc::new(NativeLauncher::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../fixtures/engines/fake-claude.py"),
            dir.into(),
        )),
        Arc::new(Host),
    )
}
fn request(prompt: &str) -> TurnRequest {
    TurnRequest::new(
        vellum_types::TurnId::new("01J00000000000000000000T01").unwrap(),
        "fixture",
        prompt,
    )
}
#[test]
#[ignore = "process: needs Python 3 and permission to launch fixture child processes"]
fn native_inspection_completion_errors_and_drop_are_bounded() {
    let dir = std::env::temp_dir();
    let engine = engine(&dir);
    assert_eq!(engine.availability(), EngineStatus::Ready);
    for prompt in [
        "TEST DATA done",
        "TEST DATA truncated",
        "TEST DATA large",
        "TEST DATA stderr",
    ] {
        let bridge = RecordingBridge::probe();
        let mut stream = engine.run_turn(request(prompt), &bridge).unwrap();
        let mut events = vec![];
        while let Some(e) = block_on(stream.next()) {
            events.push(e);
        }
        if prompt == "TEST DATA done" {
            assert_eq!(events.last(), Some(&TurnEvent::Ended(TurnEnd::Done)));
        } else {
            assert!(matches!(
                events.last(),
                Some(TurnEvent::Ended(TurnEnd::Failed(_)))
            ));
        }
        assert!(!format!("{events:?}").contains("canary-secret"));
    }
    let missing = NativeLauncher::new(dir.join("no-such-vellum-claude"), dir.clone());
    assert_eq!(missing.inspect().status, EngineStatus::Missing);
}
#[test]
#[ignore = "process: needs Python 3 and Unix process groups"]
fn stop_and_drop_kill_the_owned_process_tree_inside_three_seconds() {
    let dir = std::env::temp_dir().join(format!("vellum-claude-tree-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for stop in [true, false] {
        let engine = engine(&dir);
        let bridge = RecordingBridge::probe();
        let req = request("TEST DATA slow tree");
        let id = req.turn.clone();
        let mut stream = engine.run_turn(req, &bridge).unwrap();
        assert!(matches!(block_on(stream.next()), Some(TurnEvent::Text(_))));
        let child = std::fs::read_to_string(dir.join("child.pid")).unwrap();
        let now = Instant::now();
        if stop {
            engine.interrupt(&id);
            while let Some(e) = block_on(stream.next()) {
                if let TurnEvent::Ended(end) = e {
                    assert_eq!(end, TurnEnd::Stopped);
                }
            }
        }
        drop(stream);
        loop {
            let output = std::process::Command::new("ps")
                .args(["-o", "stat=", "-p", child.trim()])
                .output()
                .unwrap();
            let state = String::from_utf8_lossy(&output.stdout);
            if state.trim().is_empty() || state.trim().starts_with('Z') {
                break;
            }
            assert!(
                now.elapsed() < Duration::from_secs(3),
                "descendant survived cancellation"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
