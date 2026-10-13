//! The Guard: checked before every agent transaction (guide Invariant 4; ARCHITECTURE §5, §7.5).
//!
//! [`guard`] is pure. Given a checked write, the leases, the pause set, the blocked `(agent, frame)` pairs and the
//! limits, it answers [`Verdict::Allow`] or [`Verdict::Reject`] with a typed [`Rejection`]. Only agents are
//! checked: people, file changes and Vellum's own subsystems always pass (your edits win). For an agent, in order:
//!
//! 1. stopped, then paused (Pause all, or this agent alone);
//! 2. the limits: ops per transaction, then nodes inserted;
//! 3. frames a person stopped this agent on, then frames another agent holds.
//!
//! A write is inside a frame when it sets, inserts under, removes or moves anything in the frame's subtree, or
//! removes or moves the frame or one of its ancestors. The subtree is read in the document before the write: the
//! first edit that reaches into the frame always names a node that was already there, so the batch is caught
//! there. Reading (copying out of a frame) is not a write there. Variables live outside every frame. A lease on a component
//! definition's node also covers replacing that definition.
//!
//! [`apply_guarded`] is the write path for agents: the actor check before validation, then the plan, then the
//! rest of the Guard, then the commit, and finally the activity that extends the agent's own leases. Recording a
//! rejection in History is the caller's job (the daemon owns History).

pub mod reasons;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fmt;

use vellum_types::{Actor, AgentId, ComponentId, Document, Node, NodeId, Origin, PageId, PageKind};

use crate::lease::{BlockedPairs, Leases};
use crate::ops::{self, Applied, Edit, Inputs, OpError, Parent, Plan, Transaction};
use crate::ports::{DocStore, IdGen, Timestamp};

/// Why the Guard rejected an agent's write. Each has a plain-language message from [`reasons`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// Another agent holds a frame the write touches.
    Leased {
        /// The frame.
        frame: NodeId,
        /// Who holds it.
        holder: AgentId,
    },
    /// A person pressed Pause all.
    AllPaused,
    /// A person paused this agent.
    Paused {
        /// The agent.
        agent: AgentId,
    },
    /// A person stopped this agent.
    Stopped {
        /// The agent.
        agent: AgentId,
    },
    /// A person pressed Stop on this agent's lease tab for a frame the write touches.
    StoppedHere {
        /// The agent.
        agent: AgentId,
        /// The frame.
        frame: NodeId,
    },
    /// The transaction has more ops than an agent may send at once.
    TooManyOps {
        /// How many it has.
        count: usize,
        /// The most allowed.
        limit: usize,
    },
    /// The transaction inserts more nodes than an agent may at once.
    TooManyNodes {
        /// How many it inserts.
        count: usize,
        /// The most allowed.
        limit: usize,
    },
}

impl Rejection {
    /// The key of this reason in `@vellum/config`'s `guardReasons`, for hosts that render it themselves.
    pub fn key(&self) -> &'static str {
        match self {
            Self::Leased { .. } => "leased",
            Self::AllPaused => "agentsPaused",
            Self::Paused { .. } => "agentPaused",
            Self::Stopped { .. } => "stopped",
            Self::StoppedHere { .. } => "stoppedHere",
            Self::TooManyOps { .. } => "tooManyChanges",
            Self::TooManyNodes { .. } => "tooManyLayers",
        }
    }

    /// The reason in plain language, with agents named by `name` (a hue such as "Teal", or a client such as
    /// "Claude Code"; the daemon knows which).
    pub fn message(&self, name: &dyn Fn(&AgentId) -> String) -> String {
        let fill = |template: &str, key: &str, value: &str| template.replace(key, value);
        match self {
            Self::Leased { holder, .. } => fill(reasons::LEASED, "{agent}", &name(holder)),
            Self::AllPaused => reasons::AGENTS_PAUSED.to_owned(),
            Self::Paused { agent } => fill(reasons::AGENT_PAUSED, "{agent}", &name(agent)),
            Self::Stopped { .. } => reasons::STOPPED.to_owned(),
            Self::StoppedHere { .. } => reasons::STOPPED_HERE.to_owned(),
            Self::TooManyOps { count, limit } => {
                let text = fill(reasons::TOO_MANY_CHANGES, "{count}", &count.to_string());
                fill(&text, "{limit}", &limit.to_string())
            }
            Self::TooManyNodes { count, limit } => {
                let text = fill(reasons::TOO_MANY_LAYERS, "{count}", &count.to_string());
                fill(&text, "{limit}", &limit.to_string())
            }
        }
    }
}

