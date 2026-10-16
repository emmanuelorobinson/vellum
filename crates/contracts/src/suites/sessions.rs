//! The `SessionStore` contract (ARCHITECTURE §7.6, ADR-006). `run` takes a factory and gives every rule a fresh,
//! empty store. TEST DATA throughout.
use serde_json::json;
use vellum_core::plans::{Placeholder, Plan, PlanEvent, PlanItem};
use vellum_core::ports::*;
use vellum_core::turns::{TurnInput, TurnState};
use vellum_types::{Actor, AgentId, DeviceId, Origin, SessionId, TurnId};

pub fn run<S: SessionStore>(make: impl Fn() -> S) {
    appends_in_order(&make());
    round_trips_every_event(&make());
    lists_by_document(&make());
    refuses_bad_appends(&make());
    renames(&make());
    caches_pictures_by_content(&make());
    delete_cascades(&make());
}

fn id(n: u32) -> String {
    format!("01J{n:023}")
}
pub fn session_id(n: u32) -> SessionId {
    SessionId::new(id(n)).expect("a ULID")
}
fn turn() -> TurnId {
    TurnId::new(id(900)).expect("a ULID")
}
fn agent() -> AgentId {
    AgentId::new(id(800)).expect("a ULID")
}
pub fn doc(path: &str) -> DocKey {
    DocKey::new("/TEST DATA/juniper", path).expect("a key")
}
pub fn new_session(n: u32, doc: DocKey, kind: SessionKind, at: i64) -> NewSession {
    NewSession {
        id: session_id(n),
        doc,
        kind,
        agent: agent(),
        title: format!("TEST DATA session {n}"),
        created_at: Timestamp(at),
    }
}
fn builtin() -> SessionKind {
    SessionKind::Engine {
        engine: "builtin".into(),
    }
}
fn by_agent(session: &SessionId) -> Origin {
    Origin::new(Actor::Agent { agent: agent() })
        .in_session(session.clone())
        .in_turn(turn())
}
fn by_person(session: &SessionId) -> Origin {
    Origin::new(Actor::Person {
        device: DeviceId::new(id(700)).expect("a ULID"),
    })
    .in_session(session.clone())
    .in_turn(turn())
}
fn event(origin: Origin, at: i64, body: EventBody) -> NewEvent {
    NewEvent {
        at: Timestamp(at),
        origin,
        body,
    }
}
fn step(thumbnail: Option<ImageRef>) -> EventBody {
    EventBody::Step(StepRecord {
        tool: "insert_nodes".into(),
        arguments: r#"{"nodes":[]}"#.into(),
        result: r#"{"created":["a1"]}"#.into(),
        thumbnail,
        started_at: Timestamp(5),
        duration_ms: 40,
        outcome: StepOutcome::Done,
        detail: json!({"verb": "designed"}),
    })
}

fn appends_in_order(store: &dyn SessionStore) {
    let s = session_id(1);
    store
        .create(new_session(1, doc("a.vellum"), builtin(), 100))
        .unwrap();
    for n in 1..=5 {
        let seq = store
            .append(&s, event(by_agent(&s), 100 + n, step(None)))
            .unwrap();
        assert_eq!(
            seq,
            EventSeq(n as u64),
            "SessionStore: places start at 1 with no gaps"
        );
    }
    let all = store.events(&s, None).unwrap();
    assert_eq!(
        all.iter().map(|e| e.seq.0).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5],
        "SessionStore: events oldest first"
    );
    let later = store.events(&s, Some(EventSeq(3))).unwrap();
    assert_eq!(
        later.iter().map(|e| e.seq.0).collect::<Vec<_>>(),
        [4, 5],
        "SessionStore: events since a place"
    );
    assert!(store.events(&s, Some(EventSeq(5))).unwrap().is_empty());
    let meta = store
        .session(&s)
        .unwrap()
        .expect("SessionStore: session after create");
    assert_eq!(meta.events, 5, "SessionStore: event count");
    assert_eq!(
        meta.updated_at,
        Timestamp(105),
        "SessionStore: updated at the newest event"
    );
}

