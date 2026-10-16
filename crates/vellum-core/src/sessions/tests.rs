//! TEST DATA throughout.
use super::*;
use crate::parallel::{Axis, LaneEnd, LaneRecord, Mode, OptionRecord, ParallelEvent};
use crate::ports::{ChatRole, DocKey, ImageRef, NewSession, SessionMeta, StepOutcome, StepRecord};
use crate::turns::SessionDot;
use serde_json::json;
use std::cell::RefCell;
use vellum_types::{Actor, AgentId, DeviceId, Subsystem};

const SESSION: &str = "01J0000000000000000000000S";
const OTHER: &str = "01J0000000000000000000000T";
const TURN: &str = "01J0000000000000000000000V";
const AGENT: &str = "01J0000000000000000000000A";

/// Just enough store to drive the helpers; the real contract is `vellum_contracts::suites::sessions`.
#[derive(Default)]
struct VecStore {
    events: RefCell<Vec<SessionEvent>>,
    images: RefCell<Vec<(ImageRef, Vec<u8>)>>,
}
impl SessionStore for VecStore {
    fn create(&self, _: NewSession) -> Result<(), StoreError> {
        Ok(())
    }
    fn sessions(&self, _: &DocKey) -> Result<Vec<SessionMeta>, StoreError> {
        Ok(vec![])
    }
    fn session(&self, _: &SessionId) -> Result<Option<SessionMeta>, StoreError> {
        Ok(None)
    }
    fn rename(&self, _: &SessionId, _: &str) -> Result<(), StoreError> {
        Ok(())
    }
    fn append(&self, session: &SessionId, event: NewEvent) -> Result<EventSeq, StoreError> {
        check_event(session, &engine(), &event)?;
        let mut events = self.events.borrow_mut();
        let seq = EventSeq(events.len() as u64 + 1);
        events.push(SessionEvent {
            seq,
            at: event.at,
            origin: event.origin,
            body: event.body,
        });
        Ok(seq)
    }
    fn events(&self, _: &SessionId, _: Option<EventSeq>) -> Result<Vec<SessionEvent>, StoreError> {
        Ok(self.events.borrow().clone())
    }
    fn put_image(&self, bytes: &[u8]) -> Result<ImageRef, StoreError> {
        let name = ImageRef::new(format!("{:064x}", bytes.len())).unwrap();
        self.images
            .borrow_mut()
            .push((name.clone(), bytes.to_vec()));
        Ok(name)
    }
    fn image(&self, image: &ImageRef) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .images
            .borrow()
            .iter()
            .find(|(name, _)| name == image)
            .map(|(_, bytes)| bytes.clone()))
    }
    fn delete(&self, _: &SessionId) -> Result<(), StoreError> {
        Ok(())
    }
}

fn engine() -> SessionKind {
    SessionKind::Engine {
        engine: "builtin".into(),
    }
}
fn session() -> SessionId {
    SessionId::new(SESSION).unwrap()
}
fn turn() -> TurnId {
    TurnId::new(TURN).unwrap()
}
fn person() -> Origin {
    Origin::new(Actor::Person {
        device: DeviceId::new("01J0000000000000000000000D").unwrap(),
    })
    .in_session(session())
    .in_turn(turn())
}
fn agent() -> Origin {
    Origin::new(Actor::Agent {
        agent: AgentId::new("01J0000000000000000000000A").unwrap(),
    })
    .in_session(session())
    .in_turn(turn())
}
fn system() -> Origin {
    Origin::new(Actor::System {
        subsystem: Subsystem::ALL[0],
    })
    .in_session(session())
    .in_turn(turn())
}
fn hash(c: char) -> ImageRef {
    ImageRef::new(c.to_string().repeat(64)).unwrap()
}
fn step() -> StepRecord {
    StepRecord {
        tool: "insert_nodes".into(),
        arguments: r#"{"nodes":[]}"#.into(),
        result: r#"{"ok":true}"#.into(),
        thumbnail: Some(hash('a')),
        started_at: Timestamp(10),
        duration_ms: 25,
        outcome: StepOutcome::Done,
        detail: json!({"counts": {"created": 3}}),
    }
}

