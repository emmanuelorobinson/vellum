//! TEST DATA throughout. The property test over generated agent actions (Invariant 11) is in
//! `crates/contracts/tests/permissions.rs`, which has the real in-memory store.
use super::*;
use crate::ports::{
    DocKey, EventSeq, NewSession, SessionEvent, SessionKind, SessionMeta, StoreError, Timestamp,
};
use crate::sessions::{body_from_json, body_to_json, check_event};
use crate::turns::TurnState;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use vellum_types::{AgentId, DeviceId, SessionId, TurnId};

const ROOT: &str = "/work/juniper";
const SESSION: &str = "01J0000000000000000000000S";
const TURN: &str = "01J0000000000000000000000V";

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
    }
}

struct Store {
    kind: SessionKind,
    events: RefCell<Vec<SessionEvent>>,
    /// Appends fail once this many events exist.
    full_at: Option<usize>,
}
impl Store {
    fn new(kind: SessionKind) -> Self {
        Self {
            kind,
            events: RefCell::default(),
            full_at: None,
        }
    }
    fn bodies(&self) -> Vec<EventBody> {
        self.events
            .borrow()
            .iter()
            .map(|e| e.body.clone())
            .collect()
    }
    fn permissions(&self) -> Vec<PermissionRecord> {
        self.bodies()
            .into_iter()
            .filter_map(|b| match b {
                EventBody::Permission(record) => Some(record),
                _ => None,
            })
            .collect()
    }
}
impl SessionStore for Store {
    fn create(&self, _: NewSession) -> Result<(), StoreError> {
        Ok(())
    }
    fn sessions(&self, _: &DocKey) -> Result<Vec<SessionMeta>, StoreError> {
        Ok(vec![])
    }
    fn session(&self, id: &SessionId) -> Result<Option<SessionMeta>, StoreError> {
        Ok((id == &session()).then(|| SessionMeta {
            id: session(),
            doc: DocKey::new(ROOT, "design/app.vellum").unwrap(),
            kind: self.kind.clone(),
            agent: agent(),
            title: "Settings screen".into(),
            created_at: Timestamp(0),
            updated_at: Timestamp(0),
            events: self.events.borrow().len() as u64,
        }))
    }
    fn rename(&self, _: &SessionId, _: &str) -> Result<(), StoreError> {
        Ok(())
    }
    fn append(&self, id: &SessionId, event: NewEvent) -> Result<EventSeq, StoreError> {
        check_event(id, &self.kind, &event)?;
        let mut events = self.events.borrow_mut();
        if self.full_at.is_some_and(|n| events.len() >= n) {
            return Err(StoreError::Io("disk full".into()));
        }
        // Through the codec, as every real store does.
        let body = body_from_json(&body_to_json(&event.body))?;
        assert_eq!(body, event.body, "the codec keeps the event");
        let seq = EventSeq(events.len() as u64 + 1);
        events.push(SessionEvent {
            seq,
            at: event.at,
            origin: event.origin,
            body,
        });
        Ok(seq)
    }
    fn events(
        &self,
        _: &SessionId,
        since: Option<EventSeq>,
    ) -> Result<Vec<SessionEvent>, StoreError> {
        Ok(self
            .events
            .borrow()
            .iter()
            .filter(|e| since.is_none_or(|s| e.seq > s))
            .cloned()
            .collect())
    }
    fn put_image(&self, _: &[u8]) -> Result<crate::ports::ImageRef, StoreError> {
        Err(StoreError::Io("no pictures here".into()))
    }
    fn image(&self, _: &crate::ports::ImageRef) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(None)
    }
    fn delete(&self, _: &SessionId) -> Result<(), StoreError> {
        Ok(())
    }
}

struct FixedClock;
impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp(1_700_000_000_000)
    }
}

