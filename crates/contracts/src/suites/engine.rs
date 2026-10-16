//! The AgentEngine contract: tool calls go through the bridge, a turn ends exactly once, and Stop ends a turn
//! within 3 s (Invariant 12) with no tool call after it.
use super::model::block_on;
use crate::fakes::RecordingBridge;
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use vellum_core::ports::*;
use vellum_types::TurnId;

/// What the engine under test must be built to do on its first turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// Call the bridge's `probe` tool once with `{"value":1}`, then finish.
    CallProbe,
    /// Keep working (never finish on its own) until it is interrupted.
    WorkUntilStopped,
}

/// Interrupt must end a turn within this (Invariant 12, engine side).
pub const STOP_WITHIN: Duration = Duration::from_secs(3);

fn turn_id(n: u8) -> TurnId {
    TurnId::new(format!("01J00000000000000000000T{n:02}")).expect("a ULID-shaped id")
}

fn drain(stream: &mut dyn TurnStream) -> Vec<TurnEvent> {
    let mut events = vec![];
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
        assert!(events.len() < 10_000, "AgentEngine: turn never ended");
    }
    events
}

fn ended_once(events: &[TurnEvent]) -> TurnEnd {
    let ends: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            TurnEvent::Ended(end) => Some(*end),
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 1, "AgentEngine: a turn ends exactly once");
    assert!(
        matches!(events.last(), Some(TurnEvent::Ended(_))),
        "AgentEngine: Ended is the last event"
    );
    ends[0]
}

pub fn agent_engine(make: &dyn Fn(Scenario) -> Arc<dyn AgentEngine>) {
    // Tool calls go through the bridge and are reported.
    let engine = make(Scenario::CallProbe);
    assert!(!engine.id().is_empty(), "AgentEngine: missing identity");
    assert_eq!(engine.availability(), EngineStatus::Ready);
    let bridge = RecordingBridge::probe();
    let mut stream = engine
        .run_turn(TurnRequest::new(turn_id(1), "test", "probe once"), &bridge)
        .expect("AgentEngine: run_turn");
    let events = drain(&mut *stream);
    assert!(
        block_on(stream.next()).is_none(),
        "AgentEngine: stream is not fused"
    );
    drop(stream);
    assert_eq!(
        ended_once(&events),
        TurnEnd::Done,
        "AgentEngine: {events:?}"
    );
    assert_eq!(
        bridge.calls(),
        vec![("probe".to_owned(), json!({"value": 1}))],
        "AgentEngine: tool calls must go through the bridge, once each"
    );
    let started = events
        .iter()
        .position(|e| matches!(e, TurnEvent::ToolStarted { name, .. } if name == "probe"));
    let finished = events.iter().position(
        |e| matches!(e, TurnEvent::ToolFinished { name, failed: false, .. } if name == "probe"),
    );
    assert!(
        matches!((started, finished), (Some(s), Some(f)) if s < f),
        "AgentEngine: a tool call is reported started, then finished"
    );
    assert!(
        events.iter().any(|e| matches!(e, TurnEvent::Messages(_))),
        "AgentEngine: the turn's messages are handed back"
    );

    // Unknown turns are ignored.
    make(Scenario::CallProbe).interrupt(&turn_id(9));

    // Stop from another thread ends a working turn quickly, with nothing after it.
    let engine = make(Scenario::WorkUntilStopped);
    let bridge = RecordingBridge::probe();
    let turn = turn_id(2);
    let mut stream = engine
        .run_turn(
            TurnRequest::new(turn.clone(), "test", "keep going"),
            &bridge,
        )
        .expect("AgentEngine: run_turn");
    let stopper = {
        let engine = engine.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let at = Instant::now();
            engine.interrupt(&turn);
            at
        })
    };
    let events = drain(&mut *stream);
    let ended = Instant::now();
    let asked = stopper.join().expect("the stopper thread");
    assert_eq!(
        ended_once(&events),
        TurnEnd::Stopped,
        "AgentEngine: {events:?}"
    );
    assert!(
        ended.duration_since(asked) < STOP_WITHIN,
        "AgentEngine: Stop took {:?}",
        ended.duration_since(asked)
    );
    let calls = bridge.calls().len();
    drop(stream);
    assert_eq!(bridge.calls().len(), calls, "AgentEngine: work after Stop");

    // A turn stopped before it first runs makes no call at all.
    let engine = make(Scenario::CallProbe);
    let bridge = RecordingBridge::probe();
    let turn = turn_id(3);
    let mut stream = engine
        .run_turn(
            TurnRequest::new(turn.clone(), "test", "probe once"),
            &bridge,
        )
        .expect("AgentEngine: run_turn");
    engine.interrupt(&turn);
    let events = drain(&mut *stream);
    assert_eq!(ended_once(&events), TurnEnd::Stopped);
    assert!(
        bridge.calls().is_empty(),
        "AgentEngine: called a tool after Stop"
    );
}
