//! The permission broker (ARCHITECTURE §7.7, guide Invariant 11): anything an agent does outside the canvas is
//! decided here and recorded in the session store before it happens.
//!
//! [`Broker`] is the one [`PermissionBroker`]. Hosts differ only in the [`PermissionPrompt`] they give it (the
//! editor's Permission card, the terminal) or none (a headless daemon, which denies whatever no rule decides). A
//! request is decided, in order, by a deny rule, "Skip all permission prompts", the engine's auto mode, an allow
//! setting or rule, an earlier "Allow for this session", and otherwise the person. Every decision, allowed or
//! not, is appended to the session as one [`EventBody::Permission`] event, and only a recorded `Allowed` decision
//! yields a [`Permit`]: the code that reads, writes, runs or connects takes the permit for exactly that action.
//! When the person is asked during an engine turn, the turn goes `running → awaiting_permission → running` (two
//! transition events, guide §10.1); a turn stopped while it waits gets a denial.
mod policy;
#[cfg(test)]
mod tests;

pub use policy::{
    Classified, ClassifyError, Effect, PathInRepo, PermissionSettings, Rule, RuleError, RuleTarget,
    Setting, Verdict, classify, evaluate, glob, grant_covers, normalize,
};

use crate::ports::{
    Action, Answer, Clock, DecisionSource, EngineFuture, EventBody, EventSeq, NewEvent,
    PermissionBroker, PermissionKind, PermissionPrompt, PermissionRecord, PermissionRequest,
    Question, Refusal, SessionStore,
};
use crate::sessions::{TransitionError, record_transition, turn_state};
use crate::turns::{TurnInput, TurnRejection, TurnState};
use std::rc::Rc;
use vellum_types::{Actor, Origin};

/// Proof that an action was allowed and the decision recorded. Only the broker makes one; whatever performs the
/// action takes it by value and checks [`Permit::covers`]. Not `Clone`: "Allow once" is once.
#[derive(Debug, PartialEq, Eq)]
pub struct Permit {
    action: Action,
    kind: PermissionKind,
    origin: Origin,
    record: EventSeq,
}

impl Permit {
    pub fn action(&self) -> &Action {
        &self.action
    }
    pub fn kind(&self) -> PermissionKind {
        self.kind
    }
    /// Who it was granted to.
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    /// Where the `Allowed` decision is in the session.
    pub fn record(&self) -> EventSeq {
        self.record
    }
    /// Whether it allows exactly `action`.
    pub fn covers(&self, action: &Action) -> bool {
        &self.action == action
    }
}

/// The broker every host uses.
pub struct Broker {
    settings: PermissionSettings,
    store: Rc<dyn SessionStore>,
    clock: Rc<dyn Clock>,
    prompt: Option<Rc<dyn PermissionPrompt>>,
    /// Who answers the prompt: the person at this device. Their answer moves the turn back to running.
    person: Actor,
}

impl Broker {
    /// A broker that asks `prompt` when no rule decides, or denies when there is none.
    pub fn new(
        settings: PermissionSettings,
        store: Rc<dyn SessionStore>,
        clock: Rc<dyn Clock>,
        prompt: Option<Rc<dyn PermissionPrompt>>,
        person: Actor,
    ) -> Self {
        Self {
            settings,
            store,
            clock,
            prompt,
            person,
        }
    }

    pub fn settings(&self) -> &PermissionSettings {
        &self.settings
    }

    /// Settings → Permissions changed.
    pub fn set_settings(&mut self, settings: PermissionSettings) {
        self.settings = settings;
    }

    /// Appends the decision; a permit only for a recorded allow.
    fn record(
        &self,
        request: &PermissionRequest,
        kind: PermissionKind,
        answer: Answer,
        source: DecisionSource,
    ) -> Result<Permit, Refusal> {
        let record = PermissionRecord {
            action: request.action.clone(),
            kind,
            engine: request.engine.clone(),
            answer,
            source,
        };
        let session = request
            .origin
            .session
            .clone()
            .expect("checked before deciding");
        let appended = self.store.append(
            &session,
            NewEvent {
                at: self.clock.now(),
                origin: request.origin.clone(),
                body: EventBody::Permission(record.clone()),
            },
        );
        match appended {
            Ok(seq) if record.allowed() => Ok(Permit {
                action: request.action.clone(),
                kind,
                origin: request.origin.clone(),
                record: seq,
            }),
            Ok(seq) => Err(Refusal {
                record: Box::new(record),
                seq: Some(seq),
            }),
            // Not recorded, so not allowed, whatever was decided.
            Err(error) => Err(Refusal {
                record: Box::new(PermissionRecord {
                    answer: Answer::Deny,
                    source: DecisionSource::Failed {
                        reason: format!("the decision could not be recorded ({error})"),
                    },
                    ..record
                }),
                seq: None,
            }),
        }
    }

