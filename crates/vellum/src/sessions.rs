//! Recording agent sessions (ARCHITECTURE §7.6, ADR-006): what the daemon writes to the session store.
//!
//! External MCP clients get one session per document they work on, made at their first step there and holding
//! their tool calls only. An engine turn's host makes its session first ([`Session::log_as`]) and records the
//! turn's usage, messages and end with [`SessionLog::record_turn_event`]; its steps arrive through the same
//! bridge steps as any client's. Sessions are keyed by repository root and repo-relative path ([`doc_key`]).
use std::path::{Component, Path, PathBuf};

use vellum_core::ports::{
    DocKey, DocKeyError, EventBody, EventSeq, ImageRef, NewEvent, NewSession, SessionKind,
    SessionStore, StepOutcome, StepRecord, StoreError, Timestamp, TurnEnd, TurnEvent,
};
use vellum_core::sessions::pricing::Pricing;
use vellum_core::sessions::{TransitionError, record_transition, store_messages, usage_record};
use vellum_core::turns::TurnInput;
use vellum_sessions_sqlite::SqliteSessions;
use vellum_types::{Actor, Origin, SessionId};

use crate::daemon::Documents;
use crate::tools::{Outcome, Session, Step, thumbnail_png};

/// The prices estimated costs come from, shipped with the binary (guide §9).
pub const PRICING_JSON: &str = include_str!("../data/pricing.json");

/// The shipped prices.
pub fn pricing() -> Pricing {
    Pricing::from_json(PRICING_JSON).expect("the shipped pricing.json is valid (tested)")
}

/// Where a daemon whose private folder is `state` (`~/.vellum`) keeps its sessions and their pictures.
pub fn store_paths(state: &Path) -> (PathBuf, PathBuf) {
    (state.join("sessions.db"), state.join("cache"))
}

