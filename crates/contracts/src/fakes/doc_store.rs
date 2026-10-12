use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use vellum_core::ops::{Edit, Parent, Place, apply_edits};
use vellum_core::ports::{ChangeSet, Commit, DocStore, SyncCursor, SyncError, SyncPayload};
use vellum_types::{
    Actor, Component, ComponentId, Document, Node, NodeId, Origin, PageId, Subsystem, Variable,
    VariableId,
};

/// An edit as the fake writes it into sync payloads. `vellum-core` has no serde, so the fake keeps its own
/// spelling and converts.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum Step {
    Insert {
        page: Option<PageId>,
        parent: Option<NodeId>,
        index: usize,
        node: Box<Node>,
    },
    Remove {
        id: NodeId,
    },
    Move {
        id: NodeId,
        page: Option<PageId>,
        parent: Option<NodeId>,
        index: usize,
    },
    Set {
        node: Box<Node>,
    },
    SetComponent {
        id: ComponentId,
        component: Option<Box<Component>>,
    },
    SetVariable {
        id: VariableId,
        variable: Option<Box<Variable>>,
    },
}

fn split(place: &Place) -> (Option<PageId>, Option<NodeId>, usize) {
    match &place.parent {
        Parent::Page(page) => (Some(page.clone()), None, place.index),
        Parent::Node(node) => (None, Some(node.clone()), place.index),
    }
}

fn join(page: Option<PageId>, parent: Option<NodeId>, index: usize) -> Option<Place> {
    let parent = match (page, parent) {
        (_, Some(node)) => Parent::Node(node),
        (Some(page), None) => Parent::Page(page),
        (None, None) => return None,
    };
    Some(Place { parent, index })
}

impl From<&Edit> for Step {
    fn from(edit: &Edit) -> Self {
        match edit.clone() {
            Edit::Insert { to, node } => {
                let (page, parent, index) = split(&to);
                Self::Insert {
                    page,
                    parent,
                    index,
                    node,
                }
            }
            Edit::Remove { id } => Self::Remove { id },
            Edit::Move { id, to } => {
                let (page, parent, index) = split(&to);
                Self::Move {
                    id,
                    page,
                    parent,
                    index,
                }
            }
            Edit::Set { node } => Self::Set { node },
            Edit::SetComponent { id, component } => Self::SetComponent { id, component },
            Edit::SetVariable { id, variable } => Self::SetVariable { id, variable },
        }
    }
}

