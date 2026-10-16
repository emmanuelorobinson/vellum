//! The Guard (guide Invariant 4, §10.2; ARCHITECTURE §5, §7.5). Everything here is `TEST DATA`.

use serde_json::json;
use vellum_types::{Actor, AgentId, Document, FileReason, Origin, Subsystem};

use super::{
    AgentControl, GuardState, Limits, PauseSet, Rejection, Verdict, WriteError, apply_guarded,
    guard, reasons,
};
use crate::lease::{BlockedPairs, Leases};
use crate::ops::tests::common::{
    ALICE, Ids, Store, document, id, in_node, on_page, patch, person, shape, var_id, variable,
};
use crate::ops::{Inputs, Op, Plan, Transaction, plan};
use crate::ports::Timestamp;

const TEAL: &str = "01JA00000000000000000000A1";
const ROSE: &str = "01JA00000000000000000000A2";
const NOW: Timestamp = Timestamp(100_000);

fn agent(text: &str) -> AgentId {
    AgentId::new(text).expect("a ULID")
}

fn by(text: &str) -> Actor {
    Actor::Agent { agent: agent(text) }
}

fn teal_tx(ops: Vec<Op>) -> Transaction {
    Transaction::new(Origin::new(by(TEAL)), ops)
}

fn planned(doc: &Document, tx: &Transaction) -> Plan {
    plan(doc, tx, &mut Ids::default())
        .unwrap_or_else(|e| panic!("refused: {e} at {}", e.location()))
}

/// Guards `tx` against the sample document.
fn check(tx: &Transaction, leases: &Leases, pause: &PauseSet, blocked: &BlockedPairs) -> Verdict {
    check_with(tx, leases, pause, blocked, &Limits::default())
}

fn check_with(
    tx: &Transaction,
    leases: &Leases,
    pause: &PauseSet,
    blocked: &BlockedPairs,
    limits: &Limits,
) -> Verdict {
    let doc = document();
    let plan = planned(&doc, tx);
    guard(
        &super::Write::new(&doc, tx, &plan),
        leases,
        pause,
        blocked,
        limits,
        NOW,
    )
}

fn leased(frame: &str, holder: &str, at: i64) -> Leases {
    let mut leases = Leases::default();
    leases
        .claim(
            &id(frame),
            agent(holder),
            &BlockedPairs::default(),
            Timestamp(at),
        )
        .expect("a free frame");
    leases
}

fn rejection(verdict: Verdict) -> Rejection {
    match verdict {
        Verdict::Reject(rejection) => rejection,
        Verdict::Allow { .. } => panic!("the write should have been rejected"),
    }
}

fn touch_rect() -> Op {
    Op::Update {
        id: id("rect000001"),
        patch: patch(json!({"opacity": 0.5})),
    }
}

#[derive(Debug, Clone, Copy)]
enum LeaseCase {
    Free,
    Own,
    Other,
    OtherExpired,
}

#[derive(Debug, Clone, Copy)]
enum PauseCase {
    Running,
    All,
    SelfPaused,
    SelfStopped,
    OtherPaused,
}