/// Answers from a script; records each question. `during` runs while the question is open.
#[derive(Default)]
struct Scripted {
    answers: RefCell<VecDeque<Option<Answer>>>,
    asked: RefCell<Vec<Question>>,
    during: RefCell<Option<Box<dyn Fn()>>>,
}
impl Scripted {
    fn answering(answers: impl IntoIterator<Item = Option<Answer>>) -> Rc<Self> {
        Rc::new(Self {
            answers: RefCell::new(answers.into_iter().collect()),
            ..Self::default()
        })
    }
}
impl PermissionPrompt for Scripted {
    fn ask<'a>(&'a self, question: Question) -> EngineFuture<'a, Option<Answer>> {
        self.asked.borrow_mut().push(question);
        if let Some(during) = &*self.during.borrow() {
            during();
        }
        let answer = self
            .answers
            .borrow_mut()
            .pop_front()
            .expect("asked more often than the script answers");
        // Pending once, like a person thinking.
        let polled = Cell::new(false);
        Box::pin(std::future::poll_fn(move |cx| {
            if polled.replace(true) {
                Poll::Ready(answer)
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }))
    }
}

fn session() -> SessionId {
    SessionId::new(SESSION).unwrap()
}
fn turn() -> TurnId {
    TurnId::new(TURN).unwrap()
}
fn agent() -> AgentId {
    AgentId::new("01J0000000000000000000000A").unwrap()
}
fn person() -> Actor {
    Actor::Person {
        device: DeviceId::new("01J0000000000000000000000D").unwrap(),
    }
}
fn agent_origin() -> Origin {
    Origin::new(Actor::Agent { agent: agent() }).in_session(session())
}
fn in_turn() -> Origin {
    agent_origin().in_turn(turn())
}
fn engine_kind() -> SessionKind {
    SessionKind::Engine {
        engine: "builtin".into(),
    }
}

fn read(path: &str) -> Action {
    Action::ReadFile { path: path.into() }
}
fn write(path: &str) -> Action {
    Action::WriteFile { path: path.into() }
}
fn run(line: &str) -> Action {
    let mut words = line.split(' ').map(str::to_owned);
    Action::RunCommand {
        program: words.next().unwrap(),
        args: words.collect(),
        cwd: ROOT.into(),
    }
}
fn net(url: &str) -> Action {
    Action::Network { url: url.into() }
}

fn request(origin: Origin, action: Action) -> PermissionRequest {
    PermissionRequest {
        origin,
        action,
        engine: Some("builtin".into()),
        engine_verdict: None,
    }
}

fn broker(settings: PermissionSettings, store: &Rc<Store>, prompt: Option<Rc<Scripted>>) -> Broker {
    Broker::new(
        settings,
        store.clone(),
        Rc::new(FixedClock),
        prompt.map(|p| p as Rc<dyn PermissionPrompt>),
        person(),
    )
}

fn kind_of(action: &Action) -> PermissionKind {
    classify(action, ROOT).unwrap().kind
}

fn verdict(settings: &PermissionSettings, action: &Action) -> Verdict {
    verdict_with(settings, action, None, &[])
}

fn verdict_with(
    settings: &PermissionSettings,
    action: &Action,
    engine: Option<(&str, bool)>,
    events: &[SessionEvent],
) -> Verdict {
    evaluate(
        settings,
        &classify(action, ROOT).unwrap(),
        action,
        engine.map(|(e, _)| e),
        engine.map(|(_, v)| v),
        events,
    )
}

fn allowed_by(source: DecisionSource) -> Verdict {
    Verdict::Decided {
        answer: Answer::AllowOnce,
        source,
    }
}

fn rule(effect: Effect, text: &str) -> Rule {
    Rule::parse(effect, text).unwrap()
}

// ---- classification -------------------------------------------------------------------------------------------

#[test]
fn requests_are_sorted_by_where_they_reach() {
    assert_eq!(
        kind_of(&read("/work/juniper/DESIGN.md")),
        PermissionKind::ReadRepo
    );
    assert_eq!(
        kind_of(&read("/work/juniper/src/../DESIGN.md")),
        PermissionKind::ReadRepo
    );
    assert_eq!(
        kind_of(&read("/work/juniper/../secrets.txt")),
        PermissionKind::ReadOutside
    );
    assert_eq!(
        kind_of(&read("/work/juniper-old/DESIGN.md")),
        PermissionKind::ReadOutside
    );
    assert_eq!(kind_of(&read("/etc/passwd")), PermissionKind::ReadOutside);
    assert_eq!(kind_of(&read("/work/juniper")), PermissionKind::ReadOutside);
    assert_eq!(
        kind_of(&write("/work/juniper/app/theme.css")),
        PermissionKind::WriteRepo
    );
    assert_eq!(
        kind_of(&write("/tmp/theme.css")),
        PermissionKind::WriteOutside
    );
    assert_eq!(kind_of(&run("pnpm build")), PermissionKind::RunCommand);
    assert_eq!(
        kind_of(&net("https://example.com")),
        PermissionKind::Network
    );
}