#[test]
fn previews_cut_with_a_marker() {
    let value = json!({"text": "é".repeat(50)});
    let whole = value.to_string();
    assert_eq!(preview(&value, 1_000), whole);
    let cut = preview(&value, 10);
    let total = whole.chars().count();
    assert_eq!(
        cut,
        format!(
            "{}… [{} more characters]",
            whole.chars().take(10).collect::<String>(),
            total - 10
        )
    );
}

#[test]
fn tokens_by_kind_from_provider_usage() {
    // input counts cache reads and writes; the uncached part is what remains.
    let usage = ModelUsage {
        input_tokens: Some(1_000),
        output_tokens: Some(200),
        reasoning_tokens: Some(50),
        cached_input_tokens: Some(600),
        cache_write_tokens: Some(100),
    };
    assert_eq!(
        tokens_of(&usage),
        Tokens {
            input: 300,
            output: 200,
            cache_read: 600,
            cache_write: 100
        }
    );
    assert_eq!(tokens_of(&ModelUsage::default()), Tokens::default());
    // A provider that reports more cached than total input does not underflow.
    let odd = ModelUsage {
        input_tokens: Some(5),
        cached_input_tokens: Some(9),
        ..ModelUsage::default()
    };
    assert_eq!(tokens_of(&odd).input, 0);
}

#[test]
fn usage_records_carry_an_estimate_only_when_priced() {
    let pricing = Pricing::from_json(
        r#"{"version":1,"checked":"2026-10-16","models":[{"id":"m","input":1000000,"output":2000000}]}"#,
    )
    .unwrap();
    let usage = ModelUsage {
        input_tokens: Some(1_000),
        output_tokens: Some(1_000),
        ..ModelUsage::default()
    };
    assert_eq!(
        usage_record(Some(&pricing), "m", &usage).estimate,
        Some(3_000)
    );
    assert_eq!(usage_record(Some(&pricing), "x", &usage).estimate, None);
    assert_eq!(usage_record(None, "m", &usage).estimate, None);
    let cached = ModelUsage {
        cached_input_tokens: Some(10),
        input_tokens: Some(10),
        ..ModelUsage::default()
    };
    // "m" has no cache-read price.
    assert_eq!(usage_record(Some(&pricing), "m", &cached).estimate, None);
}

#[test]
fn totals_add_up_by_kind_and_model() {
    let event = |seq, model: &str, tokens: Tokens, estimate| SessionEvent {
        seq: EventSeq(seq),
        at: Timestamp(0),
        origin: agent(),
        body: EventBody::Usage(UsageRecord {
            model: model.into(),
            tokens,
            estimate,
        }),
    };
    let t = |input, output, cache_read, cache_write| Tokens {
        input,
        output,
        cache_read,
        cache_write,
    };
    let events = vec![
        event(1, "a", t(10, 1, 100, 5), Some(40)),
        SessionEvent {
            seq: EventSeq(2),
            at: Timestamp(0),
            origin: agent(),
            body: EventBody::Step(step()),
        },
        event(3, "b", t(20, 2, 0, 0), None),
        event(4, "a", t(1, 1, 1, 1), Some(2)),
    ];
    let totals = totals(&events);
    assert_eq!(totals.tokens, t(31, 4, 101, 6));
    assert_eq!(totals.by_model["a"], t(11, 2, 101, 6));
    assert_eq!(totals.by_model["b"], t(20, 2, 0, 0));
    assert_eq!(totals.estimate, 42);
    assert_eq!(totals.unpriced, 1);
}