/// Names agents by id; a host with names calls [`Rejection::message`] instead.
impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(&|agent: &AgentId| agent.to_string()))
    }
}

impl std::error::Error for Rejection {}

/// The Guard's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The write may go ahead.
    Allow {
        /// Frames the agent holds that the write touched; their leases are extended once it is applied.
        own_frames: BTreeSet<NodeId>,
    },
    /// The write is refused.
    Reject(Rejection),
}

/// How much one agent transaction may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most ops in one transaction.
    pub max_ops: usize,
    /// The most nodes one transaction may insert, counting every node of each inserted or copied subtree.
    pub max_inserted_nodes: usize,
}

impl Default for Limits {
    /// 1,000 of each: the same node budget as a script's output (guide Prompt 26).
    fn default() -> Self {
        Self {
            max_ops: 1_000,
            max_inserted_nodes: 1_000,
        }
    }
}

/// Which agents a person paused or stopped. Session state, never file content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PauseSet {
    all: bool,
    paused: BTreeSet<AgentId>,
    stopped: BTreeSet<AgentId>,
}

impl PauseSet {
    /// Pause all.
    pub fn pause_all(&mut self) {
        self.all = true;
    }

    /// Resume after Pause all. Lifts every pause, but not a stop: a stopped agent's turn is over, and its next
    /// turn resumes it with [`PauseSet::resume`].
    pub fn resume_all(&mut self) {
        self.all = false;
        self.paused.clear();
    }

    /// Whether Pause all is on.
    pub fn is_all_paused(&self) -> bool {
        self.all
    }

    /// Pauses one agent.
    pub fn pause(&mut self, agent: AgentId) {
        self.paused.insert(agent);
    }

    /// Stops one agent (Stop, Stop all).
    pub fn stop(&mut self, agent: AgentId) {
        self.stopped.insert(agent);
    }

    /// Lifts a pause or stop on one agent. Pause all stays on if it was.
    pub fn resume(&mut self, agent: &AgentId) {
        self.paused.remove(agent);
        self.stopped.remove(agent);
    }

    /// Whether `agent` may write at all: stopped first, then Pause all, then its own pause.
    pub fn check(&self, agent: &AgentId) -> Result<(), Rejection> {
        if self.stopped.contains(agent) {
            Err(Rejection::Stopped {
                agent: agent.clone(),
            })
        } else if self.all {
            Err(Rejection::AllPaused)
        } else if self.paused.contains(agent) {
            Err(Rejection::Paused {
                agent: agent.clone(),
            })
        } else {
            Ok(())
        }
    }
}

/// A checked write, as the Guard sees it: who, how many ops, the document it was planned against, and the edits.
#[derive(Debug, Clone, Copy)]
pub struct Write<'a> {
    /// Who.
    pub origin: &'a Origin,
    /// How many ops the transaction has.
    pub op_count: usize,
    /// The document the transaction was planned against.
    pub before: &'a Document,
    /// What it changes.
    pub edits: &'a [Edit],
}

impl<'a> Write<'a> {
    /// The write `tx` makes, planned as `plan` against `before`.
    pub fn new(before: &'a Document, tx: &'a Transaction, plan: &'a Plan) -> Self {
        Self {
            origin: &tx.origin,
            op_count: tx.ops.len(),
            before,
            edits: &plan.edits,
        }
    }
}

/// Allows or rejects `write`. Pure: `now` comes from the caller's clock.
pub fn guard(
    write: &Write<'_>,
    leases: &Leases,
    pause: &PauseSet,
    blocked: &BlockedPairs,
    limits: &Limits,
    now: Timestamp,
) -> Verdict {
    let Actor::Agent { agent } = &write.origin.actor else {
        return Verdict::Allow {
            own_frames: BTreeSet::new(),
        };
    };
    match check_agent(write, agent, leases, pause, blocked, limits, now) {
        Ok(own_frames) => Verdict::Allow { own_frames },
        Err(rejection) => Verdict::Reject(rejection),
    }
}