fn round_trips_every_event(store: &dyn SessionStore) {
    let s = session_id(2);
    store
        .create(new_session(2, doc("a.vellum"), builtin(), 0))
        .unwrap();
    let picture = store.put_image(b"TEST DATA picture").unwrap();
    let bodies = vec![
        EventBody::Prompt {
            text: "Add a pricing card".into(),
            context: vec!["@DESIGN.md".into(), "node:Hero".into()],
            images: vec![picture.clone()],
        },
        EventBody::Plan(PlanEvent::Proposed(sample_plan())),
        EventBody::Plan(PlanEvent::Built {
            plan: sample_plan(),
            placeholders: vec![Placeholder {
                item: 1,
                frame: vellum_types::NodeId::new("plannedHero1").unwrap(),
                name: "Hero".into(),
                page: vellum_types::PageId::new("pageHome1").unwrap(),
                bounds: Rect::new(0.0, 0.0, 1440.0, 900.0),
                agent: AgentId::new("01J00000000000000000000002").unwrap(),
                turn: TurnId::new("01J00000000000000000000003").unwrap(),
            }],
        }),
        EventBody::Plan(PlanEvent::Ticked(vec![1])),
        step(Some(picture.clone())),
        EventBody::Usage(UsageRecord {
            model: "m".into(),
            tokens: Tokens {
                input: 1,
                output: 2,
                cache_read: 3,
                cache_write: 4,
            },
            estimate: Some(10),
        }),
        EventBody::Messages(vec![StoredMessage {
            role: ChatRole::Assistant,
            parts: vec![
                StoredPart::Text("Done.".into()),
                StoredPart::Image {
                    mime: "image/png".into(),
                    image: picture.clone(),
                },
                StoredPart::ToolCall {
                    id: "c".into(),
                    name: "screenshot".into(),
                    arguments: json!({"node": "a1", "scale": 0.5}),
                    metadata: json!(null),
                },
            ],
        }]),
        EventBody::Turn(TurnTransition {
            turn: turn(),
            from: TurnState::Queued,
            input: TurnInput::Start,
            to: TurnState::Planning,
        }),
        EventBody::Permission(PermissionRecord {
            action: Action::RunCommand {
                program: "pnpm".into(),
                args: vec!["build".into(), "--filter".into(), "app".into()],
                cwd: "/TEST DATA/juniper".into(),
            },
            kind: PermissionKind::RunCommand,
            engine: Some("claude-code".into()),
            answer: Answer::AllowForSession,
            source: DecisionSource::Person,
        }),
        EventBody::Permission(PermissionRecord {
            action: Action::ReadFile {
                path: "/TEST DATA/juniper/.env".into(),
            },
            kind: PermissionKind::ReadRepo,
            engine: None,
            answer: Answer::Deny,
            source: DecisionSource::Rule {
                rule: "deny read .env*".into(),
            },
        }),
    ];
    for (n, body) in bodies.iter().enumerate() {
        let origin = if n == 0 { by_person(&s) } else { by_agent(&s) };
        store
            .append(&s, event(origin, n as i64, body.clone()))
            .unwrap();
    }
    let read = store.events(&s, None).unwrap();
    assert_eq!(read.len(), bodies.len());
    for (event, body) in read.iter().zip(&bodies) {
        assert_eq!(
            &event.body, body,
            "SessionStore: an event reads back as written"
        );
    }
    assert!(
        read[0].origin.actor.is_person(),
        "SessionStore: the origin is kept"
    );
    assert_eq!(
        read[1].origin,
        by_agent(&s),
        "SessionStore: the origin is kept"
    );
    assert_eq!(read[3].at, Timestamp(3), "SessionStore: the time is kept");
}

