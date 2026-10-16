//! The `PermissionBroker` contract (ARCHITECTURE §7.7, guide Invariant 11). `run` takes a factory that builds a
//! broker over the store, settings and prompt it is given, so each rule starts fresh. TEST DATA throughout.
use super::model::block_on;
use crate::fakes::{ManualClock, MemorySessions, ScriptedPrompt};
use std::rc::Rc;
use vellum_core::permissions::{Effect, PermissionSettings, Permit, Rule};
use vellum_core::ports::*;
use vellum_types::{Actor, AgentId, Origin, SessionId};

/// What a factory gets.
pub struct Parts {
    pub store: Rc<dyn SessionStore>,
    pub clock: Rc<dyn Clock>,
    pub settings: PermissionSettings,
    pub prompt: Option<Rc<dyn PermissionPrompt>>,
}

pub const ROOT: &str = "/TEST DATA/juniper";

pub fn run<B: PermissionBroker>(make: impl Fn(Parts) -> B) {
    allow_once_is_recorded_and_exact(&make);
    deny_is_recorded(&make);
    session_grant_is_asked_once(&make);
    nobody_to_ask_denies(&make);
    skip_all_allows_without_asking(&make);
    deny_rule_beats_skip_all(&make);
    no_session_no_permit(&make);
}

pub fn session() -> SessionId {
    SessionId::new("01J000000000000000000000PS").expect("a ULID")
}
pub fn agent() -> AgentId {
    AgentId::new("01J000000000000000000000PA").expect("a ULID")
}
pub fn origin() -> Origin {
    Origin::new(Actor::Agent { agent: agent() }).in_session(session())
}
/// A store holding one engine session on `ROOT/design/app.vellum`.
pub fn store() -> Rc<MemorySessions> {
    let store = MemorySessions::new();
    store
        .create(NewSession {
            id: session(),
            doc: DocKey::new(ROOT, "design/app.vellum").expect("a key"),
            kind: SessionKind::Engine {
                engine: "builtin".into(),
            },
            agent: agent(),
            title: "TEST DATA".into(),
            created_at: Timestamp(0),
        })
        .expect("a fresh store");
    Rc::new(store)
}
pub fn request(action: Action) -> PermissionRequest {
    PermissionRequest {
        origin: origin(),
        action,
        engine: Some("builtin".into()),
        engine_verdict: None,
    }
}
fn read(path: &str) -> Action {
    Action::ReadFile {
        path: format!("{ROOT}/{path}"),
    }
}

struct Setup {
    store: Rc<MemorySessions>,
    prompt: Rc<ScriptedPrompt>,
}

fn setup<B: PermissionBroker>(
    make: &impl Fn(Parts) -> B,
    settings: PermissionSettings,
    prompt: Option<ScriptedPrompt>,
) -> (B, Setup) {
    let store = store();
    let present = prompt.is_some();
    let prompt = Rc::new(prompt.unwrap_or_else(|| ScriptedPrompt::always(Answer::AllowOnce)));
    let broker = make(Parts {
        store: store.clone(),
        clock: Rc::new(ManualClock::new(1_000)),
        settings,
        prompt: present.then(|| prompt.clone() as Rc<dyn PermissionPrompt>),
    });
    (broker, Setup { store, prompt })
}

fn decisions(store: &MemorySessions) -> Vec<(EventSeq, PermissionRecord)> {
    store
        .events(&session(), None)
        .expect("the session exists")
        .into_iter()
        .filter_map(|event| match event.body {
            EventBody::Permission(record) => Some((event.seq, record)),
            _ => None,
        })
        .collect()
}

/// The record a permit points at says this action was allowed.
pub fn check_permit(store: &dyn SessionStore, permit: &Permit, action: &Action) {
    assert!(
        permit.covers(action),
        "PermissionBroker: a permit covers its action"
    );
    let events = store.events(&session(), None).expect("the session exists");
    let event = events
        .iter()
        .find(|e| e.seq == permit.record())
        .expect("PermissionBroker: a permit points at its record");
    match &event.body {
        EventBody::Permission(record) => {
            assert!(
                record.allowed(),
                "PermissionBroker: a permit's record allows"
            );
            assert_eq!(
                &record.action, action,
                "PermissionBroker: the record names the action"
            );
        }
        other => panic!("PermissionBroker: a permit points at {other:?}"),
    }
    assert_eq!(
        event.origin,
        *permit.origin(),
        "PermissionBroker: the record keeps the origin"
    );
}

