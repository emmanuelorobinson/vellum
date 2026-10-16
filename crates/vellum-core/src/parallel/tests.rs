//! Parallel lanes, options and their log (guide Prompt 39). Everything here is `TEST DATA`.
use super::*;
use crate::plans::{Placeholder, Plan, PlanItem};
use crate::ports::{EventSeq, Rect, SessionEvent, Timestamp};
use vellum_types::{Actor, AgentId, NodeId, Origin, PageId, SessionId, TurnId};

fn agent(n: u32) -> AgentId {
    AgentId::new(format!("01JA{n:022}")).unwrap()
}
fn turn(n: u32) -> TurnId {
    TurnId::new(format!("01JT{n:022}")).unwrap()
}
fn session() -> SessionId {
    SessionId::new("01JS0000000000000000000001").unwrap()
}
fn page() -> PageId {
    PageId::new("pageHome01").unwrap()
}

fn plan(n: u32) -> Plan {
    Plan {
        title: "TEST DATA: Onboarding".into(),
        items: (1..=n)
            .map(|id| PlanItem {
                id,
                name: format!("Screen {id}"),
                page: page(),
                intent: format!("TEST DATA: screen {id}"),
                width: Some(390.0),
                height: Some(844.0),
                frame: None,
            })
            .collect(),
        changes: None,
    }
}

fn placeholder(item: u32, by: &AgentId) -> Placeholder {
    Placeholder {
        item,
        frame: NodeId::new(format!("frameP{item:04}")).unwrap(),
        name: format!("Screen {item}"),
        page: page(),
        bounds: Rect::new(f64::from(item) * 500.0, 0.0, 390.0, 844.0),
        agent: by.clone(),
        turn: turn(1),
    }
}

#[test]
fn split_divides_items_into_contiguous_lanes_of_about_equal_size() {
    let items: Vec<u32> = (1..=12).collect();
    let lanes = split(&items, 4);
    assert_eq!(lanes.len(), 4);
    assert!(lanes.iter().all(|lane| lane.len() == 3), "{lanes:?}");
    assert_eq!(lanes.concat(), items, "every item once, in plan order");

    let lanes = split(&(1..=7).collect::<Vec<_>>(), 3);
    assert_eq!(lanes, [vec![1, 2, 3], vec![4, 5], vec![6, 7]]);

    // Never an empty lane: two items for four agents is two lanes.
    assert_eq!(split(&[1, 2], 4), [vec![1], vec![2]]);
    assert_eq!(split(&[5], 1), [vec![5]]);
    assert!(split(&[], 3).is_empty());
}

#[test]
fn lane_counts_and_variant_counts_are_checked() {
    assert_eq!(check_lanes(0), Err(ParallelError::Lanes(0)));
    assert_eq!(check_lanes(1), Ok(()));
    assert_eq!(check_lanes(MAX_LANES), Ok(()));
    assert_eq!(
        check_lanes(MAX_LANES + 1),
        Err(ParallelError::Lanes(MAX_LANES + 1))
    );
    assert_eq!(check_variants(1), Err(ParallelError::Variants(1)));
    assert_eq!(check_variants(2), Ok(()));
    assert_eq!(check_variants(6), Ok(()));
    assert_eq!(check_variants(7), Err(ParallelError::Variants(7)));
    assert!(
        ParallelError::Lanes(7).to_string().contains("1 to 6"),
        "{}",
        ParallelError::Lanes(7)
    );
}

#[test]
fn options_are_lettered() {
    assert_eq!(
        (1..=6).map(option_label).collect::<Vec<_>>(),
        ["A", "B", "C", "D", "E", "F"]
    );
}

#[test]
fn a_split_lane_is_told_its_items_only_and_to_leave_the_others_alone() {
    let teal = agent(1);
    let saffron = agent(2);
    let placeholders: Vec<Placeholder> = (1..=4)
        .map(|n| placeholder(n, if n <= 2 { &teal } else { &saffron }))
        .collect();
    let prompt = split_prompt(&plan(4), &[3, 4], &placeholders, 2);
    assert!(prompt.contains("frameP0003") && prompt.contains("frameP0004"));
    assert!(
        !prompt.contains("frameP0001") && !prompt.contains("Screen 1 "),
        "{prompt}"
    );
    assert!(prompt.contains("2 agents"), "{prompt}");
    assert!(prompt.contains("report_progress"), "{prompt}");
}

#[test]
fn a_side_by_side_lane_gets_the_whole_brief_and_its_own_frame() {
    let area = placeholder(2, &agent(2));
    let prompt = side_by_side_prompt("TEST DATA: a paywall", 2, 3, &area);
    assert!(prompt.starts_with("TEST DATA: a paywall"), "{prompt}");
    assert!(prompt.contains("frameP0002"), "{prompt}");
    assert!(prompt.contains("\"B\""), "the option is lettered: {prompt}");
    assert!(prompt.contains("3 agents"), "{prompt}");
    assert!(prompt.contains("x 1000, y 0, 390 × 844"), "{prompt}");
}