fn lists_by_document(store: &dyn SessionStore) {
    store
        .create(new_session(10, doc("a.vellum"), builtin(), 10))
        .unwrap();
    store
        .create(new_session(
            11,
            doc("a.vellum"),
            SessionKind::External {
                client: "Claude Code".into(),
            },
            20,
        ))
        .unwrap();
    store
        .create(new_session(12, doc("b/a.vellum"), builtin(), 30))
        .unwrap();
    let elsewhere = DocKey::new("/TEST DATA/other", "a.vellum").unwrap();
    store
        .create(new_session(13, elsewhere.clone(), builtin(), 40))
        .unwrap();
    assert_eq!(
        store.create(new_session(10, doc("a.vellum"), builtin(), 50)),
        Err(StoreError::Exists),
        "SessionStore: one session per id"
    );
    let ids = |doc: &DocKey| {
        store
            .sessions(doc)
            .unwrap()
            .into_iter()
            .map(|m| m.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(&doc("a.vellum")),
        [session_id(11), session_id(10)],
        "SessionStore: a document's sessions, newest first"
    );
    // Activity moves a session to the front.
    let s = session_id(10);
    store
        .append(&s, event(by_agent(&s), 60, step(None)))
        .unwrap();
    assert_eq!(
        ids(&doc("a.vellum")),
        [session_id(10), session_id(11)],
        "SessionStore: newest activity first"
    );
    assert_eq!(ids(&doc("b/a.vellum")), [session_id(12)]);
    assert_eq!(
        ids(&elsewhere),
        [session_id(13)],
        "SessionStore: keyed by repository too"
    );
    assert!(ids(&doc("none.vellum")).is_empty());
    let meta = store.session(&session_id(11)).unwrap().unwrap();
    assert_eq!(
        meta.kind,
        SessionKind::External {
            client: "Claude Code".into()
        }
    );
    assert_eq!(meta.agent, agent());
    assert_eq!(meta.created_at, Timestamp(20));
    assert_eq!(store.session(&session_id(99)).unwrap(), None);
}

fn refuses_bad_appends(store: &dyn SessionStore) {
    let s = session_id(20);
    store
        .create(new_session(20, doc("a.vellum"), builtin(), 0))
        .unwrap();
    let x = session_id(21);
    store
        .create(new_session(
            21,
            doc("a.vellum"),
            SessionKind::External {
                client: "Cursor".into(),
            },
            0,
        ))
        .unwrap();
    let unknown = session_id(29);
    assert_eq!(
        store.append(&unknown, event(by_agent(&unknown), 0, step(None))),
        Err(StoreError::UnknownSession),
        "SessionStore: append to an unknown session"
    );
    assert_eq!(
        store.events(&unknown, None),
        Err(StoreError::UnknownSession),
        "SessionStore: events of an unknown session"
    );
    assert_eq!(
        store.append(&s, event(by_agent(&x), 0, step(None))),
        Err(StoreError::WrongSession),
        "SessionStore: an origin from another session"
    );
    assert_eq!(
        store.append(
            &x,
            event(by_agent(&x), 0, EventBody::Plan(PlanEvent::Abandoned))
        ),
        Err(StoreError::NotRecorded),
        "SessionStore: external sessions keep tool calls and permission decisions only"
    );
    let decision = EventBody::Permission(PermissionRecord {
        action: Action::Network {
            url: "https://example.com".into(),
        },
        kind: PermissionKind::Network,
        engine: None,
        answer: Answer::Deny,
        source: DecisionSource::Unattended,
    });
    assert_eq!(
        store.append(&x, event(by_agent(&x), 0, decision.clone())),
        Ok(EventSeq(1)),
        "SessionStore: external sessions keep permission decisions (Invariant 11)"
    );
    let ghost = ImageRef::new("0".repeat(64)).unwrap();
    assert_eq!(
        store.append(&s, event(by_agent(&s), 0, step(Some(ghost.clone())))),
        Err(StoreError::MissingImage(ghost)),
        "SessionStore: pictures must be cached first"
    );
    // Refusals take no place.
    assert_eq!(
        store.append(&s, event(by_agent(&s), 1, step(None))),
        Ok(EventSeq(1))
    );
    assert_eq!(
        store.append(&x, event(by_agent(&x), 1, step(None))),
        Ok(EventSeq(2))
    );
}

fn renames(store: &dyn SessionStore) {
    let s = session_id(30);
    store
        .create(new_session(30, doc("a.vellum"), builtin(), 0))
        .unwrap();
    store.rename(&s, "Token cleanup").unwrap();
    assert_eq!(store.session(&s).unwrap().unwrap().title, "Token cleanup");
    assert_eq!(
        store.rename(&session_id(39), "x"),
        Err(StoreError::UnknownSession)
    );
}

fn caches_pictures_by_content(store: &dyn SessionStore) {
    let a = store.put_image(b"TEST DATA a").unwrap();
    assert_eq!(a.as_str().len(), 64);
    assert_eq!(
        store.put_image(b"TEST DATA a").unwrap(),
        a,
        "SessionStore: same bytes, same name"
    );
    let b = store.put_image(b"TEST DATA b").unwrap();
    assert_ne!(a, b, "SessionStore: different bytes, different names");
    assert_eq!(
        store.image(&a).unwrap().as_deref(),
        Some(&b"TEST DATA a"[..])
    );
    assert_eq!(
        store.put_image(b"").unwrap().as_str().len(),
        64,
        "SessionStore: empty pictures"
    );
    let none = ImageRef::new("f".repeat(64)).unwrap();
    assert_eq!(store.image(&none).unwrap(), None);
}

fn delete_cascades(store: &dyn SessionStore) {
    let (s, t) = (session_id(40), session_id(41));
    store
        .create(new_session(40, doc("a.vellum"), builtin(), 0))
        .unwrap();
    store
        .create(new_session(41, doc("a.vellum"), builtin(), 0))
        .unwrap();
    let own = store.put_image(b"TEST DATA only in s").unwrap();
    let shared = store.put_image(b"TEST DATA in both").unwrap();
    store
        .append(&s, event(by_agent(&s), 1, step(Some(own.clone()))))
        .unwrap();
    store
        .append(&s, event(by_agent(&s), 2, step(Some(shared.clone()))))
        .unwrap();
    store
        .append(&t, event(by_agent(&t), 3, step(Some(shared.clone()))))
        .unwrap();
    store.delete(&s).unwrap();
    assert_eq!(
        store.session(&s).unwrap(),
        None,
        "SessionStore: delete removes the session"
    );
    assert_eq!(
        store.events(&s, None),
        Err(StoreError::UnknownSession),
        "SessionStore: delete removes its events"
    );
    assert_eq!(
        store.image(&own).unwrap(),
        None,
        "SessionStore: delete removes pictures only it named"
    );
    assert!(
        store.image(&shared).unwrap().is_some(),
        "SessionStore: delete keeps pictures another session names"
    );
    assert_eq!(store.sessions(&doc("a.vellum")).unwrap().len(), 1);
    store.delete(&s).unwrap();
    store.delete(&session_id(49)).unwrap();
    // The id is free again and starts from 1.
    store
        .create(new_session(40, doc("a.vellum"), builtin(), 9))
        .unwrap();
    assert_eq!(
        store.append(&s, event(by_agent(&s), 9, step(None))),
        Ok(EventSeq(1))
    );
    store.delete(&t).unwrap();
    assert_eq!(store.image(&shared).unwrap(), None);
}

/// TEST DATA. A two-item plan.
fn sample_plan() -> Plan {
    Plan {
        title: "TEST DATA: Pricing".into(),
        items: ["Hero", "Pricing"]
            .iter()
            .enumerate()
            .map(|(n, name)| PlanItem {
                id: n as u32 + 1,
                name: (*name).into(),
                page: vellum_types::PageId::new("pageHome1").unwrap(),
                intent: format!("TEST DATA: {name}"),
                width: Some(1440.0),
                height: Some(900.0),
                frame: None,
            })
            .collect(),
        changes: Some(40),
    }
}