fn allow_once_is_recorded_and_exact<B: PermissionBroker>(make: &impl Fn(Parts) -> B) {
    let (broker, s) = setup(
        make,
        PermissionSettings::default(),
        Some(ScriptedPrompt::always(Answer::AllowOnce)),
    );
    let action = read("DESIGN.md");
    let permit = block_on(broker.request(request(action.clone())))
        .expect("PermissionBroker: Allow once allows");
    check_permit(&*s.store, &permit, &action);
    assert!(
        !permit.covers(&read("README.md")),
        "PermissionBroker: a permit covers only its action"
    );
    block_on(broker.request(request(action))).expect("allowed again");
    assert_eq!(
        s.prompt.asked().len(),
        2,
        "PermissionBroker: Allow once asks again next time"
    );
    assert_eq!(
        decisions(&s.store).len(),
        2,
        "PermissionBroker: every decision is recorded"
    );
}

fn deny_is_recorded<B: PermissionBroker>(make: &impl Fn(Parts) -> B) {
    let (broker, s) = setup(
        make,
        PermissionSettings::default(),
        Some(ScriptedPrompt::always(Answer::Deny)),
    );
    let refusal = block_on(broker.request(request(read("DESIGN.md"))))
        .expect_err("PermissionBroker: Deny denies");
    let recorded = decisions(&s.store);
    assert_eq!(recorded.len(), 1, "PermissionBroker: a denial is recorded");
    assert_eq!(
        refusal.seq,
        Some(recorded[0].0),
        "PermissionBroker: the refusal names its record"
    );
    assert_eq!(recorded[0].1.answer, Answer::Deny);
}

fn session_grant_is_asked_once<B: PermissionBroker>(make: &impl Fn(Parts) -> B) {
    let (broker, s) = setup(
        make,
        PermissionSettings::default(),
        Some(ScriptedPrompt::new(
            [Some(Answer::AllowForSession)],
            Some(Answer::Deny),
        )),
    );
    for path in ["DESIGN.md", "src/app.css", "docs/spec.md"] {
        let action = read(path);
        let permit = block_on(broker.request(request(action.clone())))
            .expect("PermissionBroker: Allow for this session covers repo reads");
        check_permit(&*s.store, &permit, &action);
    }
    assert_eq!(
        s.prompt.asked().len(),
        1,
        "PermissionBroker: a repo read is asked once per session"
    );
    assert_eq!(
        decisions(&s.store).len(),
        3,
        "PermissionBroker: grants are recorded per use"
    );
}

fn nobody_to_ask_denies<B: PermissionBroker>(make: &impl Fn(Parts) -> B) {
    let (broker, s) = setup(make, PermissionSettings::default(), None);
    let refusal = block_on(broker.request(request(read("DESIGN.md"))))
        .expect_err("PermissionBroker: with no prompt and no rule, deny");
    assert_eq!(refusal.record.source, DecisionSource::Unattended);
    assert_eq!(decisions(&s.store).len(), 1);
}

fn skip_all_allows_without_asking<B: PermissionBroker>(make: &impl Fn(Parts) -> B) {
    let (broker, s) = setup(
        make,
        PermissionSettings {
            skip_all: true,
            ..Default::default()
        },
        Some(ScriptedPrompt::always(Answer::Deny)),
    );
    let action = Action::Network {
        url: "https://example.com".into(),
    };
    let permit = block_on(broker.request(request(action.clone())))
        .expect("PermissionBroker: skip-all allows");
    check_permit(&*s.store, &permit, &action);
    assert!(
        s.prompt.asked().is_empty(),
        "PermissionBroker: skip-all never asks"
    );
    assert_eq!(decisions(&s.store)[0].1.source, DecisionSource::SkipAll);
}

fn deny_rule_beats_skip_all<B: PermissionBroker>(make: &impl Fn(Parts) -> B) {
    let (broker, s) = setup(
        make,
        PermissionSettings {
            skip_all: true,
            rules: vec![Rule::parse(Effect::Deny, "read .env*").expect("a rule")],
            ..Default::default()
        },
        None,
    );
    let refusal = block_on(broker.request(request(read(".env.local"))))
        .expect_err("PermissionBroker: a deny rule wins over skip-all");
    assert!(matches!(refusal.record.source, DecisionSource::Rule { .. }));
    assert_eq!(decisions(&s.store).len(), 1);
}

fn no_session_no_permit<B: PermissionBroker>(make: &impl Fn(Parts) -> B) {
    let (broker, s) = setup(
        make,
        PermissionSettings {
            skip_all: true,
            ..Default::default()
        },
        None,
    );
    let mut anonymous = request(read("DESIGN.md"));
    anonymous.origin = Origin::new(Actor::Agent { agent: agent() });
    let refusal = block_on(broker.request(anonymous))
        .expect_err("PermissionBroker: nothing is allowed without a session to record it in");
    assert_eq!(refusal.seq, None);
    assert!(decisions(&s.store).is_empty());
}