/// Actor kinds × lease states × pause states × blocked, with a write inside `frame00001`. The expected column is
/// the rule from Invariant 4, written out once.
#[test]
fn guard_exhaustive_table() {
    let actors = [
        ("person", person(ALICE)),
        ("agent", by(TEAL)),
        (
            "file",
            Actor::File {
                reason: FileReason::ExternalChange,
            },
        ),
        (
            "system",
            Actor::System {
                subsystem: Subsystem::Importer,
            },
        ),
    ];
    let lease_cases = [
        LeaseCase::Free,
        LeaseCase::Own,
        LeaseCase::Other,
        LeaseCase::OtherExpired,
    ];
    let pause_cases = [
        PauseCase::Running,
        PauseCase::All,
        PauseCase::SelfPaused,
        PauseCase::SelfStopped,
        PauseCase::OtherPaused,
    ];
    let mut rows = 0;
    for (actor_name, actor) in &actors {
        for lease_case in lease_cases {
            for pause_case in pause_cases {
                for is_blocked in [false, true] {
                    let leases = match lease_case {
                        LeaseCase::Free => Leases::default(),
                        LeaseCase::Own => leased("frame00001", TEAL, NOW.0 - 1_000),
                        LeaseCase::Other => leased("frame00001", ROSE, NOW.0 - 1_000),
                        LeaseCase::OtherExpired => leased("frame00001", ROSE, NOW.0 - 60_000),
                    };
                    let mut pause = PauseSet::default();
                    match pause_case {
                        PauseCase::Running => {}
                        PauseCase::All => pause.pause_all(),
                        PauseCase::SelfPaused => pause.pause(agent(TEAL)),
                        PauseCase::SelfStopped => pause.stop(agent(TEAL)),
                        PauseCase::OtherPaused => pause.pause(agent(ROSE)),
                    }
                    let mut blocked = BlockedPairs::default();
                    if is_blocked {
                        blocked.block(agent(TEAL), id("frame00001"));
                    }
                    let tx = Transaction::new(Origin::new(actor.clone()), vec![touch_rect()]);
                    let verdict = check(&tx, &leases, &pause, &blocked);

                    let expected = if !actor.is_agent() {
                        None
                    } else if matches!(pause_case, PauseCase::SelfStopped) {
                        Some(Rejection::Stopped { agent: agent(TEAL) })
                    } else if matches!(pause_case, PauseCase::All) {
                        Some(Rejection::AllPaused)
                    } else if matches!(pause_case, PauseCase::SelfPaused) {
                        Some(Rejection::Paused { agent: agent(TEAL) })
                    } else if is_blocked {
                        Some(Rejection::StoppedHere {
                            agent: agent(TEAL),
                            frame: id("frame00001"),
                        })
                    } else if matches!(lease_case, LeaseCase::Other) {
                        Some(Rejection::Leased {
                            frame: id("frame00001"),
                            holder: agent(ROSE),
                        })
                    } else {
                        None
                    };
                    let name = format!(
                        "{actor_name} × {lease_case:?} × {pause_case:?} × blocked={is_blocked}"
                    );
                    match (expected, verdict) {
                        (None, Verdict::Allow { .. }) => {}
                        (Some(want), Verdict::Reject(got)) => assert_eq!(got, want, "{name}"),
                        (want, got) => panic!("{name}: expected {want:?}, got {got:?}"),
                    }
                    rows += 1;
                }
            }
        }
    }
    assert_eq!(rows, 4 * 4 * 5 * 2);
}

#[test]
fn guard_allows_an_agent_inside_its_own_lease_and_reports_it() {
    let leases = leased("frame00001", TEAL, NOW.0);
    let verdict = check(
        &teal_tx(vec![touch_rect()]),
        &leases,
        &PauseSet::default(),
        &BlockedPairs::default(),
    );
    assert_eq!(
        verdict,
        Verdict::Allow {
            own_frames: [id("frame00001")].into()
        }
    );
}

#[test]
fn guard_allows_an_agent_outside_another_agents_lease() {
    let leases = leased("frame00001", ROSE, NOW.0);
    let tx = teal_tx(vec![Op::Update {
        id: id("rect000003"),
        patch: patch(json!({"opacity": 0.5})),
    }]);
    let verdict = check(&tx, &leases, &PauseSet::default(), &BlockedPairs::default());
    assert_eq!(
        verdict,
        Verdict::Allow {
            own_frames: Default::default()
        }
    );
}