#[test]
fn relative_paths_and_climbs_above_the_root_are_refused() {
    assert!(classify(&read("DESIGN.md"), ROOT).is_err());
    assert!(classify(&read("/../etc/passwd"), ROOT).is_err());
    assert!(classify(&read("/work/a"), "work").is_err());
    let relative_cwd = Action::RunCommand {
        program: "ls".into(),
        args: vec![],
        cwd: "here".into(),
    };
    assert!(classify(&relative_cwd, ROOT).is_err());
}

#[test]
fn windows_paths_are_compared_by_component() {
    let root = "C:\\work\\juniper";
    let k = |p: &str| classify(&read(p), root).unwrap().kind;
    assert_eq!(k("C:\\work\\juniper\\DESIGN.md"), PermissionKind::ReadRepo);
    assert_eq!(
        k("\\\\?\\C:\\work\\juniper\\DESIGN.md"),
        PermissionKind::ReadRepo
    );
    assert_eq!(k("C:/work/juniper2/DESIGN.md"), PermissionKind::ReadOutside);
    assert_eq!(
        normalize("\\\\server\\share\\a\\..\\b").as_deref(),
        Some("//server/share/b")
    );
}

#[test]
fn globs_stop_at_slashes_in_paths_only() {
    assert!(glob("docs/*.md", "docs/a.md", true));
    assert!(!glob("docs/*.md", "docs/x/a.md", true));
    assert!(glob("docs/**", "docs/x/a.md", true));
    assert!(glob("**/*.css", "theme.css", true));
    assert!(glob("**/*.css", "app/styles/theme.css", true));
    assert!(glob("?.md", "a.md", true));
    assert!(!glob("?.md", "ab.md", true));
    assert!(glob("pnpm *", "pnpm build --filter ./app", false));
    assert!(!glob("pnpm build", "pnpm build --watch", false));
    assert!(glob("", "", true));
    assert!(!glob("a", "", true));
}

#[test]
fn rules_parse_and_say_what_they_are() {
    let r = rule(Effect::Deny, "  run rm *  ");
    assert_eq!((r.target, r.pattern.as_str()), (RuleTarget::Run, "rm *"));
    assert_eq!(r.to_string(), "deny run rm *");
    assert!(Rule::parse(Effect::Allow, "read").is_err());
    assert!(Rule::parse(Effect::Allow, "delete docs/**").is_err());
    assert!(Rule::parse(Effect::Allow, "").is_err());
}

// ---- defaults and policy --------------------------------------------------------------------------------------

#[test]
fn by_default_everything_outside_the_canvas_asks() {
    let settings = PermissionSettings::default();
    assert!(!settings.skip_all, "skip-all is off by default");
    assert!(!settings.shows_skip_all_warning());
    let three = vec![Answer::AllowOnce, Answer::AllowForSession, Answer::Deny];
    let two = vec![Answer::AllowOnce, Answer::Deny];
    for (action, offered) in [
        (read("/work/juniper/DESIGN.md"), &three),
        (write("/work/juniper/theme.css"), &three),
        (run("pnpm build"), &three),
        (read("/etc/hosts"), &two),
        (write("/tmp/x"), &two),
        (net("https://example.com"), &two),
    ] {
        assert_eq!(
            verdict(&settings, &action),
            Verdict::Ask {
                offered: offered.clone()
            },
            "{action:?}"
        );
    }
}

