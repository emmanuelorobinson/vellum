//! TEST DATA: scripted engines and canned tools.
use serde_json::json;
use std::sync::Arc;
use vellum_contracts::{
    fakes::{FakeEngine, FakeStep, RecordingBridge},
    suites::{self, engine::Scenario, model::block_on},
};
use vellum_core::ports::*;
use vellum_types::TurnId;

fn probe() -> FakeStep {
    FakeStep::Call {
        name: "probe".into(),
        input: json!({"value": 1}),
    }
}

fn honest(scenario: Scenario) -> Arc<dyn AgentEngine> {
    Arc::new(FakeEngine::new(vec![match scenario {
        Scenario::CallProbe => vec![probe()],
        Scenario::WorkUntilStopped => vec![probe(), FakeStep::Work],
    }]))
}

#[test]
fn fake_engine_obeys_the_contract() {
    suites::engine::agent_engine(&honest);
}

fn catches(make: fn(Scenario) -> Arc<dyn AgentEngine>) -> bool {
    std::panic::catch_unwind(|| suites::engine::agent_engine(&make)).is_err()
}

#[test]
fn the_suite_catches_broken_engines() {
    // Never touches the bridge.
    assert!(catches(|_| Arc::new(FakeEngine::new(vec![vec![]]))));
    // Calls the tool twice.
    assert!(catches(|s| match s {
        Scenario::CallProbe => Arc::new(FakeEngine::new(vec![vec![probe(), probe()]])),
        s => honest(s),
    }));
    // Finishes on its own instead of working until Stop.
    assert!(catches(|s| match s {
        Scenario::WorkUntilStopped => Arc::new(FakeEngine::new(vec![vec![FakeStep::Emit(
            TurnEvent::Ended(TurnEnd::Done),
        )]])),
        s => honest(s),
    }));
    // Says it is unavailable.
    assert!(catches(|s| Arc::new(
        FakeEngine::new(vec![match s {
            Scenario::CallProbe => vec![probe()],
            Scenario::WorkUntilStopped => vec![FakeStep::Work],
        }])
        .with_status(EngineStatus::Missing)
    )));
}

#[test]
fn fake_engine_replays_events_and_records_requests() {
    let engine = FakeEngine::new(vec![vec![
        FakeStep::Emit(TurnEvent::Text("Building the card".into())),
        probe(),
        FakeStep::Emit(TurnEvent::Ended(TurnEnd::Failed(EngineError::RoundLimit))),
    ]]);
    let bridge = RecordingBridge::probe();
    let turn = TurnId::new("01J00000000000000000000T44").unwrap();
    let mut stream = engine
        .run_turn(TurnRequest::new(turn.clone(), "m", "Build a card"), &bridge)
        .unwrap();
    assert!(matches!(
        engine.run_turn(TurnRequest::new(turn.clone(), "m", "again"), &bridge),
        Err(EngineError::Busy)
    ));
    let mut events = vec![];
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    assert_eq!(events[0], TurnEvent::Text("Building the card".into()));
    assert!(matches!(
        events.last(),
        Some(TurnEvent::Ended(TurnEnd::Failed(EngineError::RoundLimit)))
    ));
    let TurnEvent::Messages(messages) = &events[events.len() - 2] else {
        panic!("messages before the end: {events:?}");
    };
    assert_eq!(messages.len(), 2);
    assert_eq!(engine.requests()[0].prompt, "Build a card");
    assert_eq!(bridge.calls().len(), 1);
    let unknown = bridge.call("nope", json!({}));
    assert!(unknown.failed);
}