fn bodies() -> Vec<EventBody> {
    vec![
        EventBody::Prompt {
            text: "Add a pricing card".into(),
            context: vec!["frame:Hero".into()],
            images: vec![hash('b')],
        },
        EventBody::Messages(vec![
            StoredMessage {
                role: ChatRole::User,
                parts: vec![
                    StoredPart::Text("hi".into()),
                    StoredPart::Image {
                        mime: "image/png".into(),
                        image: hash('c'),
                    },
                ],
            },
            StoredMessage {
                role: ChatRole::Assistant,
                parts: vec![
                    StoredPart::ToolCall {
                        id: "c1".into(),
                        name: "get_page".into(),
                        arguments: json!({"page": "p"}),
                        metadata: json!(null),
                    },
                    StoredPart::ToolResult {
                        id: "c1".into(),
                        name: "get_page".into(),
                        result: json!({"kind": "not_found"}),
                        failed: true,
                    },
                    StoredPart::Continuation(json!({"signature": "s"})),
                ],
            },
        ]),
        EventBody::Plan(crate::plans::PlanEvent::Ticked(vec![1, 2])),
        EventBody::Parallel(ParallelEvent::Lanes {
            mode: Mode::Explore(Axis::Style),
            lanes: vec![
                LaneRecord {
                    agent: vellum_types::AgentId::new(AGENT).unwrap(),
                    name: "TEST DATA lane".into(),
                    model: "m".into(),
                    items: vec![1, 3],
                    task: None,
                },
                LaneRecord {
                    agent: vellum_types::AgentId::new(AGENT).unwrap(),
                    name: "TEST DATA helper".into(),
                    model: "m2".into(),
                    items: vec![],
                    task: Some("TEST DATA: icons".into()),
                },
            ],
        }),
        EventBody::Parallel(ParallelEvent::LaneEnded {
            agent: vellum_types::AgentId::new(AGENT).unwrap(),
            end: LaneEnd::Stopped,
        }),
        EventBody::Parallel(ParallelEvent::Option(OptionRecord {
            option: 2,
            agent: vellum_types::AgentId::new(AGENT).unwrap(),
            frame: vellum_types::NodeId::new("frameOptB1").unwrap(),
            label: Some("TEST DATA: serif headings".into()),
        })),
        EventBody::Parallel(ParallelEvent::Option(OptionRecord {
            option: 1,
            agent: vellum_types::AgentId::new(AGENT).unwrap(),
            frame: vellum_types::NodeId::new("frameOptA1").unwrap(),
            label: None,
        })),
        EventBody::Undo {
            changed: 47,
            kept: 2,
        },
        EventBody::Step(step()),
        EventBody::Step(StepRecord {
            thumbnail: None,
            outcome: StepOutcome::Rejected,
            ..step()
        }),
        EventBody::Usage(UsageRecord {
            model: "m".into(),
            tokens: Tokens {
                input: 1,
                output: 2,
                cache_read: 3,
                cache_write: 4,
            },
            estimate: Some(7),
        }),
        EventBody::Usage(UsageRecord {
            model: "m".into(),
            tokens: Tokens::default(),
            estimate: None,
        }),
        EventBody::Turn(TurnTransition {
            turn: turn(),
            from: TurnState::Running,
            input: TurnInput::PermissionRequested,
            to: TurnState::AwaitingPermission,
        }),
    ]
}

#[test]
fn every_body_round_trips_through_json() {
    for body in bodies() {
        let json = body_to_json(&body);
        assert_eq!(body_from_json(&json).unwrap(), body, "{json}");
        // And through text, as a store keeps it.
        let text = json.to_string();
        assert_eq!(
            body_from_json(&serde_json::from_str(&text).unwrap()).unwrap(),
            body
        );
    }
    for origin in [person(), agent(), system(), Origin::new(person().actor)] {
        assert_eq!(origin_from_json(origin_to_json(&origin)).unwrap(), origin);
    }
}

