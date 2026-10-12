use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use vellum_core::ports::{
    ActorId, ChangeSet, DocStore, Op, OpError, OpErrorKind, Origin, SessionId, SyncCursor,
    SyncError, SyncPayload, Transaction, TurnId,
};
use vellum_types::{Document, Node, NodeId, PageId};

/// The one thing the store knows how to do to a document. Every op, undo and import is a list of these, and
/// each has an inverse, which is how undo works. Replace keeps the children the node has now.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum Step {
    Insert {
        page: PageId,
        parent: Option<NodeId>,
        index: usize,
        node: Box<Node>,
    },
    Remove {
        id: NodeId,
    },
    Replace {
        node: Box<Node>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireOrigin {
    actor: String,
    session: Option<String>,
    turn: Option<String>,
}

impl From<&Origin> for WireOrigin {
    fn from(o: &Origin) -> Self {
        Self {
            actor: o.actor.to_string(),
            session: o.session.as_ref().map(ToString::to_string),
            turn: o.turn.as_ref().map(ToString::to_string),
        }
    }
}

impl From<&WireOrigin> for Origin {
    fn from(o: &WireOrigin) -> Self {
        Self {
            actor: ActorId::new(o.actor.clone()),
            session: o.session.clone().map(SessionId::new),
            turn: o.turn.clone().map(TurnId::new),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    peer: u64,
    seq: u64,
    origin: WireOrigin,
    steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Wire {
    entries: Vec<Entry>,
}

#[derive(Default)]
struct Touch {
    inserted: BTreeSet<NodeId>,
    updated: BTreeSet<NodeId>,
    removed: BTreeSet<NodeId>,
}

impl Touch {
    fn insert(&mut self, id: NodeId) {
        if self.removed.remove(&id) {
            self.updated.insert(id);
        } else {
            self.inserted.insert(id);
        }
    }

    fn remove(&mut self, id: NodeId) {
        self.updated.remove(&id);
        if !self.inserted.remove(&id) {
            self.removed.insert(id);
        }
    }

    fn update(&mut self, id: NodeId) {
        if !self.inserted.contains(&id) {
            self.updated.insert(id);
        }
    }

    fn into_change_set(self, origin: Origin) -> ChangeSet {
        ChangeSet {
            origin,
            inserted: self.inserted.into_iter().collect(),
            updated: self.updated.into_iter().collect(),
            removed: self.removed.into_iter().collect(),
        }
    }
}

fn subtree_ids(node: &Node, out: &mut Vec<NodeId>) {
    out.push(node.id().clone());
    node.children().iter().for_each(|c| subtree_ids(c, out));
}

fn all_ids(doc: &Document) -> BTreeSet<NodeId> {
    let mut ids = Vec::new();
    doc.pages
        .iter()
        .flat_map(|p| &p.children)
        .for_each(|n| subtree_ids(n, &mut ids));
    ids.into_iter().collect()
}

fn find_mut<'a>(nodes: &'a mut [Node], id: &NodeId) -> Option<&'a mut Node> {
    for node in nodes {
        if node.id() == id {
            return Some(node);
        }
        if let Some(children) = node.children_mut()
            && let Some(found) = find_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}

fn find_node_mut<'a>(doc: &'a mut Document, id: &NodeId) -> Option<&'a mut Node> {
    doc.pages
        .iter_mut()
        .find_map(|p| find_mut(&mut p.children, id))
}

/// Where a node sits: its page, its parent and its index among the siblings.
fn locate(doc: &Document, id: &NodeId) -> Option<(PageId, Option<NodeId>, usize)> {
    fn walk(nodes: &[Node], parent: Option<&Node>, id: &NodeId) -> Option<(Option<NodeId>, usize)> {
        for (i, node) in nodes.iter().enumerate() {
            if node.id() == id {
                return Some((parent.map(|p| p.id().clone()), i));
            }
            if let Some(found) = walk(node.children(), Some(node), id) {
                return Some(found);
            }
        }
        None
    }
    doc.pages
        .iter()
        .find_map(|p| walk(&p.children, None, id).map(|(parent, i)| (p.id.clone(), parent, i)))
}

/// Does one step; returns its inverse.
fn apply_step(doc: &mut Document, step: &Step, touch: &mut Touch) -> Result<Step, OpErrorKind> {
    match step {
        Step::Insert {
            page,
            parent,
            index,
            node,
        } => {
            let mut new_ids = Vec::new();
            subtree_ids(node, &mut new_ids);
            let taken = all_ids(doc);
            let mut seen = BTreeSet::new();
            for id in &new_ids {
                if taken.contains(id) || !seen.insert(id) {
                    return Err(OpErrorKind::DuplicateId(id.clone()));
                }
            }
            let page_ref = doc
                .pages
                .iter_mut()
                .find(|p| &p.id == page)
                .ok_or_else(|| OpErrorKind::UnknownPage(page.clone()))?;
            let siblings = match parent {
                None => &mut page_ref.children,
                Some(parent) => {
                    let container = find_mut(&mut page_ref.children, parent)
                        .ok_or_else(|| OpErrorKind::UnknownNode(parent.clone()))?;
                    container
                        .children_mut()
                        .ok_or_else(|| OpErrorKind::NotAContainer(parent.clone()))?
                }
            };
            if *index > siblings.len() {
                return Err(OpErrorKind::IndexOutOfRange {
                    index: *index,
                    len: siblings.len(),
                });
            }
            siblings.insert(*index, (**node).clone());
            new_ids.into_iter().for_each(|id| touch.insert(id));
            Ok(Step::Remove {
                id: node.id().clone(),
            })
        }
        Step::Remove { id } => {
            let (page, parent, index) =
                locate(doc, id).ok_or_else(|| OpErrorKind::UnknownNode(id.clone()))?;
            let page_ref = doc
                .pages
                .iter_mut()
                .find(|p| p.id == page)
                .expect("located on this page");
            let siblings = match &parent {
                None => &mut page_ref.children,
                Some(parent) => find_mut(&mut page_ref.children, parent)
                    .and_then(Node::children_mut)
                    .expect("located inside this parent"),
            };
            let node = siblings.remove(index);
            let mut gone = Vec::new();
            subtree_ids(&node, &mut gone);
            gone.into_iter().for_each(|id| touch.remove(id));
            Ok(Step::Insert {
                page,
                parent,
                index,
                node: Box::new(node),
            })
        }
        Step::Replace { node } => {
            let target = find_node_mut(doc, node.id())
                .ok_or_else(|| OpErrorKind::UnknownNode(node.id().clone()))?;
            let before = target.clone();
            let mut next = node.clone();
            if let (Some(kids), Some(slot)) =
                (Some(target.children().to_vec()), next.children_mut())
            {
                *slot = kids;
            }
            *target = *next;
            touch.update(node.id().clone());
            Ok(Step::Replace {
                node: Box::new(before),
            })
        }
    }
}

fn patched(node: &Node, patch: &vellum_types::NodePatch) -> Result<Node, OpErrorKind> {
    let invalid = |message: String| OpErrorKind::InvalidPatch { message };
    let mut json = serde_json::to_value(node).map_err(|e| invalid(e.to_string()))?;
    if let Json::Object(map) = &mut json {
        map.extend(patch.properties().clone());
    }
    let next: Node = serde_json::from_value(json).map_err(|e| invalid(e.to_string()))?;
    if next.id() != node.id() || next.kind() != node.kind() {
        return Err(invalid("a patch cannot change what a layer is".into()));
    }
    Ok(next)
}

/// A document in memory, with per-actor undo and redo kept as lists of inverse steps, and sync by replaying a
/// log of steps. It does what a CRDT does only for edits that arrive one after another: two stores that edit at
/// the same time do not converge (Prompt 14's Loro store does).
#[derive(Debug)]
pub struct InMemoryDocStore {
    doc: Document,
    peer: u64,
    log: Vec<Entry>,
    seen: BTreeMap<u64, u64>,
    undo: BTreeMap<ActorId, Vec<Vec<Step>>>,
    redo: BTreeMap<ActorId, Vec<Vec<Step>>>,
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

    fn record(&mut self, origin: &Origin, steps: Vec<Step>) {
        let seq = self.seen.get(&self.peer).copied().unwrap_or(0) + 1;
        self.seen.insert(self.peer, seq);
        self.log.push(Entry {
            peer: self.peer,
            seq,
            origin: origin.into(),
            steps,
        });
    }

    /// Runs `steps` on a copy; on success returns the copy, what changed and the steps that reverse it, last
    /// first.
    fn run(&self, steps: &[Step]) -> Result<(Document, Touch, Vec<Step>), (usize, OpErrorKind)> {
        let mut work = self.doc.clone();
        let mut touch = Touch::default();
        let mut inverses = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            inverses.push(apply_step(&mut work, step, &mut touch).map_err(|kind| (i, kind))?);
        }
        inverses.reverse();
        Ok((work, touch, inverses))
    }

    fn step_back(&mut self, actor: &ActorId, forward: bool) -> Option<ChangeSet> {
        let stack = if forward {
            &mut self.redo
        } else {
            &mut self.undo
        };
        let steps = stack.get_mut(actor)?.pop()?;
        match self.run(&steps) {
            Ok((doc, touch, inverses)) => {
                self.doc = doc;
                let other = if forward {
                    &mut self.undo
                } else {
                    &mut self.redo
                };
                other.entry(actor.clone()).or_default().push(inverses);
                let origin = Origin::new(actor.clone());
                self.record(&origin, steps);
                Some(touch.into_change_set(origin))
            }
            Err(_) => {
                let stack = if forward {
                    &mut self.redo
                } else {
                    &mut self.undo
                };
                stack.entry(actor.clone()).or_default().push(steps);
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

    fn apply(&mut self, tx: Transaction) -> Result<ChangeSet, OpError> {
        if tx.ops.is_empty() {
            return Err(OpError {
                op_index: 0,
                kind: OpErrorKind::Empty,
            });
        }
        let mut work = self.doc.clone();
        let mut touch = Touch::default();
        let (mut forward, mut inverses) = (Vec::new(), Vec::new());
        for (op_index, op) in tx.ops.iter().enumerate() {
            let fail = |kind| OpError { op_index, kind };
            let step = match op {
                Op::Insert {
                    page,
                    parent,
                    index,
                    node,
                } => Step::Insert {
                    page: page.clone(),
                    parent: parent.clone(),
                    index: *index,
                    node: node.clone(),
                },
                Op::Delete { id } => Step::Remove { id: id.clone() },
                Op::Update { id, patch } => {
                    let current = find_node_mut(&mut work, id)
                        .ok_or_else(|| fail(OpErrorKind::UnknownNode(id.clone())))?;
                    Step::Replace {
                        node: Box::new(patched(current, patch).map_err(fail)?),
                    }
                }
            };
            inverses.push(apply_step(&mut work, &step, &mut touch).map_err(fail)?);
            forward.push(step);
        }
        inverses.reverse();
        self.doc = work;
        let actor = tx.origin.actor.clone();
        self.undo.entry(actor.clone()).or_default().push(inverses);
        self.redo.remove(&actor);
        self.record(&tx.origin, forward);
        Ok(touch.into_change_set(tx.origin))
    }

    fn undo(&mut self, actor: &ActorId) -> Option<ChangeSet> {
        self.step_back(actor, false)
    }

    fn redo(&mut self, actor: &ActorId) -> Option<ChangeSet> {
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
        let mut work = self.doc.clone();
        let mut touch = Touch::default();
        let mut fresh = Vec::new();
        let mut seen = self.seen.clone();
        for entry in wire.entries {
            if entry.seq <= seen.get(&entry.peer).copied().unwrap_or(0) {
                continue;
            }
            for step in &entry.steps {
                apply_step(&mut work, step, &mut touch)
                    .map_err(|kind| SyncError::Malformed(format!("{kind:?}")))?;
            }
            seen.insert(entry.peer, entry.seq);
            fresh.push(entry);
        }
        let origin = fresh.last().map_or_else(
            || Origin::new(ActorId::new("sync")),
            |e| Origin::from(&e.origin),
        );
        self.doc = work;
        self.seen = seen;
        self.log.extend(fresh);
        Ok(touch.into_change_set(origin))
    }
}