/// Whether `dir` is a repository root on this computer: it holds `.git` (a folder, or a file in a worktree).
pub fn holds_git(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// The session key of the document at `file` (an absolute path): the nearest enclosing folder that `is_repo`
/// says is a repository root, and the path inside it. A document outside any repository is keyed by its own
/// folder.
pub fn doc_key(file: &Path, is_repo: &dyn Fn(&Path) -> bool) -> Result<DocKey, DocKeyError> {
    let folder = file.parent().ok_or(DocKeyError::PathNotRelative)?;
    let root = folder
        .ancestors()
        .find(|dir| is_repo(dir))
        .unwrap_or(folder);
    let inside = file
        .strip_prefix(root)
        .map_err(|_| DocKeyError::PathNotRelative)?;
    let parts: Vec<String> = inside
        .components()
        .map(|part| match part {
            Component::Normal(name) => Ok(name.to_string_lossy().into_owned()),
            _ => Err(DocKeyError::PathNotRelative),
        })
        .collect::<Result<_, _>>()?;
    DocKey::new(root.to_string_lossy(), parts.join("/"))
}

/// A step as the session log keeps it.
pub fn step_record(step: &Step, thumbnail: Option<ImageRef>) -> StepRecord {
    StepRecord {
        tool: step.tool.clone(),
        arguments: step.arguments.clone(),
        result: step.result.clone(),
        thumbnail,
        started_at: Timestamp(step.started_at),
        duration_ms: u64::try_from(step.duration_ms).unwrap_or(0),
        outcome: match step.outcome {
            Outcome::Done => StepOutcome::Done,
            Outcome::Failed if step.refused => StepOutcome::Rejected,
            Outcome::Failed => StepOutcome::Failed,
        },
        detail: serde_json::to_value(step).expect("a step is plain data"),
    }
}

/// The daemon's session store and the prices it estimates with.
pub struct SessionLog {
    store: Box<dyn SessionStore>,
    pricing: Pricing,
    is_repo: Box<dyn Fn(&Path) -> bool>,
}

impl SessionLog {
    /// A log that finds repository roots on this computer's disk ([`holds_git`]).
    pub fn new(store: Box<dyn SessionStore>, pricing: Pricing) -> Self {
        Self {
            store,
            pricing,
            is_repo: Box::new(holds_git),
        }
    }

    /// Finds repository roots with `is_repo` instead, for documents that do not live on this disk.
    pub fn with_repo_check(mut self, is_repo: impl Fn(&Path) -> bool + 'static) -> Self {
        self.is_repo = Box::new(is_repo);
        self
    }

    /// The SQLite store under `state` (`~/.vellum/sessions.db`, pictures in `~/.vellum/cache/`).
    pub fn open(state: &Path) -> Result<Self, StoreError> {
        let (db, cache) = store_paths(state);
        Ok(Self::new(
            Box::new(SqliteSessions::open(db, cache)?),
            pricing(),
        ))
    }

    pub fn store(&self) -> &dyn SessionStore {
        &*self.store
    }

    pub fn pricing(&self) -> &Pricing {
        &self.pricing
    }

    /// Moves the connection's steps into the store and returns how many were recorded. A step that names no
    /// document goes with the connection's last one; before it has one, such steps are not recorded (there is
    /// no session to put them in). Thumbnails are drawn and cached here.
    pub fn record_steps(
        &self,
        docs: &mut Documents,
        session: &mut Session,
    ) -> Result<usize, StoreError> {
        let mut recorded = 0;
        for step in session.take_steps() {
            if let Some(document) = &step.document {
                session.last_document = Some(document.clone());
            }
            let Some(document) = session.last_document.clone() else {
                continue;
            };
            let (Some(agent), Some(connection)) = (session.agent().cloned(), session.id().cloned())
            else {
                continue;
            };
            let by_path = session
                .logged_paths
                .iter()
                .find(|(path, _)| path == &document)
                .map(|(_, id)| id.clone());
            let id = match by_path {
                Some(id) => id,
                None => {
                    match self.session_for(docs, session, &document, &agent, connection, &step)? {
                        Some(id) => id,
                        None => continue,
                    }
                }
            };
            let thumbnail = match (&step.thumbnail, &step.document) {
                (Some(thumbnail), Some(document)) => {
                    thumbnail_png(docs, session, document, thumbnail)
                        .map(|png| self.store.put_image(&png))
                        .transpose()?
                }
                _ => None,
            };
            let mut origin = Origin::new(Actor::Agent { agent }).in_session(id.clone());
            origin.turn = session.turn().cloned();
            self.store.append(
                &id,
                NewEvent {
                    at: Timestamp(step.started_at.saturating_add(step.duration_ms)),
                    origin,
                    body: EventBody::Step(step_record(&step, thumbnail)),
                },
            )?;
            recorded += 1;
        }
        Ok(recorded)
    }

    /// The session a step on `document` goes to, found by its repository key, made on first use. `None` when the
    /// document has no key here.
    fn session_for(
        &self,
        docs: &mut Documents,
        session: &mut Session,
        document: &str,
        agent: &vellum_types::AgentId,
        connection: SessionId,
        step: &Step,
    ) -> Result<Option<SessionId>, StoreError> {
        let file = match &session.cwd {
            Some(cwd) => cwd.join(document),
            None => PathBuf::from(document),
        };
        let Ok(doc) = doc_key(&file, &*self.is_repo) else {
            return Ok(None);
        };
        let id = match session.logged.iter().find(|(known, _)| known == &doc) {
            Some((_, id)) => id.clone(),
            None => {
                // The connection's own session id for its first document, so History's origins match the
                // log; a fresh one for each further document.
                let id = if session.logged.is_empty() {
                    connection
                } else {
                    SessionId::new(docs.ulid()).expect("a ULID")
                };
                let name = session.agent_name();
                self.store.create(NewSession {
                    id: id.clone(),
                    doc: doc.clone(),
                    kind: SessionKind::External {
                        client: name.clone(),
                    },
                    agent: agent.clone(),
                    title: name,
                    created_at: Timestamp(step.started_at),
                })?;
                session.logged.push((doc, id.clone()));
                id
            }
        };
        Ok(Some(id))
    }

    /// Records what an engine turn reports that the log keeps: usage (with its estimate), the messages for the
    /// next turn's history, and how it ended. `origin` is the turn's agent, session and turn. A turn stopped by
    /// a person was moved to `stopped` when they pressed Stop, so its end adds nothing.
    pub fn record_turn_event(
        &self,
        origin: &Origin,
        model: &str,
        event: &TurnEvent,
        at: Timestamp,
    ) -> Result<Option<EventSeq>, TransitionError> {
        let session = origin
            .session
            .clone()
            .ok_or(TransitionError::Store(StoreError::WrongSession))?;
        let append = |body| {
            self.store
                .append(
                    &session,
                    NewEvent {
                        at,
                        origin: origin.clone(),
                        body,
                    },
                )
                .map(Some)
                .map_err(TransitionError::Store)
        };
        match event {
            TurnEvent::Usage(usage) => append(EventBody::Usage(usage_record(
                Some(&self.pricing),
                model,
                usage,
            ))),
            TurnEvent::Messages(messages) => {
                let stored =
                    store_messages(&*self.store, messages).map_err(TransitionError::Store)?;
                append(EventBody::Messages(stored))
            }
            TurnEvent::Ended(TurnEnd::Done) => {
                record_transition(&*self.store, origin, TurnInput::Done, at)
                    .map(|(_, seq)| Some(seq))
            }
            TurnEvent::Ended(TurnEnd::Failed(_)) => {
                record_transition(&*self.store, origin, TurnInput::Error, at)
                    .map(|(_, seq)| Some(seq))
            }
            TurnEvent::Ended(TurnEnd::Stopped)
            | TurnEvent::Text(_)
            | TurnEvent::Reasoning(_)
            | TurnEvent::ToolStarted { .. }
            | TurnEvent::ToolFinished { .. }
            | TurnEvent::SelfCheck
            | TurnEvent::Compacted { .. } => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_prices_parse_and_price_the_default_model() {
        let pricing = pricing();
        assert_eq!(pricing.checked, "2026-10-16");
        let opus = pricing.prices("claude-opus-5-5").expect("priced");
        assert_eq!(opus.input, Some(4_000_000));
        assert_eq!(opus.output, Some(20_000_000));
        // Every Anthropic model listed prices input and output.
        for model in ["claude-fable-5-1", "claude-sonnet-5-5", "claude-haiku-5-5"] {
            let prices = pricing.prices(model).expect(model);
            assert!(prices.input.is_some() && prices.output.is_some(), "{model}");
        }
    }
}