fn check_agent(
    write: &Write<'_>,
    agent: &AgentId,
    leases: &Leases,
    pause: &PauseSet,
    blocked: &BlockedPairs,
    limits: &Limits,
    now: Timestamp,
) -> Result<BTreeSet<NodeId>, Rejection> {
    pause.check(agent)?;
    check_op_count(write.op_count, limits)?;
    let inserted: usize = write
        .edits
        .iter()
        .map(|edit| match edit {
            Edit::Insert { node, .. } => count_nodes(node),
            Edit::InsertPage { page, .. } => page.children.iter().map(count_nodes).sum(),
            _ => 0,
        })
        .sum();
    if inserted > limits.max_inserted_nodes {
        return Err(Rejection::TooManyNodes {
            count: inserted,
            limit: limits.max_inserted_nodes,
        });
    }

    for frame in blocked.frames(agent) {
        if touches(write, frame) {
            return Err(Rejection::StoppedHere {
                agent: agent.clone(),
                frame: frame.clone(),
            });
        }
    }
    let mut own_frames = BTreeSet::new();
    for (frame, holder) in leases.live(now) {
        if !touches(write, frame) {
            continue;
        }
        if holder == agent {
            own_frames.insert(frame.clone());
        } else {
            return Err(Rejection::Leased {
                frame: frame.clone(),
                holder: holder.clone(),
            });
        }
    }
    Ok(own_frames)
}

fn check_op_count(count: usize, limits: &Limits) -> Result<(), Rejection> {
    if count > limits.max_ops {
        Err(Rejection::TooManyOps {
            count,
            limit: limits.max_ops,
        })
    } else {
        Ok(())
    }
}

fn count_nodes(node: &Node) -> usize {
    1 + node.children().iter().map(count_nodes).sum::<usize>()
}

/// Where a frame sits in one document: its subtree, its ancestors and the component it belongs to, if any.
#[derive(Default)]
struct Scope {
    subtree: BTreeSet<NodeId>,
    ancestors: BTreeSet<NodeId>,
    components: BTreeSet<ComponentId>,
    /// The page the frame is on, and that page's kind.
    page: Option<(PageId, PageKind)>,
}

impl Scope {
    /// The frame's scope in the document before the write.
    fn of(frame: &NodeId, write: &Write<'_>) -> Self {
        let mut scope = Self::default();
        scope.add(write.before, frame);
        scope
    }

    fn add(&mut self, doc: &Document, frame: &NodeId) {
        let mut path = Vec::new();
        for page in &doc.pages {
            if self.walk(&page.children, frame, &mut path) {
                self.page = Some((page.id.clone(), page.kind));
                return;
            }
        }
        for (id, component) in &doc.components {
            if self.walk(std::slice::from_ref(&component.root), frame, &mut path) {
                self.components.insert(id.clone());
                return;
            }
        }
    }

    /// Looks for `frame` under `nodes`; when found, records its ancestors (`path`) and subtree.
    fn walk(&mut self, nodes: &[Node], frame: &NodeId, path: &mut Vec<NodeId>) -> bool {
        for node in nodes {
            if node.id() == frame {
                self.ancestors.extend(path.iter().cloned());
                self.collect(node);
                return true;
            }
            path.push(node.id().clone());
            let found = self.walk(node.children(), frame, path);
            path.pop();
            if found {
                return true;
            }
        }
        false
    }

    fn collect(&mut self, node: &Node) {
        self.subtree.insert(node.id().clone());
        for child in node.children() {
            self.collect(child);
        }
    }

    fn under(&self, parent: &Parent) -> bool {
        matches!(parent, Parent::Node(id) if self.subtree.contains(id))
    }

    /// Whether removing or moving `id` takes something in the frame, or the frame itself, with it.
    fn uproots(&self, id: &NodeId) -> bool {
        self.subtree.contains(id) || self.ancestors.contains(id)
    }