/// Each way of writing inside, into, out of, or over a leased frame is a write inside it.
#[test]
fn guard_rejects_every_kind_of_write_touching_another_agents_lease() {
    let leases = leased("frame00001", ROSE, NOW.0);
    let leased_by_rose = Rejection::Leased {
        frame: id("frame00001"),
        holder: agent(ROSE),
    };
    let cases: Vec<(&str, Vec<Op>)> = vec![
        (
            "update the frame",
            vec![Op::Update {
                id: id("frame00001"),
                patch: patch(json!({"opacity": 0.5})),
            }],
        ),
        (
            "insert into it",
            vec![Op::Insert {
                to: in_node("frame00001", 0),
                node: Box::new(shape("rect", "pnew000001", json!({}))),
            }],
        ),
        (
            "delete inside it",
            vec![Op::Delete {
                id: id("rect000002"),
            }],
        ),
        (
            "delete it",
            vec![Op::Delete {
                id: id("frame00001"),
            }],
        ),
        (
            "move out of it",
            vec![Op::Move {
                id: id("rect000001"),
                to: on_page("canvas0001", 0),
            }],
        ),
        (
            "move into it",
            vec![Op::Move {
                id: id("rect000003"),
                to: in_node("frame00001", 0),
            }],
        ),
        (
            "move it",
            vec![Op::Move {
                id: id("frame00001"),
                to: in_node("group00001", 0),
            }],
        ),
        (
            "reorder inside it",
            vec![Op::Reorder {
                id: id("rect000001"),
                index: 1,
            }],
        ),
        (
            "copy into it",
            vec![Op::Copy {
                id: id("rect000003"),
                to: in_node("frame00001", 0),
            }],
        ),
        (
            "replace inside it",
            vec![Op::Replace {
                id: id("rect000001"),
                node: Box::new(shape("ellipse", "rect000001", json!({}))),
            }],
        ),
        (
            "wrap it",
            vec![Op::WrapInFrame {
                ids: vec![id("frame00001")],
                frame: Box::new(shape("frame", "pnew000001", json!({}))),
            }],
        ),
        (
            "a later op in the batch",
            vec![
                Op::Update {
                    id: id("rect000003"),
                    patch: patch(json!({"opacity": 0.5})),
                },
                Op::Delete {
                    id: id("rect000001"),
                },
            ],
        ),
    ];
    for (name, ops) in cases {
        let verdict = check(
            &teal_tx(ops),
            &leases,
            &PauseSet::default(),
            &BlockedPairs::default(),
        );
        assert_eq!(rejection(verdict), leased_by_rose, "{name}");
    }
}

/// Removing a container takes the leased frame inside it with it.
#[test]
fn guard_rejects_removing_an_ancestor_of_another_agents_lease() {
    let doc = planned(
        &document(),
        &Transaction::new(
            Origin::new(person(ALICE)),
            vec![Op::Move {
                id: id("frame00001"),
                to: in_node("group00001", 0),
            }],
        ),
    )
    .document;
    let leases = leased("frame00001", ROSE, NOW.0);
    let tx = teal_tx(vec![Op::Delete {
        id: id("group00001"),
    }]);
    let plan = planned(&doc, &tx);
    let verdict = guard(
        &super::Write::new(&doc, &tx, &plan),
        &leases,
        &PauseSet::default(),
        &BlockedPairs::default(),
        &Limits::default(),
        NOW,
    );
    assert_eq!(
        rejection(verdict),
        Rejection::Leased {
            frame: id("frame00001"),
            holder: agent(ROSE)
        }
    );
}

#[test]
fn guard_lets_an_agent_read_out_of_another_agents_lease() {
    let leases = leased("frame00001", ROSE, NOW.0);
    let tx = teal_tx(vec![Op::Copy {
        id: id("rect000001"),
        to: on_page("canvas0001", 0),
    }]);
    let verdict = check(&tx, &leases, &PauseSet::default(), &BlockedPairs::default());
    assert!(matches!(verdict, Verdict::Allow { .. }), "{verdict:?}");
}

#[test]
fn guard_leaves_variables_outside_every_lease() {
    let leases = leased("frame00001", ROSE, NOW.0);
    let tx = teal_tx(vec![Op::SetVariables {
        set: [(
            var_id("color00001"),
            variable(
                json!({"type": "color", "name": "color/primary", "values": [{"value": "#000000"}]}),
            ),
        )]
        .into(),
        remove: Default::default(),
    }]);
    let verdict = check(&tx, &leases, &PauseSet::default(), &BlockedPairs::default());
    assert!(matches!(verdict, Verdict::Allow { .. }), "{verdict:?}");
}

#[test]
fn guard_covers_a_lease_inside_a_component_definition() {
    let leases = leased("croot00001", ROSE, NOW.0);
    let tx = teal_tx(vec![Op::Update {
        id: id("ctext00001"),
        patch: patch(json!({"opacity": 0.5})),
    }]);
    let verdict = check(&tx, &leases, &PauseSet::default(), &BlockedPairs::default());
    assert_eq!(
        rejection(verdict),
        Rejection::Leased {
            frame: id("croot00001"),
            holder: agent(ROSE)
        }
    );
}

