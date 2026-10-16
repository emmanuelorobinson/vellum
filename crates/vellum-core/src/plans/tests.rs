//! TEST DATA throughout.
use super::*;
use crate::ports::{EventBody, EventSeq, SessionEvent, Timestamp};
use crate::sessions::{body_from_json, body_to_json};
use serde_json::json;
use vellum_types::{Actor, DeviceId, Origin, SessionId};

fn ulid(n: u32) -> String {
    format!("01J{n:023}")
}
fn page(id: &str) -> PageId {
    PageId::new(id).unwrap()
}
fn agent() -> AgentId {
    AgentId::new(ulid(2)).unwrap()
}
fn turn(n: u32) -> TurnId {
    TurnId::new(ulid(100 + n)).unwrap()
}

fn item(id: u32, name: &str) -> PlanItem {
    PlanItem {
        id,
        name: name.into(),
        page: page("pageHome1"),
        intent: format!("TEST DATA: {name}"),
        width: Some(390.0),
        height: Some(844.0),
        frame: None,
    }
}

fn plan(names: &[&str]) -> Plan {
    Plan {
        title: "TEST DATA: Onboarding".into(),
        items: names
            .iter()
            .enumerate()
            .map(|(n, name)| item(n as u32 + 1, name))
            .collect(),
        changes: None,
    }
}

fn placeholder(item: u32, turn_n: u32) -> Placeholder {
    Placeholder {
        item,
        frame: NodeId::new(format!("planned{item}")).unwrap(),
        name: format!("Screen {item}"),
        page: page("pageHome1"),
        bounds: Rect::new(f64::from(item) * 500.0, 0.0, 390.0, 844.0),
        agent: agent(),
        turn: turn(turn_n),
    }
}

fn event(seq: u64, turn_n: u32, plan: PlanEvent) -> SessionEvent {
    SessionEvent {
        seq: EventSeq(seq),
        at: Timestamp(seq as i64),
        origin: Origin::new(Actor::Person {
            device: DeviceId::new(ulid(3)).unwrap(),
        })
        .in_session(SessionId::new(ulid(1)).unwrap())
        .in_turn(turn(turn_n)),
        body: EventBody::Plan(plan),
    }
}

// ---- the threshold ---------------------------------------------------------------------------------------------

#[test]
fn one_frame_needs_no_plan_and_two_do() {
    let settings = PlanSettings::default();
    assert!(!settings.needs_plan(&plan(&["Welcome"])));
    assert!(settings.needs_plan(&plan(&["Welcome", "Step 1"])));
}

#[test]
fn more_than_twenty_changes_need_a_plan_even_in_one_frame() {
    let settings = PlanSettings::default();
    let mut one = plan(&["Settings"]);
    one.changes = Some(20);
    assert!(!settings.needs_plan(&one));
    one.changes = Some(21);
    assert!(settings.needs_plan(&one));
}

#[test]
fn a_person_can_skip_plans_for_requests_under_a_size_they_choose() {
    let settings = PlanSettings {
        min_frames: 4,
        ..PlanSettings::default()
    };
    assert!(!settings.needs_plan(&plan(&["A", "B", "C"])));
    assert!(settings.needs_plan(&plan(&["A", "B", "C", "D"])));
}

// ---- checking a plan -------------------------------------------------------------------------------------------