    fn unrecorded(request: &PermissionRequest, kind: PermissionKind, reason: String) -> Refusal {
        Refusal {
            record: Box::new(PermissionRecord {
                action: request.action.clone(),
                kind,
                engine: request.engine.clone(),
                answer: Answer::Deny,
                source: DecisionSource::Failed { reason },
            }),
            seq: None,
        }
    }

    /// Moves the request's turn, when it has one. `Ok(false)` means the turn is over (stopped while waiting).
    fn move_turn(&self, origin: &Origin, input: TurnInput) -> Result<bool, String> {
        if origin.turn.is_none() {
            return Ok(true);
        }
        match record_transition(&*self.store, origin, input, self.clock.now()) {
            Ok(_) => Ok(true),
            Err(TransitionError::Rejected(TurnRejection::Terminal)) => Ok(false),
            Err(error) => Err(format!("the turn could not wait for an answer ({error})")),
        }
    }

    async fn decide(&self, request: PermissionRequest) -> Result<Permit, Refusal> {
        // A kind for records made before the action could be sorted; replaced as soon as it is.
        let rough = match &request.action {
            Action::ReadFile { .. } => PermissionKind::ReadOutside,
            Action::WriteFile { .. } => PermissionKind::WriteOutside,
            Action::RunCommand { .. } => PermissionKind::RunCommand,
            Action::Network { .. } => PermissionKind::Network,
        };
        let Some(session) = request.origin.session.clone() else {
            return Err(Self::unrecorded(
                &request,
                rough,
                "the request names no session".into(),
            ));
        };
        let meta = match self.store.session(&session) {
            Ok(Some(meta)) => meta,
            Ok(None) => {
                return Err(Self::unrecorded(&request, rough, "no such session".into()));
            }
            Err(error) => return Err(Self::unrecorded(&request, rough, error.to_string())),
        };
        let classified = match classify(&request.action, meta.doc.repo_root()) {
            Ok(classified) => classified,
            Err(error) => {
                return self.record(
                    &request,
                    rough,
                    Answer::Deny,
                    DecisionSource::Failed {
                        reason: error.to_string(),
                    },
                );
            }
        };
        let kind = classified.kind;
        let events = match self.store.events(&session, None) {
            Ok(events) => events,
            Err(error) => return Err(Self::unrecorded(&request, kind, error.to_string())),
        };
        // Nothing more happens for a turn that is over, whatever the rules say (Invariant 12).
        if let Some(turn) = &request.origin.turn
            && turn_state(&events, turn).is_some_and(TurnState::is_terminal)
        {
            return self.record(&request, kind, Answer::Deny, DecisionSource::TurnStopped);
        }
        let offered = match evaluate(
            &self.settings,
            &classified,
            &request.action,
            request.engine.as_deref(),
            request.engine_verdict,
            &events,
        ) {
            Verdict::Decided { answer, source } => {
                return self.record(&request, kind, answer, source);
            }
            Verdict::Ask { offered } => offered,
        };
        let Some(prompt) = self.prompt.clone() else {
            return self.record(&request, kind, Answer::Deny, DecisionSource::Unattended);
        };
        match self.move_turn(&request.origin, TurnInput::PermissionRequested) {
            Ok(true) => {}
            Ok(false) => {
                return self.record(&request, kind, Answer::Deny, DecisionSource::TurnStopped);
            }
            Err(reason) => {
                return self.record(
                    &request,
                    kind,
                    Answer::Deny,
                    DecisionSource::Failed { reason },
                );
            }
        }
        let agent = match &request.origin.actor {
            Actor::Agent { agent } => Some(agent.clone()),
            _ => None,
        };
        let answer = prompt
            .ask(Question {
                agent,
                engine: request.engine.clone(),
                action: request.action.clone(),
                kind,
                offered: offered.clone(),
            })
            .await
            .unwrap_or(Answer::Deny);
        // An answer that was not offered is read as the nearest one that was.
        let answer = if offered.contains(&answer) {
            answer
        } else {
            Answer::AllowOnce
        };
        let mut person = Origin::new(self.person.clone()).in_session(session);
        if let Some(turn) = request.origin.turn.clone() {
            person = person.in_turn(turn);
        }
        match self.move_turn(&person, TurnInput::Decided) {
            Ok(true) => self.record(&request, kind, answer, DecisionSource::Person),
            Ok(false) => self.record(&request, kind, Answer::Deny, DecisionSource::TurnStopped),
            Err(reason) => self.record(
                &request,
                kind,
                Answer::Deny,
                DecisionSource::Failed { reason },
            ),
        }
    }
}

impl PermissionBroker for Broker {
    fn request<'a>(
        &'a self,
        request: PermissionRequest,
    ) -> EngineFuture<'a, Result<Permit, Refusal>> {
        Box::pin(self.decide(request))
    }
}