#[test]
fn settings_can_pre_allow_repo_reads_writes_and_commands_only() {
    let settings = PermissionSettings {
        read_repo: Setting::Allow,
        write_files: Setting::Allow,
        run_commands: Setting::Allow,
        ..Default::default()
    };
    assert_eq!(
        verdict(&settings, &read("/work/juniper/DESIGN.md")),
        allowed_by(DecisionSource::Rule {
            rule: "read_repo = \"allow\"".into()
        })
    );
    assert!(matches!(
        verdict(&settings, &write("/work/juniper/t.css")),
        Verdict::Decided {
            answer: Answer::AllowOnce,
            ..
        }
    ));
    assert!(matches!(
        verdict(&settings, &run("pnpm build")),
        Verdict::Decided {
            answer: Answer::AllowOnce,
            ..
        }
    ));
    for outside in [
        read("/etc/hosts"),
        write("/tmp/t.css"),
        net("https://example.com"),
    ] {
        assert!(
            matches!(verdict(&settings, &outside), Verdict::Ask { .. }),
            "{outside:?} is asked every time"
        );
    }
}

#[test]
fn allow_rules_cover_matching_repo_files_and_commands() {
    let settings = PermissionSettings {
        rules: vec![
            rule(Effect::Allow, "read docs/**"),
            rule(Effect::Allow, "write tokens/*.css"),
            rule(Effect::Allow, "run pnpm build"),
            rule(Effect::Allow, "read /etc/**"),
        ],
        ..Default::default()
    };
    let decided = |a: &Action| matches!(verdict(&settings, a), Verdict::Decided { .. });
    assert!(decided(&read("/work/juniper/docs/spec/DESIGN.md")));
    assert!(!decided(&read("/work/juniper/src/main.ts")));
    assert!(decided(&write("/work/juniper/tokens/theme.css")));
    assert!(!decided(&write("/work/juniper/tokens/x/theme.css")));
    assert!(decided(&run("pnpm build")));
    assert!(!decided(&run("pnpm build --watch")));
    assert!(
        !decided(&read("/etc/hosts")),
        "allow rules never cover outside reads"
    );
}

#[test]
fn a_deny_rule_beats_everything() {
    let settings = PermissionSettings {
        read_repo: Setting::Allow,
        run_commands: Setting::Allow,
        skip_all: true,
        engine_auto: ["claude-code".to_owned()].into(),
        rules: vec![
            rule(Effect::Allow, "run rm *"),
            rule(Effect::Deny, "run rm *"),
            rule(Effect::Deny, "read .env*"),
            rule(Effect::Deny, "read /etc/**"),
        ],
        ..Default::default()
    };
    let deny = |text: &str| Verdict::Decided {
        answer: Answer::Deny,
        source: DecisionSource::Rule { rule: text.into() },
    };
    assert_eq!(verdict(&settings, &run("rm -rf /")), deny("deny run rm *"));
    assert_eq!(
        verdict_with(&settings, &run("rm x"), Some(("claude-code", true)), &[]),
        deny("deny run rm *")
    );
    assert_eq!(
        verdict(&settings, &read("/work/juniper/.env.local")),
        deny("deny read .env*")
    );
    assert_eq!(
        verdict(&settings, &read("/etc/hosts")),
        deny("deny read /etc/**")
    );
}

#[test]
fn skip_all_allows_everything_and_keeps_its_warning_on() {
    let settings = PermissionSettings {
        skip_all: true,
        ..Default::default()
    };
    assert!(settings.shows_skip_all_warning());
    for action in [
        read("/etc/hosts"),
        write("/tmp/x"),
        run("pnpm build"),
        net("https://example.com"),
    ] {
        assert_eq!(
            verdict(&settings, &action),
            allowed_by(DecisionSource::SkipAll)
        );
    }
}

#[test]
fn an_engine_decides_only_when_its_auto_mode_is_on() {
    let on = PermissionSettings {
        engine_auto: ["claude-code".to_owned()].into(),
        ..Default::default()
    };
    let action = run("pnpm build");
    let auto = |allowed| Verdict::Decided {
        answer: if allowed {
            Answer::AllowOnce
        } else {
            Answer::Deny
        },
        source: DecisionSource::EngineAuto {
            engine: "claude-code".into(),
        },
    };
    assert_eq!(
        verdict_with(&on, &action, Some(("claude-code", true)), &[]),
        auto(true)
    );
    assert_eq!(
        verdict_with(&on, &action, Some(("claude-code", false)), &[]),
        auto(false)
    );
    // Another engine, auto mode off, or no verdict: the person decides.
    assert!(matches!(
        verdict_with(&on, &action, Some(("codex", true)), &[]),
        Verdict::Ask { .. }
    ));
    assert!(matches!(
        verdict_with(
            &PermissionSettings::default(),
            &action,
            Some(("claude-code", true)),
            &[]
        ),
        Verdict::Ask { .. }
    ));
    assert!(matches!(verdict(&on, &action), Verdict::Ask { .. }));
}