/// Removing a component definition removes the leased node in it.
#[test]
fn guard_rejects_removing_a_definition_holding_another_agents_lease() {
    let component = vellum_types::ComponentId::new("comp000002").expect("a valid id");
    let mut ids = Ids::default();
    let create = Transaction::new(
        Origin::new(person(ALICE)),
        vec![Op::CreateComponent {
            id: id("frame00001"),
            component_id: component.clone(),
        }],
    );
    let doc = plan(&document(), &create, &mut ids)
        .expect("a component")
        .document;
    let root = doc.components[&component].root.id().clone();
    let mut leases = Leases::default();
    leases
        .claim(&root, agent(ROSE), &BlockedPairs::default(), NOW)
        .unwrap();

    let tx = teal_tx(vec![Op::ToggleComponent {
        id: id("frame00001"),
        component_id: component,
    }]);
    let plan = plan(&doc, &tx, &mut ids).expect("the only instance detaches");
    let verdict = guard(
        &super::Write::new(&doc, &tx, &plan),
        &leases,
        &PauseSet::default(),
        &BlockedPairs::default(),
        &Limits::default(),
        NOW,
    );
    assert_eq!(
        rejection(verdict),
        Rejection::Leased {
            frame: root,
            holder: agent(ROSE)
        }
    );
}

#[test]
fn guard_blocks_a_stopped_pair_even_without_a_lease() {
    let mut blocked = BlockedPairs::default();
    blocked.block(agent(TEAL), id("frame00001"));
    let verdict = check(
        &teal_tx(vec![touch_rect()]),
        &Leases::default(),
        &PauseSet::default(),
        &blocked,
    );
    assert_eq!(
        rejection(verdict),
        Rejection::StoppedHere {
            agent: agent(TEAL),
            frame: id("frame00001")
        }
    );
    // Rose is not blocked there.
    let rose = Transaction::new(Origin::new(by(ROSE)), vec![touch_rect()]);
    let verdict = check(&rose, &Leases::default(), &PauseSet::default(), &blocked);
    assert!(matches!(verdict, Verdict::Allow { .. }));
}

#[test]
fn guard_limits_ops_per_agent_transaction() {
    let limits = Limits {
        max_ops: 2,
        max_inserted_nodes: 100,
    };
    let three = vec![touch_rect(), touch_rect(), touch_rect()];
    let verdict = check_with(
        &teal_tx(three.clone()),
        &Leases::default(),
        &PauseSet::default(),
        &BlockedPairs::default(),
        &limits,
    );
    assert_eq!(
        rejection(verdict),
        Rejection::TooManyOps { count: 3, limit: 2 }
    );

    let two = vec![touch_rect(), touch_rect()];
    let verdict = check_with(
        &teal_tx(two),
        &Leases::default(),
        &PauseSet::default(),
        &BlockedPairs::default(),
        &limits,
    );
    assert!(matches!(verdict, Verdict::Allow { .. }));

    let by_person = Transaction::new(Origin::new(person(ALICE)), three);
    let verdict = check_with(
        &by_person,
        &Leases::default(),
        &PauseSet::default(),
        &BlockedPairs::default(),
        &limits,
    );
    assert!(
        matches!(verdict, Verdict::Allow { .. }),
        "people have no limits"
    );
}

#[test]
fn guard_limits_inserted_nodes_per_agent_transaction() {
    let limits = Limits {
        max_ops: 100,
        max_inserted_nodes: 2,
    };
    let frame = crate::ops::tests::common::node(json!({
        "type": "frame", "id": "pnew000001", "name": "TEST DATA", "width": 10, "height": 10,
        "children": [
            {"type": "rect", "id": "pnew000002", "name": "TEST DATA", "width": 1, "height": 1},
            {"type": "rect", "id": "pnew000003", "name": "TEST DATA", "width": 1, "height": 1}
        ]
    }));
    let insert = Op::Insert {
        to: on_page("canvas0001", 0),
        node: Box::new(frame),
    };
    let verdict = check_with(
        &teal_tx(vec![insert.clone()]),
        &Leases::default(),
        &PauseSet::default(),
        &BlockedPairs::default(),
        &limits,
    );
    assert_eq!(
        rejection(verdict),
        Rejection::TooManyNodes { count: 3, limit: 2 }
    );

    // Copies count as inserted nodes too: frame00001 holds two rects.
    let copy = Op::Copy {
        id: id("frame00001"),
        to: on_page("canvas0001", 0),
    };
    let verdict = check_with(
        &teal_tx(vec![copy]),
        &Leases::default(),
        &PauseSet::default(),
        &BlockedPairs::default(),
        &limits,
    );
    assert_eq!(
        rejection(verdict),
        Rejection::TooManyNodes { count: 3, limit: 2 }
    );

    let by_person = Transaction::new(Origin::new(person(ALICE)), vec![insert]);
    let verdict = check_with(
        &by_person,
        &Leases::default(),
        &PauseSet::default(),
        &BlockedPairs::default(),
        &limits,
    );
    assert!(matches!(verdict, Verdict::Allow { .. }));
}