#[test]
fn a_plan_is_checked_before_anyone_sees_it() {
    assert_eq!(check(&plan(&["Welcome", "Step 1"])), Ok(()));
    assert_eq!(check(&plan(&[])), Err(PlanError::NoItems));

    let mut untitled = plan(&["A"]);
    untitled.title = "  ".into();
    assert_eq!(check(&untitled), Err(PlanError::NoTitle));

    let mut twice = plan(&["A", "B"]);
    twice.items[1].id = 1;
    assert_eq!(check(&twice), Err(PlanError::DuplicateItem(1)));

    let mut unnamed = plan(&["A", "B"]);
    unnamed.items[1].name = String::new();
    assert_eq!(check(&unnamed), Err(PlanError::NoName(2)));

    let mut no_intent = plan(&["A"]);
    no_intent.items[0].intent = " ".into();
    assert_eq!(check(&no_intent), Err(PlanError::NoIntent(1)));

    let mut long = plan(&["A"]);
    long.items[0].intent = "x".repeat(INTENT_LIMIT + 1);
    assert_eq!(check(&long), Err(PlanError::TooLong(1, "intent")));

    let mut long_name = plan(&["A"]);
    long_name.items[0].name = "x".repeat(NAME_LIMIT + 1);
    assert_eq!(check(&long_name), Err(PlanError::TooLong(1, "name")));

    let names: Vec<String> = (0..=ITEM_LIMIT).map(|n| format!("F{n}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_eq!(check(&plan(&refs)), Err(PlanError::TooManyItems));
}

#[test]
fn a_new_frame_needs_a_size_and_a_changed_frame_does_not() {
    let mut sizeless = plan(&["A"]);
    sizeless.items[0].width = None;
    assert_eq!(check(&sizeless), Err(PlanError::NoSize(1)));
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, MAX_SIDE + 1.0] {
        let mut odd = plan(&["A"]);
        odd.items[0].height = Some(bad);
        assert_eq!(check(&odd), Err(PlanError::NoSize(1)), "{bad}");
    }
    let mut change = plan(&["Hero"]);
    change.items[0].frame = Some(NodeId::new("frameHero1").unwrap());
    change.items[0].width = None;
    change.items[0].height = None;
    assert_eq!(check(&change), Ok(()));
    assert!(!change.items[0].is_new());
}

#[test]
fn plan_errors_say_what_to_fix() {
    assert_eq!(
        PlanError::NoSize(3).to_string(),
        "Item 3 is a new frame: give its width and height, above 0 and at most 100000."
    );
    assert_eq!(
        PlanError::DuplicateItem(2).to_string(),
        "Two items are numbered 2."
    );
}

// ---- the session log -------------------------------------------------------------------------------------------

#[test]
fn plan_events_round_trip_through_the_session_codec() {
    let mut proposed = plan(&["Welcome", "Hero"]);
    proposed.changes = Some(30);
    proposed.items[1].frame = Some(NodeId::new("frameHero1").unwrap());
    proposed.items[1].width = None;
    proposed.items[1].height = None;
    let events = [
        PlanEvent::Proposed(proposed.clone()),
        PlanEvent::Built {
            plan: proposed,
            placeholders: vec![placeholder(1, 1)],
        },
        PlanEvent::Ticked(vec![1, 2]),
        PlanEvent::Finished,
        PlanEvent::Abandoned,
    ];
    for plan_event in events {
        let body = EventBody::Plan(plan_event);
        let json = body_to_json(&body);
        assert_eq!(body_from_json(&json).unwrap(), body, "{json}");
    }
}

#[test]
fn the_stored_shape_is_camel_case_and_strict() {
    let json = body_to_json(&EventBody::Plan(PlanEvent::Built {
        plan: plan(&["Welcome"]),
        placeholders: vec![placeholder(1, 1)],
    }));
    assert_eq!(json["type"], "plan");
    assert_eq!(json["event"], "built");
    assert_eq!(json["plan"]["items"][0]["page"], "pageHome1");
    assert_eq!(
        json["placeholders"][0]["bounds"],
        json!({"x": 500.0, "y": 0.0, "width": 390.0, "height": 844.0})
    );

    let mut extra = json.clone();
    extra["plan"]["items"][0]["colour"] = json!("teal");
    assert!(body_from_json(&extra).is_err());
    let unknown = json!({"type": "plan", "event": "paused"});
    assert!(body_from_json(&unknown).is_err());
    // The old text-only shape was never written by a release; it is refused like anything else unknown.
    assert!(body_from_json(&json!({"type": "plan", "text": "x"})).is_err());
}

#[test]
fn the_log_replays_into_the_plan_state() {
    let first = plan(&["Welcome", "Step 1"]);
    let edited = plan(&["Welcome", "Step 1", "Step 2"]);
    let mut log = vec![event(1, 1, PlanEvent::Proposed(first.clone()))];
    let state = plan_state(&log, &turn(1)).unwrap();
    assert_eq!(state.status, PlanStatus::Proposed);
    assert_eq!(state.plan, first);
    assert!(state.placeholders.is_empty());
    assert_eq!(plan_state(&log, &turn(2)), None, "another turn has no plan");

    // Edit request: the new proposal replaces the old one.
    log.push(event(2, 1, PlanEvent::Proposed(edited.clone())));
    assert_eq!(plan_state(&log, &turn(1)).unwrap().plan, edited);

    log.push(event(
        3,
        1,
        PlanEvent::Built {
            plan: edited.clone(),
            placeholders: vec![placeholder(1, 1), placeholder(2, 1), placeholder(3, 1)],
        },
    ));
    // Another turn's events change nothing here.
    log.push(event(4, 2, PlanEvent::Ticked(vec![1, 2, 3])));
    let state = plan_state(&log, &turn(1)).unwrap();
    assert_eq!(state.status, PlanStatus::Building);
    assert_eq!(state.progress(), (0, 3));

    log.push(event(5, 1, PlanEvent::Ticked(vec![2])));
    let state = plan_state(&log, &turn(1)).unwrap();
    assert_eq!(state.progress(), (1, 3));
    assert_eq!(
        state
            .placeholders
            .iter()
            .map(|p| p.item)
            .collect::<Vec<_>>(),
        [1, 3],
        "a finished item's placeholder becomes its frame"
    );

    let mut finished = log.clone();
    finished.push(event(6, 1, PlanEvent::Finished));
    let state = plan_state(&finished, &turn(1)).unwrap();
    assert_eq!(state.status, PlanStatus::Finished);
    assert!(state.placeholders.is_empty());
    assert_eq!(state.progress(), (1, 3), "unticked items stay unticked");

    log.push(event(6, 1, PlanEvent::Abandoned));
    let state = plan_state(&log, &turn(1)).unwrap();
    assert_eq!(state.status, PlanStatus::Abandoned);
    assert!(state.placeholders.is_empty());
}

#[test]
fn ticking_names_only_items_of_the_plan() {
    let mut state = PlanState::proposed(plan(&["A", "B"]));
    assert_eq!(state.tick(&[2]), Ok(vec![2]));
    assert_eq!(
        state.tick(&[2, 1]),
        Ok(vec![1]),
        "ticking twice is harmless"
    );
    assert_eq!(state.tick(&[3]), Err(PlanError::UnknownItem(3)));
    assert_eq!(state.progress(), (2, 2));
}

#[test]
fn the_build_prompt_lists_every_item_and_its_placeholder() {
    let mut approved = plan(&["Welcome", "Hero"]);
    approved.items[1].frame = Some(NodeId::new("frameHero1").unwrap());
    let text = build_prompt(&approved, &[placeholder(1, 1)]);
    assert!(text.contains("TEST DATA: Onboarding"), "{text}");
    assert!(
        text.contains("1. Welcome (page pageHome1): new frame `planned1` at x 500, y 0, 390 × 844"),
        "{text}"
    );
    assert!(
        text.contains("2. Hero (page pageHome1): change frame `frameHero1`"),
        "{text}"
    );
    assert!(text.contains("report_progress"), "{text}");
}