#[test]
fn an_explore_variant_varies_only_its_axis() {
    let first = NodeId::new("frameFirst").unwrap();
    let area = placeholder(3, &agent(1));
    let layout = explore_prompt("TEST DATA: a paywall", Axis::Layout, 3, 4, &first, &area);
    assert!(layout.contains("Variant 3 of 4"), "{layout}");
    assert!(layout.contains("copy_nodes") && layout.contains("frameFirst"));
    assert!(layout.contains("frameP0003"));
    assert!(layout.contains("only its layout"), "{layout}");
    assert!(layout.contains("Keep its colours"), "{layout}");
    let style = explore_prompt("TEST DATA: a paywall", Axis::Style, 2, 4, &first, &area);
    assert!(style.contains("only its style"), "{style}");
    assert!(style.contains("Keep its structure"), "{style}");
    assert!(
        style.contains("report_progress"),
        "the label comes from it: {style}"
    );
    assert_ne!(layout, style);
}

#[test]
fn spawned_helpers_are_told_their_task_and_who_asked() {
    let prompt = helper_prompt("TEST DATA: make the icons", "Teal");
    assert!(prompt.contains("Teal started you"), "{prompt}");
    assert!(prompt.contains("TEST DATA: make the icons"));
    assert!(prompt.contains("report_progress"));
}

fn event(seq: u64, actor: Actor, in_turn: &TurnId, body: EventBody) -> SessionEvent {
    SessionEvent {
        seq: EventSeq(seq),
        at: Timestamp(seq as i64),
        origin: Origin::new(actor)
            .in_session(session())
            .in_turn(in_turn.clone()),
        body,
    }
}

fn lane(n: u32, items: Vec<u32>) -> LaneRecord {
    LaneRecord {
        agent: agent(n),
        name: format!("TEST DATA lane {n}"),
        model: "fake-model".into(),
        items,
        task: None,
    }
}

#[test]
fn the_log_replays_lanes_their_ends_and_options() {
    let person = Actor::Person {
        device: vellum_types::DeviceId::new("01JD0000000000000000000001").unwrap(),
    };
    let t = turn(1);
    let other = turn(2);
    let events = vec![
        event(
            1,
            person.clone(),
            &t,
            EventBody::Parallel(ParallelEvent::Lanes {
                mode: Mode::Split,
                lanes: vec![lane(1, vec![1, 2]), lane(2, vec![3])],
            }),
        ),
        event(
            2,
            person.clone(),
            &other,
            EventBody::Parallel(ParallelEvent::LaneEnded {
                agent: agent(1),
                end: LaneEnd::Failed,
            }),
        ),
        event(
            3,
            Actor::Agent { agent: agent(1) },
            &t,
            EventBody::Parallel(ParallelEvent::Lanes {
                mode: Mode::Spawned,
                lanes: vec![LaneRecord {
                    task: Some("TEST DATA: icons".into()),
                    ..lane(3, vec![])
                }],
            }),
        ),
        event(
            4,
            Actor::Agent { agent: agent(2) },
            &t,
            EventBody::Parallel(ParallelEvent::LaneEnded {
                agent: agent(2),
                end: LaneEnd::Done,
            }),
        ),
        event(
            5,
            person,
            &t,
            EventBody::Parallel(ParallelEvent::LaneEnded {
                agent: agent(1),
                end: LaneEnd::Stopped,
            }),
        ),
        event(
            6,
            Actor::Agent { agent: agent(2) },
            &t,
            EventBody::Parallel(ParallelEvent::Option(OptionRecord {
                option: 1,
                agent: agent(2),
                frame: NodeId::new("frameOptA1").unwrap(),
                label: Some("TEST DATA: warmer palette".into()),
            })),
        ),
    ];
    let state = parallel_state(&events, &t).expect("lanes");
    assert_eq!(
        state.mode,
        Mode::Split,
        "the first Lanes event sets the mode"
    );
    assert_eq!(state.lanes.len(), 3, "a spawned helper joins the lanes");
    assert_eq!(state.lanes[0].end, Some(LaneEnd::Stopped));
    assert_eq!(state.lanes[1].end, Some(LaneEnd::Done));
    assert_eq!(state.lanes[2].end, None, "the helper is still working");
    assert_eq!(
        state.lanes[2].record.task.as_deref(),
        Some("TEST DATA: icons")
    );
    assert_eq!(state.options.len(), 1);
    assert_eq!(state.options[0].frame.as_str(), "frameOptA1");
    assert!(parallel_state(&events, &turn(3)).is_none());
}

#[test]
fn modes_and_ends_have_stable_names() {
    for mode in [
        Mode::Split,
        Mode::SideBySide,
        Mode::Explore(Axis::Layout),
        Mode::Explore(Axis::Style),
        Mode::Spawned,
    ] {
        assert_eq!(Mode::parse(mode.as_str()), Some(mode));
    }
    assert_eq!(Mode::Explore(Axis::Style).as_str(), "explore_style");
    for end in [LaneEnd::Done, LaneEnd::Failed, LaneEnd::Stopped] {
        assert_eq!(LaneEnd::parse(end.as_str()), Some(end));
    }
    assert_eq!(Mode::parse("spiral"), None);
}