#[test]
fn guard_default_limits_are_generous_and_positive() {
    let limits = Limits::default();
    assert!(limits.max_ops >= 100);
    assert!(limits.max_inserted_nodes >= 1_000);
}

#[test]
fn guard_pause_set_resume_rules() {
    let mut pause = PauseSet::default();
    pause.pause(agent(TEAL));
    pause.stop(agent(ROSE));
    pause.pause_all();
    assert!(pause.is_all_paused());

    // Resume all lifts Pause all and individual pauses, not a Stop: a stopped agent needs a new turn.
    pause.resume_all();
    assert!(!pause.is_all_paused());
    assert_eq!(pause.check(&agent(TEAL)), Ok(()));
    assert_eq!(
        pause.check(&agent(ROSE)),
        Err(Rejection::Stopped { agent: agent(ROSE) })
    );

    pause.resume(&agent(ROSE));
    assert_eq!(pause.check(&agent(ROSE)), Ok(()));
}

#[test]
fn guard_reasons_are_plain_language() {
    let name = |a: &AgentId| {
        if *a == agent(TEAL) {
            "Teal".to_owned()
        } else {
            "Rose".to_owned()
        }
    };
    let cases = [
        (
            Rejection::Leased {
                frame: id("frame00001"),
                holder: agent(ROSE),
            },
            "Frame leased by Rose",
        ),
        (Rejection::AllPaused, "Agents paused"),
        (Rejection::Paused { agent: agent(TEAL) }, "Teal is paused"),
        (Rejection::Stopped { agent: agent(TEAL) }, "Stopped by you"),
        (
            Rejection::StoppedHere {
                agent: agent(TEAL),
                frame: id("frame00001"),
            },
            "Stopped by you on this frame",
        ),
        (
            Rejection::TooManyOps {
                count: 1_200,
                limit: 1_000,
            },
            "1200 changes at once · the limit is 1000",
        ),
        (
            Rejection::TooManyNodes { count: 3, limit: 2 },
            "3 new layers at once · the limit is 2",
        ),
    ];
    for (rejection, text) in cases {
        assert_eq!(rejection.message(&name), text);
        assert!(!text.contains('!'), "no exclamation marks: {text}");
    }
}

/// Each reason template is the string `@vellum/config` exports under `guardReasons`, so the editor, the daemon and
/// MCP results say the same thing.
#[test]
fn guard_reasons_match_vellum_config() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/config/src/index.ts"
    );
    let source = std::fs::read_to_string(path).expect("packages/config/src/index.ts");
    let copy = object(&source, "export const copy = {");
    let config = object(&source, "export const guardReasons = {");
    let resolved: std::collections::BTreeMap<String, String> = config
        .into_iter()
        .map(|(key, value)| {
            let value = match value.strip_prefix("copy.") {
                Some(name) => copy
                    .iter()
                    .find(|(k, _)| k == name)
                    .unwrap_or_else(|| panic!("copy.{name} is not in @vellum/config"))
                    .1
                    .clone(),
                None => value,
            };
            (key, value)
        })
        .collect();
    let ours: std::collections::BTreeMap<String, String> = reasons::ALL
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    assert_eq!(resolved, ours);
    assert_eq!(
        reasons::LEASED,
        "Frame leased by {agent}",
        "Appendix B, Write rejected"
    );
}