impl Step {
    fn into_edit(self) -> Option<Edit> {
        Some(match self {
            Self::Insert {
                page,
                parent,
                index,
                node,
            } => Edit::Insert {
                to: join(page, parent, index)?,
                node,
            },
            Self::Remove { id } => Edit::Remove { id },
            Self::Move {
                id,
                page,
                parent,
                index,
            } => Edit::Move {
                id,
                to: join(page, parent, index)?,
            },
            Self::Set { node } => Edit::Set { node },
            Self::SetComponent { id, component } => Edit::SetComponent { id, component },
            Self::SetVariable { id, variable } => Edit::SetVariable { id, variable },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    peer: u64,
    seq: u64,
    origin: Origin,
    steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Wire {
    entries: Vec<Entry>,
}

/// A document in memory, with per-actor undo and redo kept as lists of inverse edits, and sync by replaying a
/// log of edits. It does what a CRDT does only for edits that arrive one after another: two stores that edit
/// at the same time do not converge (Prompt 14's Loro store does).
#[derive(Debug)]
pub struct InMemoryDocStore {
    doc: Document,
    peer: u64,
    log: Vec<Entry>,
    seen: BTreeMap<u64, u64>,
    undo: BTreeMap<Actor, Vec<Vec<Edit>>>,
    redo: BTreeMap<Actor, Vec<Vec<Edit>>>,
}

impl InMemoryDocStore {
    /// A store holding `doc`. `peer` names this replica in sync; two stores that exchange updates need different
    /// ones.
    pub fn new(doc: &Document, peer: u64) -> Self {
        Self {
            doc: doc.clone(),
            peer,
            log: vec![],
            seen: BTreeMap::new(),
            undo: BTreeMap::new(),
            redo: BTreeMap::new(),
        }
    }

    fn record(&mut self, origin: &Origin, edits: &[Edit]) {
        let seq = self.seen.get(&self.peer).copied().unwrap_or(0) + 1;
        self.seen.insert(self.peer, seq);
        self.log.push(Entry {
            peer: self.peer,
            seq,
            origin: origin.clone(),
            steps: edits.iter().map(Step::from).collect(),
        });
    }

    fn step_back(&mut self, actor: &Actor, forward: bool) -> Option<ChangeSet> {
        let (from, to) = if forward {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        let edits = from.get_mut(actor)?.pop()?;
        let origin = Origin::new(actor.clone());
        match apply_edits(&mut self.doc, &edits, origin.clone()) {
            Ok((inverse, changes)) => {
                to.entry(actor.clone()).or_default().push(inverse);
                self.record(&origin, &edits);
                Some(changes)
            }
            Err(_) => {
                from.entry(actor.clone()).or_default().push(edits);
                None
            }
        }
    }
}

impl DocStore for InMemoryDocStore {
    fn adapter_id(&self) -> &'static str {
        "in-memory"
    }

    fn snapshot(&self) -> Document {
        self.doc.clone()
    }

    fn apply(&mut self, commit: Commit) -> ChangeSet {
        let (origin, edits, inverse, changes) = commit.into_parts();
        apply_edits(&mut self.doc, &edits, origin.clone())
            .expect("a commit was checked against this document");
        let actor = origin.actor.clone();
        self.undo.entry(actor.clone()).or_default().push(inverse);
        self.redo.remove(&actor);
        self.record(&origin, &edits);
        changes
    }

    fn undo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        self.step_back(actor, false)
    }

    fn redo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        self.step_back(actor, true)
    }

    fn cursor(&self) -> SyncCursor {
        SyncCursor(serde_json::to_vec(&self.seen).expect("a map of numbers serialises"))
    }

    fn export_updates(&self, since: &SyncCursor) -> SyncPayload {
        let known: BTreeMap<u64, u64> = serde_json::from_slice(&since.0).unwrap_or_default();
        let entries = self
            .log
            .iter()
            .filter(|e| e.seq > known.get(&e.peer).copied().unwrap_or(0))
            .cloned()
            .collect();
        SyncPayload {
            adapter: self.adapter_id().into(),
            bytes: serde_json::to_vec(&Wire { entries }).expect("entries serialise"),
        }
    }

    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError> {
        if payload.adapter != self.adapter_id() {
            return Err(SyncError::AdapterMismatch {
                expected: self.adapter_id().into(),
                found: payload.adapter.clone(),
            });
        }
        let wire: Wire = serde_json::from_slice(&payload.bytes)
            .map_err(|e| SyncError::Malformed(e.to_string()))?;
        let mut seen = self.seen.clone();
        let mut fresh = Vec::new();
        let mut edits = Vec::new();
        for entry in wire.entries {
            if entry.seq <= seen.get(&entry.peer).copied().unwrap_or(0) {
                continue;
            }
            for step in entry.steps.iter().cloned() {
                edits.push(
                    step.into_edit()
                        .ok_or_else(|| SyncError::Malformed("a step has no place".into()))?,
                );
            }
            seen.insert(entry.peer, entry.seq);
            fresh.push(entry);
        }
        let origin = fresh.last().map_or_else(
            || {
                Origin::new(Actor::System {
                    subsystem: Subsystem::Sync,
                })
            },
            |e| e.origin.clone(),
        );
        let (_, changes) = apply_edits(&mut self.doc, &edits, origin)
            .map_err(|(index, kind)| SyncError::Malformed(format!("step {index}: {kind}")))?;
        self.seen = seen;
        self.log.extend(fresh);
        Ok(changes)
    }
}
