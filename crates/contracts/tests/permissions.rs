//! The broker against its contract, a broken broker against the contract, and guide Invariant 11 as a property
//! test over generated agent actions. TEST DATA throughout.
mod permissions {
    use proptest::prelude::*;
    use std::rc::Rc;
    use vellum_contracts::fakes::{ManualClock, ScriptedPrompt};
    use vellum_contracts::suites::model::block_on;
    use vellum_contracts::suites::permissions::{self as suite, Parts, ROOT};
    use vellum_core::permissions::{
        Broker, Effect, PermissionSettings, Permit, Rule, RuleTarget, Setting, classify, glob,
        normalize,
    };
    use vellum_core::ports::*;
    use vellum_types::{Actor, DeviceId};

    fn person() -> Actor {
        Actor::Person {
            device: DeviceId::new("01J000000000000000000000PD").unwrap(),
        }
    }

    fn broker(parts: Parts) -> Broker {
        Broker::new(
            parts.settings,
            parts.store,
            parts.clock,
            parts.prompt,
            person(),
        )
    }

    #[test]
    fn the_broker_passes_its_suite() {
        suite::run(broker);
    }

    /// Wraps the real broker but loses something on the way in.
    struct Broken {
        inner: Broker,
        rule: &'static str,
    }
    impl PermissionBroker for Broken {
        fn request<'a>(
            &'a self,
            mut request: PermissionRequest,
        ) -> EngineFuture<'a, Result<Permit, Refusal>> {
            if self.rule == "drops the session" {
                request.origin.session = None;
            }
            self.inner.request(request)
        }
    }

    #[test]
    fn broken_brokers_fail() {
        type Spoil = fn(&mut Parts);
        let cases: [(&str, Spoil); 4] = [
            ("never asks", |p| p.prompt = None),
            ("ignores settings", |p| {
                p.settings = PermissionSettings::default()
            }),
            ("skips everything", |p| p.settings.skip_all = true),
            ("drops the session", |_| {}),
        ];
        for (rule, spoil) in cases {
            let caught = std::panic::catch_unwind(|| {
                suite::run(|mut parts| {
                    spoil(&mut parts);
                    Broken {
                        inner: broker(parts),
                        rule,
                    }
                })
            });
            assert!(caught.is_err(), "the suite accepted a broker that {rule}");
        }
    }

    // ---- Invariant 11 ---------------------------------------------------------------------------------------

    /// The only way the simulated agent host does anything outside the canvas: it needs a permit for exactly
    /// that action, and keeps what it did.
    #[derive(Default)]
    struct Host {
        done: Vec<(Action, EventSeq)>,
    }
    impl Host {
        fn perform(&mut self, permit: Permit, action: &Action) {
            assert!(permit.covers(action), "a permit for another action");
            self.done.push((action.clone(), permit.record()));
        }
    }

    fn action() -> impl Strategy<Value = Action> {
        let file = prop_oneof![
            Just("DESIGN.md"),
            Just("docs/spec.md"),
            Just("src/app.css"),
            Just(".env"),
            Just("../elsewhere/notes.md"),
        ]
        .prop_map(|p| format!("{ROOT}/{p}"));
        let outside = prop_oneof![Just("/etc/hosts"), Just("/tmp/out.png")].prop_map(str::to_owned);
        let path = prop_oneof![3 => file, 1 => outside];
        prop_oneof![
            path.clone().prop_map(|path| Action::ReadFile { path }),
            path.prop_map(|path| Action::WriteFile { path }),
            prop_oneof![Just("pnpm build"), Just("pnpm test"), Just("rm -rf dist")].prop_map(
                |line| {
                    let mut words = line.split(' ').map(str::to_owned);
                    Action::RunCommand {
                        program: words.next().unwrap(),
                        args: words.collect(),
                        cwd: ROOT.into(),
                    }
                }
            ),
            Just(Action::Network {
                url: "https://example.com/icons.json".into()
            }),
        ]
    }

    fn setting() -> impl Strategy<Value = Setting> {
        prop_oneof![3 => Just(Setting::Ask), 1 => Just(Setting::Allow)]
    }

    fn settings() -> impl Strategy<Value = PermissionSettings> {
        let rule = prop_oneof![
            Just((Effect::Allow, "read docs/**")),
            Just((Effect::Allow, "run pnpm *")),
            Just((Effect::Deny, "read .env*")),
            Just((Effect::Deny, "run rm *")),
            Just((Effect::Deny, "write /tmp/**")),
        ]
        .prop_map(|(effect, text)| Rule::parse(effect, text).unwrap());
        (
            setting(),
            setting(),
            setting(),
            prop::bool::weighted(0.15),
            prop::bool::ANY,
            prop::collection::vec(rule, 0..3),
        )
            .prop_map(
                |(read_repo, write_files, run_commands, skip_all, auto, rules)| {
                    PermissionSettings {
                        read_repo,
                        write_files,
                        run_commands,
                        skip_all,
                        engine_auto: if auto {
                            ["claude-code".to_owned()].into()
                        } else {
                            Default::default()
                        },
                        rules,
                    }
                },
            )
    }

    fn answer() -> impl Strategy<Value = Option<Answer>> {
        prop_oneof![
            Just(Some(Answer::AllowOnce)),
            Just(Some(Answer::AllowForSession)),
            Just(Some(Answer::Deny)),
            Just(None),
        ]
    }

    /// One generated agent request: what it wants, which engine asks, what that engine's own permission system
    /// says, and what the person would answer if asked.
    fn call() -> impl Strategy<Value = (Action, bool, Option<bool>, Option<Answer>)> {
        (
            action(),
            prop::bool::ANY,
            prop::option::of(prop::bool::ANY),
            answer(),
        )
    }

    /// Whether a deny rule in `settings` names `action`, worked out here rather than by the policy.
    fn denied_by_rule(settings: &PermissionSettings, action: &Action) -> bool {
        let file = |path: &str| {
            let absolute = normalize(path).unwrap();
            let inside = absolute
                .strip_prefix(&format!("{ROOT}/"))
                .map(str::to_owned);
            (absolute, inside)
        };
        settings
            .rules
            .iter()
            .filter(|r| r.effect == Effect::Deny)
            .any(|r| {
                let path_rule = |path: &str| {
                    let (absolute, inside) = file(path);
                    if r.pattern.starts_with('/') {
                        glob(&r.pattern, &absolute, true)
                    } else {
                        inside.is_some_and(|inside| glob(&r.pattern, &inside, true))
                    }
                };
                match (r.target, action) {
                    (RuleTarget::Read, Action::ReadFile { path })
                    | (RuleTarget::Write, Action::WriteFile { path }) => path_rule(path),
                    (RuleTarget::Run, Action::RunCommand { program, args, .. }) => {
                        glob(&r.pattern, &Action::command_line(program, args), false)
                    }
                    _ => false,
                }
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn every_action_outside_the_canvas_has_an_allowed_record(
            settings in settings(),
            calls in prop::collection::vec(call(), 1..24),
            prompt_present in prop::bool::weighted(0.8),
        ) {
            let store = suite::store();
            let prompt = Rc::new(ScriptedPrompt::new(calls.iter().map(|c| c.3), None));
            let b = Broker::new(
                settings.clone(),
                store.clone(),
                Rc::new(ManualClock::new(5)),
                prompt_present.then(|| prompt.clone() as Rc<dyn PermissionPrompt>),
                person(),
            );
            let mut host = Host::default();
            for (action, installed, verdict, _) in &calls {
                let mut req = suite::request(action.clone());
                if *installed {
                    req.engine = Some("claude-code".into());
                    req.engine_verdict = *verdict;
                }
                match block_on(b.request(req)) {
                    Ok(permit) => host.perform(permit, action),
                    Err(refusal) => prop_assert!(refusal.seq.is_some(), "every refusal here is recorded"),
                }
            }

            let events = store.events(&suite::session(), None).unwrap();
            let records: Vec<_> = events
                .iter()
                .filter_map(|e| match &e.body {
                    EventBody::Permission(r) => Some((e.seq, r.clone())),
                    _ => None,
                })
                .collect();
            prop_assert_eq!(records.len(), calls.len(), "one decision record per request");

            // Everything performed has its own Allowed record for that exact action, from this agent.
            for (action, seq) in &host.done {
                let (_, record) = records.iter().find(|(s, _)| s == seq).expect("the permit's record");
                prop_assert!(record.allowed());
                prop_assert_eq!(&record.action, action);
                let event = events.iter().find(|e| e.seq == *seq).unwrap();
                prop_assert_eq!(&event.origin, &suite::origin());
            }
            let allowed = records.iter().filter(|(_, r)| r.allowed()).count();
            prop_assert_eq!(allowed, host.done.len(), "every allowed record was a permit");

            // Who decided is one the person chose: themselves, a rule, an engine they put in auto mode, or skip-all.
            // A deny rule always denies, whatever else is on.
            for ((_, record), (action, ..)) in records.iter().zip(&calls) {
                if denied_by_rule(&settings, action) {
                    prop_assert!(!record.allowed(), "a deny rule let {:?} through", action);
                }
            }

            // The prompt hands out the scripted answers in the order it is asked.
            let mut script = calls.iter().map(|c| c.3);
            let mut asked = 0;
            for ((_, record), (action, installed, verdict, _)) in records.iter().zip(&calls) {
                let kind = classify(action, ROOT).unwrap().kind;
                prop_assert_eq!(record.kind, kind);
                let deny_rule = settings.rules.iter().any(|r| {
                    r.effect == Effect::Deny && matches!(&record.source, DecisionSource::Rule { rule } if *rule == r.to_string())
                });
                match &record.source {
                    DecisionSource::Person => {
                        prop_assert!(prompt_present);
                        let expected = match script.next().unwrap().unwrap_or(Answer::Deny) {
                            Answer::AllowForSession if !kind.session_grant_offered() => Answer::AllowOnce,
                            other => other,
                        };
                        prop_assert_eq!(record.answer, expected);
                        asked += 1;
                    }
                    DecisionSource::SkipAll => prop_assert!(settings.skip_all),
                    DecisionSource::EngineAuto { engine } => {
                        prop_assert!(settings.engine_auto.contains(engine) && *installed);
                        prop_assert_eq!(record.allowed(), verdict.unwrap());
                    }
                    DecisionSource::Rule { .. } if deny_rule => prop_assert!(!record.allowed()),
                    DecisionSource::Rule { .. } => prop_assert!(matches!(
                        kind,
                        PermissionKind::ReadRepo | PermissionKind::WriteRepo | PermissionKind::RunCommand
                    )),
                    DecisionSource::SessionGrant { grant } => {
                        let (_, earlier) = records.iter().find(|(s, _)| s == grant).unwrap();
                        prop_assert_eq!(earlier.answer, Answer::AllowForSession);
                        prop_assert_eq!(&earlier.source, &DecisionSource::Person);
                    }
                    DecisionSource::Unattended => prop_assert!(!prompt_present && !record.allowed()),
                    other => prop_assert!(false, "unexpected source {:?}", other),
                }
                // Outside the repository and on the network, nothing but skip-all or an engine's auto mode stands
                // in for the person.
                if !kind.session_grant_offered() && record.allowed() {
                    let stood_in = matches!(
                        record.source,
                        DecisionSource::Person | DecisionSource::SkipAll | DecisionSource::EngineAuto { .. }
                    );
                    prop_assert!(stood_in);
                }
            }
            prop_assert_eq!(asked, prompt.asked().len(), "the person was asked only when nothing else decided");
        }
    }
}