/// The `key: "value",` and `key: copy.name,` lines of one `export const … = { … } as const;` object.
fn object(source: &str, opening: &str) -> Vec<(String, String)> {
    let start = source
        .find(opening)
        .unwrap_or_else(|| panic!("no `{opening}`"));
    let end = start
        + source[start..]
            .find("} as const;")
            .expect("the object is closed");
    source[start..end]
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
        .map(|line| {
            let (key, value) = line.trim().split_once(": ").expect("key: value");
            let value = value.trim_end_matches(',');
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            (key.to_owned(), value.to_owned())
        })
        .collect()
}

// --- the guarded write path ---

fn state() -> GuardState {
    GuardState::default()
}

/// Invariant 4: after Pause all, the next agent transaction is rejected and nothing reaches the store; a person's
/// still applies.
#[test]
fn guard_pause_all_rejects_the_next_agent_transaction() {
    let mut store = Store::new(document());
    let mut guard_state = state();
    guard_state.pause_all(NOW);

    let result = apply_guarded(
        &mut store,
        &mut Ids::default(),
        teal_tx(vec![touch_rect()]),
        &Inputs::default(),
        &mut guard_state,
        NOW,
    );
    assert_eq!(
        result.unwrap_err(),
        WriteError::Rejected(Rejection::AllPaused)
    );
    assert_eq!(store.applied, 0);
    assert_eq!(store.doc, document());

    let by_person = Transaction::new(Origin::new(person(ALICE)), vec![touch_rect()]);
    apply_guarded(
        &mut store,
        &mut Ids::default(),
        by_person,
        &Inputs::default(),
        &mut guard_state,
        NOW,
    )
    .expect("people are never paused");
    assert_eq!(store.applied, 1);
}

#[test]
fn guard_pause_all_releases_leases() {
    let mut guard_state = state();
    guard_state
        .leases
        .claim(
            &id("frame00001"),
            agent(ROSE),
            &BlockedPairs::default(),
            NOW,
        )
        .unwrap();
    guard_state.pause_all(NOW);
    assert!(guard_state.leases.live(NOW).next().is_none());
}

/// A paused agent is rejected before its batch is even checked: the reason is the pause, not the bad op.
#[test]
fn guard_rejects_a_paused_agent_before_validation() {
    let mut store = Store::new(document());
    let mut guard_state = state();
    guard_state.pause.pause(agent(TEAL));
    let bad = teal_tx(vec![Op::Delete {
        id: id("missing001"),
    }]);
    let result = apply_guarded(
        &mut store,
        &mut Ids::default(),
        bad,
        &Inputs::default(),
        &mut guard_state,
        NOW,
    );
    assert_eq!(
        result.unwrap_err(),
        WriteError::Rejected(Rejection::Paused { agent: agent(TEAL) })
    );
}

#[test]
fn guard_invalid_batches_are_still_refused_as_invalid() {
    let mut store = Store::new(document());
    let bad = teal_tx(vec![Op::Delete {
        id: id("missing001"),
    }]);
    let result = apply_guarded(
        &mut store,
        &mut Ids::default(),
        bad,
        &Inputs::default(),
        &mut state(),
        NOW,
    );
    assert!(matches!(result, Err(WriteError::Invalid(_))), "{result:?}");
}

#[test]
fn guard_activity_inside_an_own_lease_extends_it() {
    let mut store = Store::new(document());
    let mut guard_state = state();
    guard_state
        .leases
        .claim(
            &id("frame00001"),
            agent(TEAL),
            &BlockedPairs::default(),
            Timestamp(0),
        )
        .unwrap();
    apply_guarded(
        &mut store,
        &mut Ids::default(),
        teal_tx(vec![touch_rect()]),
        &Inputs::default(),
        &mut guard_state,
        Timestamp(50_000),
    )
    .expect("Teal's own lease");
    // Without the write it would have expired at 60 s; now it lasts until 110 s.
    assert_eq!(
        guard_state
            .leases
            .holder(&id("frame00001"), Timestamp(100_000)),
        Some(&agent(TEAL))
    );
    assert_eq!(
        guard_state
            .leases
            .holder(&id("frame00001"), Timestamp(110_000)),
        None
    );
}

