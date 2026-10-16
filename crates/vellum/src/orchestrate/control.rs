//! What a person presses on running agents: Stop (the session's turn), Stop this agent (one lane), Stop all,
//! Pause all, and Undo this turn (guide Invariants 3, 4, 12; §10.1 rule 1; DESIGN.md "Composer", "Agent Presence").
//!
//! Each press interrupts the engines at once and puts its Guard change ([`AgentControl`]) where every agent
//! transaction is checked. When the documents are busy with a run, the change waits in the orchestrator and is
//! applied before that run's next tool call, so no agent's next write gets past it.
use vellum_core::guard::AgentControl;
use vellum_core::parallel::{LaneEnd, ParallelEvent};
use vellum_core::ports::{ChangeSet, EventBody, TurnUndone};
use vellum_core::sessions::{record_transition, turn_state};
use vellum_core::turns::{TurnInput, TurnState};
use vellum_types::{AgentId, TurnId};

use super::{OrchestrationError, Orchestrator, TurnSpec, not_now};
use crate::daemon::Documents;
use crate::tools::ToolError;

impl Orchestrator<'_> {
    /// Applies `change` now when the documents are at hand, or before the next tool call of the run that holds
    /// them.
    fn press(&self, docs: Option<&mut Documents>, change: AgentControl) {
        self.pending.borrow_mut().push(change);
        if let Some(docs) = docs {
            self.flush(docs);
        }
    }

    /// The agents working in `turn` and the engines they run on.
    fn working(&self, turn: &TurnId) -> Vec<(AgentId, usize)> {
        self.running
            .borrow()
            .get(turn)
            .map(|run| {
                run.lanes
                    .iter()
                    .filter(|lane| !lane.ended)
                    .map(|lane| (lane.agent.clone(), lane.engine))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Stop, pressed by the person on the session: the turn is stopped, every lane's engine interrupted and every
    /// lane's agent refused by the Guard until its next turn. Everything already built stays. A plan is abandoned:
    /// when a run is in progress it clears the placeholders as it ends; otherwise pass `docs` and they go now.
    pub fn stop(
        &self,
        docs: Option<&mut Documents>,
        spec: &TurnSpec,
    ) -> Result<TurnState, OrchestrationError> {
        self.stop_turn(docs, spec, true)
    }

    fn stop_turn(
        &self,
        mut docs: Option<&mut Documents>,
        spec: &TurnSpec,
        refuse: bool,
    ) -> Result<TurnState, OrchestrationError> {
        let at = docs
            .as_deref()
            .map_or(self.now.get(), |docs| self.clock(docs));
        let person = Self::person(spec);
        let (state, _) = record_transition(self.log.store(), &person, TurnInput::Stop, at)?;
        let working = self.working(&spec.turn);
        if working.is_empty() {
            self.engine.interrupt(&spec.turn);
        }
        for (agent, engine) in &working {
            self.engine_at(*engine).interrupt(&spec.turn);
            if refuse {
                self.press(docs.as_deref_mut(), AgentControl::Stop(agent.clone()));
            }
        }
        if !self.running.borrow().contains_key(&spec.turn) {
            self.abandon(docs, spec, &person, at)?;
        }
        Ok(state)
    }

    /// Stop this agent: one lane of the turn ends, its engine interrupted and its next write refused; the other
    /// lanes go on. Stopping the last working lane stops the turn. Returns the turn's state.
    pub fn stop_agent(
        &self,
        docs: Option<&mut Documents>,
        spec: &TurnSpec,
        agent: &AgentId,
    ) -> Result<TurnState, OrchestrationError> {
        let working = self.working(&spec.turn);
        let Some((_, engine)) = working.iter().find(|(a, _)| a == agent) else {
            return Err(OrchestrationError::NoLane(agent.clone()));
        };
        if working.len() == 1 {
            return self.stop(docs, spec);
        }
        {
            let mut running = self.running.borrow_mut();
            let run = running.get_mut(&spec.turn).expect("it is working");
            for lane in run.lanes.iter_mut().filter(|lane| &lane.agent == agent) {
                lane.halted = true;
            }
        }
        let at = docs
            .as_deref()
            .map_or(self.now.get(), |docs| self.clock(docs));
        self.engine_at(*engine).interrupt(&spec.turn);
        self.press(docs, AgentControl::Stop(agent.clone()));
        self.append(
            &Self::person(spec),
            EventBody::Parallel(ParallelEvent::LaneEnded {
                agent: agent.clone(),
                end: LaneEnd::Stopped,
            }),
            at,
        )?;
        let events = self.events(spec)?;
        Ok(turn_state(&events, &spec.turn).unwrap_or(TurnState::Queued))
    }

    /// Stop all: every turn with agents at work is stopped, as Stop does to one. Agents outside them (an external
    /// client) are not paused; that is Pause all.
    pub fn stop_all(
        &self,
        mut docs: Option<&mut Documents>,
    ) -> Result<Vec<(TurnId, TurnState)>, OrchestrationError> {
        let specs: Vec<TurnSpec> = self
            .running
            .borrow()
            .values()
            .map(|run| run.spec.clone())
            .collect();
        specs
            .iter()
            .map(|spec| Ok((spec.turn.clone(), self.stop(docs.as_deref_mut(), spec)?)))
            .collect()
    }

    /// Pause all agents: every agent's next write, built-in or external and in every open file, is refused by the
    /// Guard ("Agents paused"); every turn at work is stopped by the person (§10.1 rule 1) and its engines
    /// interrupted. Returns the stopped turns. Lifted by [`Self::resume_all`].
    pub fn pause_all(
        &self,
        mut docs: Option<&mut Documents>,
    ) -> Result<Vec<TurnId>, OrchestrationError> {
        self.press(docs.as_deref_mut(), AgentControl::PauseAll);
        let specs: Vec<TurnSpec> = self
            .running
            .borrow()
            .values()
            .map(|run| run.spec.clone())
            .collect();
        for spec in &specs {
            self.stop_turn(docs.as_deref_mut(), spec, false)?;
        }
        Ok(specs.into_iter().map(|spec| spec.turn).collect())
    }

    /// Resume after Pause all.
    pub fn resume_all(&self, docs: &mut Documents) {
        self.press(Some(docs), AgentControl::ResumeAll);
    }

    /// Undo this turn, pressed by the person on a turn that has ended: every transaction of every lane of the
    /// turn is taken back, and nobody else's change is lost (a Revert transaction keeps what others built on).
    pub fn undo_turn(
        &self,
        docs: &mut Documents,
        spec: &TurnSpec,
    ) -> Result<TurnUndone, OrchestrationError> {
        let state = turn_state(&self.events(spec)?, &spec.turn);
        if self.running.borrow().contains_key(&spec.turn)
            || !state.is_some_and(TurnState::is_terminal)
        {
            return Err(not_now());
        }
        let undone = docs
            .target(&spec.document)
            .and_then(|target| target.undo_turn(&spec.turn, &spec.person))
            .map_err(ToolError::from)?;
        let (changed, kept) = match &undone {
            TurnUndone::Nothing => return Ok(undone),
            TurnUndone::Undone(changes) => (count(changes), 0),
            TurnUndone::Reverted { applied, kept } => (count(&applied.changes), kept.len()),
        };
        let at = self.clock(docs);
        self.append(
            &Self::person(spec),
            EventBody::Undo {
                changed,
                kept: u32::try_from(kept).unwrap_or(u32::MAX),
            },
            at,
        )?;
        Ok(undone)
    }
}

/// Layers and variables a change touched.
fn count(changes: &ChangeSet) -> u32 {
    let n = changes.inserted.len()
        + changes.updated.len()
        + changes.removed.len()
        + changes.variables.len();
    u32::try_from(n).unwrap_or(u32::MAX)
}