fn granted(seq: u64, action: Action, answer: Answer, source: DecisionSource) -> SessionEvent {
    SessionEvent {
        seq: EventSeq(seq),
        at: Timestamp(0),
        origin: agent_origin(),
        body: EventBody::Permission(PermissionRecord {
            kind: kind_of(&action),
            action,
            engine: Some("builtin".into()),
            answer,
            source,
        }),
    }
}

#[test]
fn allow_for_this_session_covers_what_it_says() {
    let settings = PermissionSettings::default();
    let events = [
        granted(
            3,
            read("/work/juniper/DESIGN.md"),
            Answer::AllowForSession,
            DecisionSource::Person,
        ),
        granted(
            4,
            write("/work/juniper/theme.css"),
            Answer::AllowForSession,
            DecisionSource::Person,
        ),
        granted(
            5,
            run("pnpm build"),
            Answer::AllowForSession,
            DecisionSource::Person,
        ),
    ];
    let v = |a: Action| verdict_with(&settings, &a, None, &events);
    assert_eq!(
        v(read("/work/juniper/src/app.ts")),
        allowed_by(DecisionSource::SessionGrant { grant: EventSeq(3) }),
        "a repo-read grant covers every repo read"
    );
    assert_eq!(
        v(write("/work/juniper/theme.css")),
        allowed_by(DecisionSource::SessionGrant { grant: EventSeq(4) })
    );
    assert!(matches!(
        v(write("/work/juniper/other.css")),
        Verdict::Ask { .. }
    ));
    assert_eq!(
        v(run("pnpm build")),
        allowed_by(DecisionSource::SessionGrant { grant: EventSeq(5) })
    );
    assert!(matches!(v(run("pnpm test")), Verdict::Ask { .. }));
    assert!(matches!(v(read("/etc/hosts")), Verdict::Ask { .. }));
}

#[test]
fn only_a_persons_session_grant_counts() {
    let settings = PermissionSettings::default();
    let action = read("/work/juniper/DESIGN.md");
    for event in [
        granted(1, action.clone(), Answer::AllowOnce, DecisionSource::Person),
        granted(1, action.clone(), Answer::Deny, DecisionSource::Person),
        granted(
            1,
            action.clone(),
            Answer::AllowForSession,
            DecisionSource::SkipAll,
        ),
        granted(
            1,
            read("/etc/hosts"),
            Answer::AllowForSession,
            DecisionSource::Person,
        ),
    ] {
        assert!(matches!(
            verdict_with(&settings, &action, None, &[event]),
            Verdict::Ask { .. }
        ));
    }
    assert!(!grant_covers(
        &PermissionRecord {
            action: net("https://a"),
            kind: PermissionKind::Network,
            engine: None,
            answer: Answer::AllowForSession,
            source: DecisionSource::Person,
        },
        PermissionKind::Network,
        &net("https://a"),
    ));
}

// ---- the broker ------------------------------------------------------------------------------------------------

#[test]
fn allow_once_yields_a_permit_for_exactly_that_action_and_records_it() {
    let store = Rc::new(Store::new(engine_kind()));
    let prompt = Scripted::answering([Some(Answer::AllowOnce), Some(Answer::AllowOnce)]);
    let b = broker(PermissionSettings::default(), &store, Some(prompt.clone()));
    let action = read("/work/juniper/DESIGN.md");
    let permit = block_on(b.request(request(agent_origin(), action.clone()))).unwrap();
    assert!(permit.covers(&action));
    assert!(!permit.covers(&read("/work/juniper/other.md")));
    assert_eq!(permit.kind(), PermissionKind::ReadRepo);
    assert_eq!(permit.origin(), &agent_origin());
    assert_eq!(
        store.events.borrow()[permit.record().0 as usize - 1].body,
        EventBody::Permission(PermissionRecord {
            action: action.clone(),
            kind: PermissionKind::ReadRepo,
            engine: Some("builtin".into()),
            answer: Answer::AllowOnce,
            source: DecisionSource::Person,
        })
    );
    let question = prompt.asked.borrow()[0].clone();
    assert_eq!(question.agent, Some(agent()));
    assert_eq!(question.kind, PermissionKind::ReadRepo);
    // Once is once: the same read asks again.
    block_on(b.request(request(agent_origin(), action))).unwrap();
    assert_eq!(prompt.asked.borrow().len(), 2);
}