#[test]
fn guard_rejected_writes_leave_the_store_untouched() {
    let mut store = Store::new(document());
    let mut guard_state = state();
    guard_state
        .leases
        .claim(
            &id("frame00001"),
            agent(ROSE),
            &BlockedPairs::default(),
            NOW,
        )
        .unwrap();
    let result = apply_guarded(
        &mut store,
        &mut Ids::default(),
        teal_tx(vec![touch_rect()]),
        &Inputs::default(),
        &mut guard_state,
        NOW,
    );
    assert_eq!(
        result.unwrap_err(),
        WriteError::Rejected(Rejection::Leased {
            frame: id("frame00001"),
            holder: agent(ROSE)
        })
    );
    assert_eq!(store.applied, 0);
    assert_eq!(store.doc, document());
}

/// Stop on a lease tab: the lease goes, and the agent's next write there is rejected until resumed.
#[test]
fn guard_stop_on_lease_blocks_until_resumed() {
    let mut store = Store::new(document());
    let mut guard_state = state();
    guard_state
        .leases
        .claim(
            &id("frame00001"),
            agent(TEAL),
            &BlockedPairs::default(),
            NOW,
        )
        .unwrap();
    assert_eq!(
        guard_state.stop_on_lease(&id("frame00001"), NOW),
        Some(agent(TEAL))
    );

    let result = apply_guarded(
        &mut store,
        &mut Ids::default(),
        teal_tx(vec![touch_rect()]),
        &Inputs::default(),
        &mut guard_state,
        NOW,
    );
    assert_eq!(
        result.unwrap_err(),
        WriteError::Rejected(Rejection::StoppedHere {
            agent: agent(TEAL),
            frame: id("frame00001")
        })
    );
    guard_state.blocked.resume(&agent(TEAL), &id("frame00001"));
    apply_guarded(
        &mut store,
        &mut Ids::default(),
        teal_tx(vec![touch_rect()]),
        &Inputs::default(),
        &mut guard_state,
        NOW,
    )
    .expect("resumed");
}

#[test]
fn guard_write_error_says_why() {
    let error = WriteError::Rejected(Rejection::AllPaused);
    assert_eq!(error.to_string(), "Agents paused");
}

// ---- AgentControl: what Stop, Stop this agent, Stop all and Pause all do to the Guard (Prompt 39) ----

fn write_by(guard_state: &mut GuardState, who: &str) -> Result<(), WriteError> {
    let mut store = Store::new(document());
    apply_guarded(
        &mut store,
        &mut Ids::default(),
        Transaction::new(Origin::new(by(who)), vec![touch_rect()]),
        &Inputs::default(),
        guard_state,
        NOW,
    )
    .map(|_| ())
}

#[test]
fn control_stop_rejects_only_that_agent_and_frees_its_leases() {
    let mut guard_state = state();
    guard_state
        .leases
        .claim(
            &id("frame00001"),
            agent(TEAL),
            &BlockedPairs::default(),
            NOW,
        )
        .unwrap();
    guard_state.control(&AgentControl::Stop(agent(TEAL)), NOW);
    assert_eq!(
        write_by(&mut guard_state, TEAL),
        Err(WriteError::Rejected(Rejection::Stopped {
            agent: agent(TEAL)
        }))
    );
    assert!(
        guard_state.leases.live(NOW).next().is_none(),
        "a stopped agent holds nothing"
    );
    write_by(&mut guard_state, ROSE).expect("the other agent goes on");

    guard_state.control(&AgentControl::Resume(agent(TEAL)), NOW);
    write_by(&mut guard_state, TEAL).expect("its next turn resumes it");
}

#[test]
fn control_pause_all_and_resume_all() {
    let mut guard_state = state();
    guard_state.control(&AgentControl::PauseAll, NOW);
    for who in [TEAL, ROSE] {
        assert_eq!(
            write_by(&mut guard_state, who),
            Err(WriteError::Rejected(Rejection::AllPaused))
        );
    }
    guard_state.control(&AgentControl::ResumeAll, NOW);
    write_by(&mut guard_state, TEAL).expect("resumed");

    guard_state.control(&AgentControl::Pause(agent(ROSE)), NOW);
    assert_eq!(
        write_by(&mut guard_state, ROSE),
        Err(WriteError::Rejected(Rejection::Paused {
            agent: agent(ROSE)
        }))
    );
    write_by(&mut guard_state, TEAL).expect("only Rose is paused");
}