    fn hit_by(&self, edit: &Edit) -> bool {
        match edit {
            Edit::Insert { to, .. } => self.under(&to.parent),
            Edit::Remove { id } => self.uproots(id),
            Edit::Move { id, to } => self.uproots(id) || self.under(&to.parent),
            Edit::Set { node } => self.subtree.contains(node.id()),
            Edit::SetComponent { id, .. } => self.components.contains(id),
            Edit::SetVariable { .. } => false,
            // Removing the page takes the frame with it; changing its kind can refuse what it holds.
            Edit::RemovePage { id } => self.page.as_ref().is_some_and(|(page, _)| page == id),
            Edit::SetPage { page } => self
                .page
                .as_ref()
                .is_some_and(|(id, kind)| *id == page.id && *kind != page.kind),
            Edit::InsertPage { .. } | Edit::MovePage { .. } | Edit::SetShell { .. } => false,
        }
    }
}

/// Whether `write` writes inside `frame`.
fn touches(write: &Write<'_>, frame: &NodeId) -> bool {
    let scope = Scope::of(frame, write);
    write.edits.iter().any(|edit| scope.hit_by(edit))
}

/// Everything the Guard checks against, for one open document. Session state, never file content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuardState {
    /// The leases.
    pub leases: Leases,
    /// Paused and stopped agents.
    pub pause: PauseSet,
    /// `(agent, frame)` pairs stopped from a lease tab.
    pub blocked: BlockedPairs,
    /// The per-transaction limits.
    pub limits: Limits,
}

impl GuardState {
    /// Pause all: every agent's next write is rejected and every lease is freed.
    pub fn pause_all(&mut self, now: Timestamp) {
        self.pause.pause_all();
        self.leases.pause_all(now);
    }

    /// Resume after Pause all.
    pub fn resume_all(&mut self) {
        self.pause.resume_all();
    }

    /// Stop on `frame`'s lease tab: the lease is freed and its holder blocked there until resumed.
    pub fn stop_on_lease(&mut self, frame: &NodeId, now: Timestamp) -> Option<AgentId> {
        self.leases.stop(frame, &mut self.blocked, now)
    }

    /// [`guard`] with this state.
    pub fn check(&self, write: &Write<'_>, now: Timestamp) -> Verdict {
        guard(
            write,
            &self.leases,
            &self.pause,
            &self.blocked,
            &self.limits,
            now,
        )
    }
}

/// Why a guarded write did not happen. Nothing was applied either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    /// The transaction is not valid for the document.
    Invalid(OpError),
    /// The Guard rejected it.
    Rejected(Rejection),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Rejected(rejection) => rejection.fmt(f),
        }
    }
}

impl std::error::Error for WriteError {}

/// Checks `tx` with the Guard and applies it, or refuses it whole and leaves the store untouched. A paused or
/// stopped agent is rejected before its ops are even validated, so a running engine learns at once that it was
/// paused. On success the agent's own leases it wrote inside are extended.
pub fn apply_guarded<S: DocStore + ?Sized>(
    store: &mut S,
    ids: &mut dyn IdGen,
    tx: Transaction,
    inputs: &Inputs<'_>,
    state: &mut GuardState,
    now: Timestamp,
) -> Result<Applied, WriteError> {
    let agent = match &tx.origin.actor {
        Actor::Agent { agent } => Some(agent.clone()),
        _ => None,
    };
    if let Some(agent) = &agent {
        state.pause.check(agent).map_err(WriteError::Rejected)?;
        check_op_count(tx.ops.len(), &state.limits).map_err(WriteError::Rejected)?;
    }
    let before = store.snapshot();
    let plan = ops::plan_with_inputs(&before, &tx, ids, inputs).map_err(WriteError::Invalid)?;
    let own_frames = match state.check(&Write::new(&before, &tx, &plan), now) {
        Verdict::Allow { own_frames } => own_frames,
        Verdict::Reject(rejection) => return Err(WriteError::Rejected(rejection)),
    };
    let applied = ops::commit(store, tx.origin, plan);
    if let Some(agent) = &agent {
        for frame in &own_frames {
            state.leases.activity(frame, agent, now);
        }
    }
    Ok(applied)
}