#[test]
fn reading_refuses_what_this_version_did_not_write() {
    for bad in [
        json!({"type": "chat"}),
        json!({"text": "no type"}),
        json!({"type": "plan", "text": "x", "extra": 1}),
        json!({"type": "plan"}),
        json!({"type": "step", "tool": "t", "arguments": "", "result": "", "startedAt": 0,
               "durationMs": 0, "outcome": "maybe", "detail": null}),
        json!({"type": "prompt", "text": "", "context": [], "images": ["nothex"]}),
        json!({"type": "usage", "model": "m", "tokens": {"input": -1, "output": 0, "cacheRead": 0, "cacheWrite": 0}}),
        json!({"type": "turn", "turn": TURN, "from": "running", "input": "explode", "to": "done"}),
        json!({"type": "turn", "turn": "x", "from": "running", "input": "done", "to": "done"}),
        json!({"type": "messages", "messages": [{"role": "system", "parts": []}]}),
        json!({"type": "messages", "messages": [{"role": "user", "parts": [{"text": "a", "image": {}}]}]}),
        json!({"type": "parallel", "event": "lanes", "mode": "spiral", "lanes": []}),
        json!({"type": "parallel", "event": "laneEnded", "agent": AGENT, "end": "paused"}),
        json!({"type": "parallel", "event": "option", "option": 1, "agent": AGENT, "frame": "f1", "extra": 1}),
        json!({"type": "undo", "changed": 1}),
    ] {
        assert!(
            matches!(body_from_json(&bad), Err(StoreError::Corrupt(_))),
            "{bad}"
        );
    }
}

#[test]
fn events_must_belong_to_their_session_and_turn() {
    let event = |origin: Origin, body| NewEvent {
        at: Timestamp(0),
        origin,
        body,
    };
    let s = session();
    assert_eq!(
        check_event(&s, &engine(), &event(agent(), EventBody::Step(step()))),
        Ok(())
    );
    let elsewhere = agent().in_session(SessionId::new(OTHER).unwrap());
    assert_eq!(
        check_event(&s, &engine(), &event(elsewhere, EventBody::Step(step()))),
        Err(StoreError::WrongSession)
    );
    let mut no_session = agent();
    no_session.session = None;
    assert_eq!(
        check_event(&s, &engine(), &event(no_session, EventBody::Step(step()))),
        Err(StoreError::WrongSession)
    );
    let mut no_turn = person();
    no_turn.turn = None;
    let change = EventBody::Turn(TurnTransition {
        turn: turn(),
        from: TurnState::Queued,
        input: TurnInput::Start,
        to: TurnState::Planning,
    });
    assert_eq!(
        check_event(&s, &engine(), &event(no_turn, change)),
        Err(StoreError::WrongSession)
    );
}

#[test]
fn external_sessions_record_tool_calls_only() {
    let external = SessionKind::External {
        client: "Claude Code".into(),
    };
    for body in bodies() {
        let event = NewEvent {
            at: Timestamp(0),
            origin: agent(),
            body: body.clone(),
        };
        let expected = if matches!(body, EventBody::Step(_)) {
            Ok(())
        } else {
            Err(StoreError::NotRecorded)
        };
        assert_eq!(
            check_event(&session(), &external, &event),
            expected,
            "{body:?}"
        );
        assert_eq!(check_event(&session(), &engine(), &event), Ok(()));
    }
}