#[test]
fn allow_for_this_session_is_asked_once() {
    let store = Rc::new(Store::new(engine_kind()));
    let prompt = Scripted::answering([Some(Answer::AllowForSession)]);
    let b = broker(PermissionSettings::default(), &store, Some(prompt.clone()));
    let first =
        block_on(b.request(request(agent_origin(), read("/work/juniper/DESIGN.md")))).unwrap();
    let second =
        block_on(b.request(request(agent_origin(), read("/work/juniper/src/a.ts")))).unwrap();
    assert_eq!(prompt.asked.borrow().len(), 1);
    let records = store.permissions();
    assert_eq!(records.len(), 2, "every decision is recorded, grants too");
    assert_eq!(
        records[1].source,
        DecisionSource::SessionGrant {
            grant: first.record()
        }
    );
    assert_eq!(second.record(), EventSeq(2));
}

#[test]
fn deny_is_recorded_and_says_why() {
    let store = Rc::new(Store::new(engine_kind()));
    let prompt = Scripted::answering([Some(Answer::Deny), None]);
    let b = broker(PermissionSettings::default(), &store, Some(prompt));
    let refusal = block_on(b.request(request(agent_origin(), run("pnpm build")))).unwrap_err();
    assert_eq!(refusal.seq, Some(EventSeq(1)));
    assert_eq!(refusal.record.answer, Answer::Deny);
    assert_eq!(
        refusal.to_string(),
        "Not allowed to run pnpm build: the person denied it."
    );
    // A question dismissed without an answer is a denial too.
    let dismissed = block_on(b.request(request(agent_origin(), run("pnpm build")))).unwrap_err();
    assert_eq!(dismissed.record.source, DecisionSource::Person);
    assert_eq!(store.permissions().len(), 2);
}

#[test]
fn a_session_answer_that_was_not_offered_counts_once() {
    let store = Rc::new(Store::new(engine_kind()));
    let prompt = Scripted::answering([Some(Answer::AllowForSession), Some(Answer::Deny)]);
    let b = broker(PermissionSettings::default(), &store, Some(prompt.clone()));
    let permit = block_on(b.request(request(agent_origin(), read("/etc/hosts")))).unwrap();
    assert_eq!(
        prompt.asked.borrow()[0].offered,
        vec![Answer::AllowOnce, Answer::Deny]
    );
    assert_eq!(store.permissions()[0].answer, Answer::AllowOnce);
    assert!(permit.covers(&read("/etc/hosts")));
    assert!(block_on(b.request(request(agent_origin(), read("/etc/hosts")))).is_err());
}

#[test]
fn with_nobody_to_ask_whatever_no_rule_allows_is_denied() {
    let store = Rc::new(Store::new(engine_kind()));
    let b = broker(PermissionSettings::default(), &store, None);
    let refusal =
        block_on(b.request(request(agent_origin(), write("/work/juniper/t.css")))).unwrap_err();
    assert_eq!(refusal.record.source, DecisionSource::Unattended);
    assert_eq!(refusal.seq, Some(EventSeq(1)));
}

