//! Leases: a frame an agent is working on (guide §10.2; ARCHITECTURE §7.5).
//!
//! ```text
//! free ──claim(agent)──▶ held(agent, expires_at) ──activity──▶ held (expiry extended 60 s)
//!                           │ release | expiry | Stop on lease tab (person) | Pause all
//!                           ▼
//!                         free
//! ```
//!
//! [`LeaseState::next`] is the machine for one frame; [`Leases`] keeps one per frame. Leases are session state,
//! never file content. Time comes in as a [`Timestamp`] from an injected clock; a lease whose `expires_at` has
//! come is free whether or not [`Leases::expire`] has dropped it yet. A held lease never blocks a person: the
//! Guard only checks agents.

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use vellum_types::{AgentId, NodeId};

use crate::ports::Timestamp;

/// How long a lease lasts without activity: 60 s.
pub const LEASE_IDLE_MS: i64 = 60_000;

/// One frame's lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseState {
    /// Nobody holds it.
    Free,
    /// An agent holds it until `expires_at` (exclusive).
    Held {
        /// The holder.
        agent: AgentId,
        /// When it lapses without activity.
        expires_at: Timestamp,
    },
}

/// What can happen to a lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseEvent {
    /// An agent claims the frame (`claim_frame`). The holder claiming again extends it.
    Claim(AgentId),
    /// An agent wrote inside the frame. Extends the holder's lease; anyone else's activity changes nothing.
    Activity(AgentId),
    /// An agent lets go (`release_frame`).
    Release(AgentId),
    /// Time passed.
    Expire,
    /// A person pressed Stop on the lease tab. [`Leases::stop`] also blocks the pair.
    Stop,
    /// A person pressed Pause all.
    PauseAll,
}

/// Why a lease event was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseError {
    /// Another agent holds the frame.
    LeasedBy(AgentId),
    /// Nobody holds the frame, so there is nothing to release.
    NotHeld,
    /// A person stopped this agent on this frame; it may not claim it until resumed.
    Blocked,
}

impl fmt::Display for LeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LeasedBy(agent) => write!(f, "the frame is leased by {agent}"),
            Self::NotHeld => f.write_str("the frame is not leased"),
            Self::Blocked => f.write_str("stopped on this frame until resumed"),
        }
    }
}

impl std::error::Error for LeaseError {}

impl LeaseState {
    /// The holder at `now`; none when free or expired.
    pub fn holder(&self, now: Timestamp) -> Option<&AgentId> {
        match self {
            Self::Held { agent, expires_at } if now < *expires_at => Some(agent),
            _ => None,
        }
    }

    /// The state after `event` at `now`. An expired lease behaves as free.
    pub fn next(&self, event: &LeaseEvent, now: Timestamp) -> Result<Self, LeaseError> {
        let extended = |agent: &AgentId| Self::Held {
            agent: agent.clone(),
            expires_at: Timestamp(now.0.saturating_add(LEASE_IDLE_MS)),
        };
        let holder = self.holder(now);
        match (event, holder) {
            (LeaseEvent::Claim(agent), None) => Ok(extended(agent)),
            (LeaseEvent::Claim(agent), Some(holder)) if holder == agent => Ok(extended(agent)),
            (LeaseEvent::Claim(_), Some(holder)) => Err(LeaseError::LeasedBy(holder.clone())),
            (LeaseEvent::Activity(agent), Some(holder)) if holder == agent => Ok(extended(agent)),
            (LeaseEvent::Activity(_), Some(_)) => Ok(self.clone()),
            (LeaseEvent::Release(agent), Some(holder)) if holder == agent => Ok(Self::Free),
            (LeaseEvent::Release(_), Some(holder)) => Err(LeaseError::LeasedBy(holder.clone())),
            (LeaseEvent::Release(_), None) => Err(LeaseError::NotHeld),
            (LeaseEvent::Expire, Some(_)) => Ok(self.clone()),
            (LeaseEvent::Stop | LeaseEvent::PauseAll, Some(_))
            | (
                LeaseEvent::Activity(_)
                | LeaseEvent::Expire
                | LeaseEvent::Stop
                | LeaseEvent::PauseAll,
                None,
            ) => Ok(Self::Free),
        }
    }
}

/// `(agent, frame)` pairs a person stopped with the lease tab's Stop. The agent may not claim or write inside the
/// frame until the pair is resumed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockedPairs(BTreeSet<(AgentId, NodeId)>);

impl BlockedPairs {
    /// Blocks `agent` on `frame`.
    pub fn block(&mut self, agent: AgentId, frame: NodeId) {
        self.0.insert((agent, frame));
    }