#[test]
fn history_comes_back_with_its_pictures() {
    let store = VecStore::default();
    let messages = vec![
        ChatMessage {
            role: ChatRole::User,
            content: vec![
                ChatPart::Text("Build a card".into()),
                ChatPart::Image {
                    mime: "image/png".into(),
                    base64: "iVBORw0KGgo=".into(),
                },
            ],
        },
        ChatMessage {
            role: ChatRole::Assistant,
            content: vec![
                ChatPart::ToolCall(ModelCall {
                    id: "c1".into(),
                    name: "screenshot".into(),
                    arguments: json!({}),
                    metadata: json!({"thoughtSignature": "x"}),
                }),
                ChatPart::Continuation(json!(1)),
            ],
        },
    ];
    let stored = store_messages(&store, &messages).unwrap();
    // The picture is in the cache, not the event.
    assert!(matches!(&stored[0].parts[1], StoredPart::Image { .. }));
    assert_eq!(store.images.borrow()[0].1, b"\x89PNG\r\n\x1a\n");
    store
        .append(
            &session(),
            NewEvent {
                at: Timestamp(1),
                origin: agent(),
                body: EventBody::Messages(stored),
            },
        )
        .unwrap();
    store
        .append(
            &session(),
            NewEvent {
                at: Timestamp(2),
                origin: agent(),
                body: EventBody::Step(step()),
            },
        )
        .unwrap();
    let events = store.events(&session(), None).unwrap();
    assert_eq!(history(&store, &events).unwrap(), messages);

    store.images.borrow_mut().clear();
    assert!(matches!(
        history(&store, &events),
        Err(StoreError::MissingImage(_))
    ));
    let bad = vec![ChatMessage {
        role: ChatRole::User,
        content: vec![ChatPart::Image {
            mime: "image/png".into(),
            base64: "not base64!".into(),
        }],
    }];
    assert!(matches!(
        store_messages(&store, &bad),
        Err(StoreError::Corrupt(_))
    ));
}

#[test]
fn every_transition_appends_one_event_with_its_actor() {
    let store = VecStore::default();
    let steps = [
        (person(), TurnInput::Start, TurnState::Planning),
        (agent(), TurnInput::PlanNeeded, TurnState::AwaitingPlan),
        (person(), TurnInput::Build, TurnState::Running),
        (
            agent(),
            TurnInput::PermissionRequested,
            TurnState::AwaitingPermission,
        ),
        (system(), TurnInput::Decided, TurnState::Running),
        (agent(), TurnInput::Done, TurnState::Done),
    ];
    for (index, (origin, input, to)) in steps.iter().enumerate() {
        let (state, seq) =
            record_transition(&store, origin, *input, Timestamp(index as i64)).unwrap();
        assert_eq!(state, *to);
        assert_eq!(seq, EventSeq(index as u64 + 1));
        let events = store.events(&session(), None).unwrap();
        assert_eq!(events.len(), index + 1, "one event per transition");
        let last = events.last().unwrap();
        assert_eq!(last.origin.actor, origin.actor);
        assert!(matches!(&last.body, EventBody::Turn(c) if c.input == *input && c.to == *to));
    }
    let events = store.events(&session(), None).unwrap();
    assert_eq!(turn_state(&events, &turn()), Some(TurnState::Done));
    assert_eq!(turn_state(&events, &TurnId::new(OTHER).unwrap()), None);
    assert_eq!(TurnState::Done.dot(), SessionDot::Done);

    // Terminal: nothing more is appended.
    assert_eq!(
        record_transition(&store, &person(), TurnInput::Stop, Timestamp(9)),
        Err(TransitionError::Rejected(TurnRejection::Terminal))
    );
    assert_eq!(store.events(&session(), None).unwrap().len(), steps.len());
}

#[test]
fn rejected_transitions_append_nothing() {
    let store = VecStore::default();
    record_transition(&store, &person(), TurnInput::Start, Timestamp(0)).unwrap();
    for (origin, input, why) in [
        (agent(), TurnInput::Stop, TurnRejection::NotAPerson),
        (system(), TurnInput::Stop, TurnRejection::NotAPerson),
        (agent(), TurnInput::Done, TurnRejection::NotNow),
    ] {
        assert_eq!(
            record_transition(&store, &origin, input, Timestamp(1)),
            Err(TransitionError::Rejected(why))
        );
    }
    assert_eq!(store.events(&session(), None).unwrap().len(), 1);
    let (state, _) = record_transition(&store, &person(), TurnInput::Stop, Timestamp(2)).unwrap();
    assert_eq!(state, TurnState::Stopped);
    let mut no_turn = person();
    no_turn.turn = None;
    assert_eq!(
        record_transition(&store, &no_turn, TurnInput::Start, Timestamp(3)),
        Err(TransitionError::Store(StoreError::WrongSession))
    );
}