#[test]
fn nothing_is_allowed_without_a_record() {
    // No session on the origin.
    let store = Rc::new(Store::new(engine_kind()));
    let skip = PermissionSettings {
        skip_all: true,
        ..Default::default()
    };
    let b = broker(skip.clone(), &store, None);
    let anonymous = Origin::new(Actor::Agent { agent: agent() });
    let refusal = block_on(b.request(request(anonymous, net("https://example.com")))).unwrap_err();
    assert_eq!(refusal.seq, None);
    // An unknown session.
    let other = Origin::new(Actor::Agent { agent: agent() })
        .in_session(SessionId::new("01J0000000000000000000000X").unwrap());
    assert!(block_on(b.request(request(other, net("https://example.com")))).is_err());
    // A store that cannot append.
    let full = Rc::new(Store {
        full_at: Some(0),
        ..Store::new(engine_kind())
    });
    let b = broker(skip, &full, None);
    let refusal =
        block_on(b.request(request(agent_origin(), net("https://example.com")))).unwrap_err();
    assert_eq!(refusal.seq, None);
    assert_eq!(refusal.record.answer, Answer::Deny);
    assert!(matches!(
        refusal.record.source,
        DecisionSource::Failed { .. }
    ));
    assert!(store.events.borrow().is_empty() && full.events.borrow().is_empty());
}

#[test]
fn a_relative_path_is_denied_and_recorded() {
    let store = Rc::new(Store::new(engine_kind()));
    let b = broker(
        PermissionSettings {
            skip_all: true,
            ..Default::default()
        },
        &store,
        None,
    );
    let refusal = block_on(b.request(request(agent_origin(), read("DESIGN.md")))).unwrap_err();
    assert!(matches!(
        refusal.record.source,
        DecisionSource::Failed { .. }
    ));
    assert_eq!(refusal.seq, Some(EventSeq(1)));
}

fn start_turn(store: &Store) {
    let person = Origin::new(person()).in_session(session()).in_turn(turn());
    for input in [TurnInput::Start, TurnInput::NoPlanNeeded] {
        record_transition(store, &person, input, Timestamp(0)).unwrap();
    }
}

fn turn_moves(store: &Store) -> Vec<(TurnInput, TurnState, bool)> {
    store
        .events
        .borrow()
        .iter()
        .filter_map(|e| match &e.body {
            EventBody::Turn(change) => Some((change.input, change.to, e.origin.actor.is_person())),
            _ => None,
        })
        .collect()
}

#[test]
fn asking_moves_the_turn_to_awaiting_permission_and_back() {
    let store = Rc::new(Store::new(engine_kind()));
    start_turn(&store);
    let prompt = Scripted::answering([Some(Answer::AllowOnce)]);
    let watched = store.clone();
    *prompt.during.borrow_mut() = Some(Box::new(move || {
        assert_eq!(
            crate::sessions::turn_state(&watched.events.borrow(), &turn()),
            Some(TurnState::AwaitingPermission),
            "the turn waits while the card is open"
        );
    }));
    let b = broker(PermissionSettings::default(), &store, Some(prompt));
    block_on(b.request(request(in_turn(), read("/work/juniper/DESIGN.md")))).unwrap();
    assert_eq!(
        turn_moves(&store)[2..],
        [
            (
                TurnInput::PermissionRequested,
                TurnState::AwaitingPermission,
                false
            ),
            (TurnInput::Decided, TurnState::Running, true),
        ]
    );
}

#[test]
fn a_rule_decides_without_moving_the_turn() {
    let store = Rc::new(Store::new(engine_kind()));
    start_turn(&store);
    let b = broker(
        PermissionSettings {
            read_repo: Setting::Allow,
            ..Default::default()
        },
        &store,
        None,
    );
    block_on(b.request(request(in_turn(), read("/work/juniper/DESIGN.md")))).unwrap();
    assert_eq!(turn_moves(&store).len(), 2);
}

#[test]
fn stopping_the_turn_while_it_waits_denies() {
    let store = Rc::new(Store::new(engine_kind()));
    start_turn(&store);
    let prompt = Scripted::answering([Some(Answer::AllowOnce)]);
    let stopper = store.clone();
    *prompt.during.borrow_mut() = Some(Box::new(move || {
        let person = Origin::new(person()).in_session(session()).in_turn(turn());
        record_transition(&*stopper, &person, TurnInput::Stop, Timestamp(1)).unwrap();
    }));
    let b = broker(PermissionSettings::default(), &store, Some(prompt));
    let refusal = block_on(b.request(request(in_turn(), run("pnpm build")))).unwrap_err();
    assert_eq!(refusal.record.source, DecisionSource::TurnStopped);
    assert_eq!(
        crate::sessions::turn_state(&store.events.borrow(), &turn()),
        Some(TurnState::Stopped)
    );
}