    /// Lifts the block on `agent` at `frame`.
    pub fn resume(&mut self, agent: &AgentId, frame: &NodeId) {
        self.0.remove(&(agent.clone(), frame.clone()));
    }

    /// Lifts every block on `agent`.
    pub fn resume_agent(&mut self, agent: &AgentId) {
        self.0.retain(|(blocked, _)| blocked != agent);
    }

    /// Whether `agent` is blocked on `frame`.
    pub fn contains(&self, agent: &AgentId, frame: &NodeId) -> bool {
        self.0.contains(&(agent.clone(), frame.clone()))
    }

    /// The frames `agent` is blocked on.
    pub fn frames<'a>(&'a self, agent: &'a AgentId) -> impl Iterator<Item = &'a NodeId> + 'a {
        self.0
            .iter()
            .filter(move |(blocked, _)| blocked == agent)
            .map(|(_, frame)| frame)
    }

    /// Whether nobody is blocked anywhere.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The leases of one open document, by frame. A frame not in the table is free.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Leases(BTreeMap<NodeId, LeaseState>);

impl Leases {
    /// `frame`'s lease.
    pub fn state(&self, frame: &NodeId) -> LeaseState {
        self.0.get(frame).cloned().unwrap_or(LeaseState::Free)
    }

    /// Who holds `frame` at `now`.
    pub fn holder(&self, frame: &NodeId, now: Timestamp) -> Option<&AgentId> {
        self.0.get(frame).and_then(|state| state.holder(now))
    }

    /// Every frame held at `now`, with its holder.
    pub fn live(&self, now: Timestamp) -> impl Iterator<Item = (&NodeId, &AgentId)> + '_ {
        self.0
            .iter()
            .filter_map(move |(frame, state)| state.holder(now).map(|agent| (frame, agent)))
    }

    /// `agent` claims `frame`, unless another agent holds it or a person stopped `agent` there.
    pub fn claim(
        &mut self,
        frame: &NodeId,
        agent: AgentId,
        blocked: &BlockedPairs,
        now: Timestamp,
    ) -> Result<(), LeaseError> {
        if blocked.contains(&agent, frame) {
            return Err(LeaseError::Blocked);
        }
        self.event(frame, &LeaseEvent::Claim(agent), now)
    }

    /// `agent` wrote inside `frame`: its lease there is extended. Nothing happens otherwise.
    pub fn activity(&mut self, frame: &NodeId, agent: &AgentId, now: Timestamp) {
        let _ = self.event(frame, &LeaseEvent::Activity(agent.clone()), now);
    }

    /// `agent` lets go of `frame`.
    pub fn release(
        &mut self,
        frame: &NodeId,
        agent: &AgentId,
        now: Timestamp,
    ) -> Result<(), LeaseError> {
        self.event(frame, &LeaseEvent::Release(agent.clone()), now)
    }

    /// Drops the leases that have lapsed at `now` and names their frames.
    pub fn expire(&mut self, now: Timestamp) -> Vec<NodeId> {
        let lapsed: Vec<NodeId> = self
            .0
            .iter()
            .filter(|(_, state)| state.holder(now).is_none())
            .map(|(frame, _)| frame.clone())
            .collect();
        for frame in &lapsed {
            self.0.remove(frame);
        }
        lapsed
    }

    /// A person pressed Stop on `frame`'s lease tab: the lease is freed and its holder blocked there. Returns the
    /// holder, if there was one.
    pub fn stop(
        &mut self,
        frame: &NodeId,
        blocked: &mut BlockedPairs,
        now: Timestamp,
    ) -> Option<AgentId> {
        let holder = self.holder(frame, now).cloned();
        let _ = self.event(frame, &LeaseEvent::Stop, now);
        if let Some(agent) = &holder {
            blocked.block(agent.clone(), frame.clone());
        }
        holder
    }

    /// A person pressed Pause all: every lease is freed. Nobody is blocked.
    pub fn pause_all(&mut self, now: Timestamp) {
        let frames: Vec<NodeId> = self.0.keys().cloned().collect();
        for frame in frames {
            let _ = self.event(&frame, &LeaseEvent::PauseAll, now);
        }
    }

    fn event(
        &mut self,
        frame: &NodeId,
        event: &LeaseEvent,
        now: Timestamp,
    ) -> Result<(), LeaseError> {
        match self.state(frame).next(event, now)? {
            LeaseState::Free => {
                self.0.remove(frame);
            }
            held => {
                self.0.insert(frame.clone(), held);
            }
        }
        Ok(())
    }
}