#[test]
fn a_stopped_turn_gets_nothing_even_with_skip_all() {
    let store = Rc::new(Store::new(engine_kind()));
    start_turn(&store);
    let person = Origin::new(person()).in_session(session()).in_turn(turn());
    record_transition(&*store, &person, TurnInput::Stop, Timestamp(1)).unwrap();
    let b = broker(
        PermissionSettings {
            skip_all: true,
            ..Default::default()
        },
        &store,
        None,
    );
    let refusal = block_on(b.request(request(in_turn(), read("/work/juniper/a.md")))).unwrap_err();
    assert_eq!(refusal.record.source, DecisionSource::TurnStopped);
}

#[test]
fn external_sessions_record_their_decisions() {
    let store = Rc::new(Store::new(SessionKind::External {
        client: "Claude Code".into(),
    }));
    let b = broker(
        PermissionSettings::default(),
        &store,
        Some(Scripted::answering([Some(Answer::AllowOnce)])),
    );
    let mut req = request(agent_origin(), read("/work/juniper/DESIGN.md"));
    req.engine = None;
    block_on(b.request(req)).unwrap();
    assert_eq!(store.permissions().len(), 1);
    assert!(
        SessionKind::External { client: "x".into() }
            .accepts(&EventBody::Permission(store.permissions()[0].clone()))
    );
}

// ---- the stored form -------------------------------------------------------------------------------------------

#[test]
fn every_record_round_trips_through_the_codec() {
    let sources = [
        DecisionSource::Person,
        DecisionSource::SessionGrant { grant: EventSeq(7) },
        DecisionSource::Rule {
            rule: "allow read docs/**".into(),
        },
        DecisionSource::EngineAuto {
            engine: "claude-code".into(),
        },
        DecisionSource::SkipAll,
        DecisionSource::Unattended,
        DecisionSource::TurnStopped,
        DecisionSource::Failed {
            reason: "disk full".into(),
        },
    ];
    let actions = [
        read("/work/juniper/DESIGN.md"),
        write("/tmp/t.css"),
        run("pnpm build --filter app"),
        net("https://example.com/a?b=c"),
    ];
    for (i, source) in sources.iter().enumerate() {
        for (j, action) in actions.iter().enumerate() {
            let body = EventBody::Permission(PermissionRecord {
                kind: kind_of(action),
                action: action.clone(),
                engine: ((i + j) % 2 == 0).then(|| "builtin".into()),
                answer: [Answer::AllowOnce, Answer::AllowForSession, Answer::Deny][(i + j) % 3],
                source: source.clone(),
            });
            let json = body_to_json(&body);
            assert_eq!(body_from_json(&json).unwrap(), body, "{json}");
        }
    }
}

#[test]
fn the_codec_refuses_what_it_did_not_write() {
    let good = body_to_json(&EventBody::Permission(PermissionRecord {
        action: read("/a/b"),
        kind: PermissionKind::ReadOutside,
        engine: None,
        answer: Answer::Deny,
        source: DecisionSource::Person,
    }));
    for (path, value) in [
        ("/kind", serde_json::json!("read_everything")),
        ("/answer", serde_json::json!("maybe")),
        ("/source/kind", serde_json::json!("vibes")),
        (
            "/action",
            serde_json::json!({ "readFile": { "path": "/a" }, "network": { "url": "x" } }),
        ),
        (
            "/action",
            serde_json::json!({ "readFile": { "path": "/a", "mode": "r" } }),
        ),
        ("/extra", serde_json::json!(1)),
    ] {
        let mut bad = good.clone();
        match bad.pointer_mut(path) {
            Some(slot) => *slot = value,
            None => {
                bad[path.trim_start_matches('/')] = value;
            }
        }
        assert!(body_from_json(&bad).is_err(), "{bad}");
    }
    let mut person_with_rule = good.clone();
    person_with_rule["source"]["rule"] = serde_json::json!("x");
    assert!(body_from_json(&person_with_rule).is_err());
}
