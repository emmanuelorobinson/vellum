//! Document history: per-actor undo and redo, and "Undo this turn" (Invariant 3).
//!
//! A store records every change it applies here: its own commits with the inverse `apply_edits` returned, and
//! changes that arrived from elsewhere as the state before and after. Undo replays an inverse, but adjusted to
//! the document as it is now, so that it never takes back somebody else's later work:
//!
//! - a property someone changed since keeps their value;
//! - a node someone has built on (drawn into, changed, moved) is not removed;
//! - a node someone moved since is not moved back;
//! - a variable or component someone changed since is not put back.
//!
//! What is left as it is, is "kept". ⌘Z ([`History::undo`]) always takes the actor's latest change back as
//! far as it can. "Undo this turn" ([`History::undo_turn`]) takes a turn back outright only when nothing is
//! kept; otherwise it builds a [`Revert`] and changes nothing, and the revert becomes a new, attributed
//! transaction when it is applied.
//!
//! Undo works from recorded inverses, never by replaying a CRDT's history: Loro's undo is per peer rather than
//! per actor and grows with the document (ADR-004). It still copies the document once and looks nodes up by
//! walking it, so it costs edits × nodes; ADR-004's "Prompt 14" section has the numbers.

mod footprint;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;
use vellum_types::{Actor, Document, NodeId, NodePatch, Origin, TurnId};

pub(crate) use footprint::Footprint;

use crate::ops::advanced::validate_references;
use crate::ops::edit::{Touched, apply_one, find, locate, subtree_ids};
use crate::ops::{Edit, Parent, Place};
use crate::ports::{ChangeSet, Kept, Revert};

/// One recorded change.
#[derive(Debug, Clone)]
struct Entry {
    /// When, in this history's own order.
    seq: u64,
    origin: Origin,
    /// The edits that take it back, in order. Empty for a change that arrived from elsewhere.
    inverse: Vec<Edit>,
    footprint: Footprint,
    /// Not undone.
    live: bool,
    /// Made here, so undo may take it back.
    local: bool,
}

/// What an undo, a redo or an undone turn applied: the edits, for the store to write to its backend, and
/// what they changed.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The edits applied, in order.
    pub edits: Vec<Edit>,
    /// Their inverse, as `apply_edits` returns it.
    pub inverse: Vec<Edit>,
    /// What they changed, attributed to whoever undid.
    pub changes: ChangeSet,
}

/// What [`History::undo_turn`] did.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnUndo {
    /// The turn has nothing left to undo.
    Nothing,
    /// Undone outright; the document has changed.
    Undone(Step),
    /// Later work depends on the turn; the document has not changed.
    Revert(Revert),
}

/// The history of one store's document, for one session. Session-scoped: Git is the permanent history.
#[derive(Debug, Clone, Default)]
pub struct History {
    entries: Vec<Entry>,
    redo: BTreeMap<Actor, Vec<Entry>>,
    seq: u64,
}

impl History {
    /// An empty history.
    pub fn new() -> Self {
        Self::default()
    }

    fn tick(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    fn push(&mut self, origin: Origin, inverse: Vec<Edit>, footprint: Footprint, local: bool) {
        let seq = self.tick();
        self.entries.push(Entry {
            seq,
            origin,
            inverse,
            footprint,
            live: true,
            local,
        });
    }

    /// Records a commit the store applied: `edits` as applied and `inverse` exactly as `apply_edits` returned
    /// it. Clears what the actor could redo.
    pub fn record(&mut self, origin: &Origin, edits: &[Edit], inverse: Vec<Edit>) {
        let footprint = Footprint::of(edits, &inverse);
        self.redo.remove(&origin.actor);
        self.push(origin.clone(), inverse, footprint, true);
    }

    /// Records changes that arrived from elsewhere, as the document before and after them, and returns what
    /// changed. They are never undone here, but count as later work for every undo.
    pub fn record_remote(
        &mut self,
        before: &Document,
        after: &Document,
        origin: Origin,
    ) -> ChangeSet {
        let (footprint, changes) = Footprint::between(before, after, origin.clone());
        if !changes.is_empty() {
            self.push(origin, Vec::new(), footprint, false);
        }
        changes
    }

    /// What every live change from `after` on touched, leaving out `skip`.
    fn later(&self, after: u64, skip: &BTreeSet<usize>) -> Footprint {
        let mut later = Footprint::default();
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.live && entry.seq > after && !skip.contains(&i) {
                later.absorb(&entry.footprint);
            }
        }
        later
    }

    /// Takes back the actor's latest change that is not undone, as far as it can without touching later work
    /// by others, and applies that to `doc`. None when there is nothing to undo.
    pub fn undo(&mut self, doc: &mut Document, actor: &Actor) -> Option<Step> {
        let at = self
            .entries
            .iter()
            .rposition(|e| e.live && e.local && &e.origin.actor == actor)?;
        let entry = &self.entries[at];
        let later = self.later(entry.seq, &BTreeSet::from([at]));
        let targets = [(entry.inverse.as_slice(), &entry.footprint, &later)];
        let done = Reverting::run(doc, &targets)?;
        let origin = entry.origin.clone();
        self.entries[at].live = false;
        *doc = done.document;
        let footprint = Footprint::of(&done.edits, &done.inverse);
        let seq = self.tick();
        self.redo.entry(actor.clone()).or_default().push(Entry {
            seq,
            origin,
            inverse: done.inverse.clone(),
            footprint,
            live: false,
            local: true,
        });
        Some(Step {
            edits: done.edits,
            inverse: done.inverse,
            changes: done.touched.into_change_set(Origin::new(actor.clone())),
        })
    }

    /// Puts back what the actor's latest undo took back, keeping work done since. None when there is nothing
    /// to redo.
    pub fn redo(&mut self, doc: &mut Document, actor: &Actor) -> Option<Step> {
        let undone = self.redo.get_mut(actor)?.pop()?;
        // Since the undo, the actor's own changes can only be redos of older changes (a new change clears what
        // they could redo): those are theirs to build on, not later work to keep.
        let own: BTreeSet<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.seq > undone.seq && &e.origin.actor == actor)
            .map(|(i, _)| i)
            .collect();
        let later = self.later(undone.seq, &own);
        let targets = [(undone.inverse.as_slice(), &undone.footprint, &later)];
        let Some(done) = Reverting::run(doc, &targets) else {
            self.redo.entry(actor.clone()).or_default().push(undone);
            return None;
        };
        *doc = done.document;
        let footprint = Footprint::of(&done.edits, &done.inverse);
        self.push(undone.origin, done.inverse.clone(), footprint, true);
        Some(Step {
            edits: done.edits,
            inverse: done.inverse,
            changes: done.touched.into_change_set(Origin::new(actor.clone())),
        })
    }

    /// "Undo this turn", asked for by `by`: takes back every live change made here whose origin carries
    /// `turn`, newest first. When nothing later depends on them they are undone and `doc` changes; otherwise
    /// `doc` stays as it is and the result is a [`Revert`] to apply as a new transaction.
    pub fn undo_turn(&mut self, doc: &mut Document, turn: &TurnId, by: &Actor) -> TurnUndo {
        let targets: BTreeSet<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.live && e.local && e.origin.turn.as_ref() == Some(turn))
            .map(|(i, _)| i)
            .collect();
        let Some(&first) = targets.first() else {
            return TurnUndo::Nothing;
        };
        // Walk back from the newest change; each target is reverted against everything after it that is not
        // part of the turn.
        let mut later = Footprint::default();
        let mut laters = BTreeMap::new();
        for i in (first..self.entries.len()).rev() {
            let entry = &self.entries[i];
            if targets.contains(&i) {
                laters.insert(i, later.clone());
            } else if entry.live {
                later.absorb(&entry.footprint);
            }
        }
        let runs: Vec<_> = targets
            .iter()
            .rev()
            .map(|i| {
                let entry = &self.entries[*i];
                (entry.inverse.as_slice(), &entry.footprint, &laters[i])
            })
            .collect();
        let Some(done) = Reverting::run(doc, &runs) else {
            return TurnUndo::Nothing;
        };
        let origin = Origin::new(by.clone());
        if !done.kept.is_empty() {
            return TurnUndo::Revert(Revert::new(
                origin,
                done.edits,
                done.kept.into_iter().collect(),
            ));
        }
        for i in &targets {
            self.entries[*i].live = false;
        }
        *doc = done.document;
        TurnUndo::Undone(Step {
            edits: done.edits,
            inverse: done.inverse,
            changes: done.touched.into_change_set(origin),
        })
    }
}

/// The result of reverting: the document it leads to, the edits that get there with their inverse (in the
/// order to apply it), what changed, and what was kept.
struct Done {
    document: Document,
    edits: Vec<Edit>,
    inverse: Vec<Edit>,
    touched: Touched,
    kept: BTreeSet<Kept>,
}

/// Reverts changes on a copy of the document, one adjusted edit at a time.
struct Reverting {
    work: Document,
    edits: Vec<Edit>,
    inverse: Vec<Edit>,
    touched: Touched,
    kept: BTreeSet<Kept>,
    /// Check references after every edit and skip the ones that break them. Slow (a copy of the document per
    /// edit); used only when the fast pass leaves a broken reference.
    strict: bool,
}

type Target<'a> = (&'a [Edit], &'a Footprint, &'a Footprint);

impl Reverting {
    /// Reverts each target (its inverse, its own footprint, what later work touched) in order. None when the
    /// result cannot be made consistent, which a document that passed its checks does not reach.
    fn run(doc: &Document, targets: &[Target<'_>]) -> Option<Done> {
        Self::pass(doc, targets, false).or_else(|| Self::pass(doc, targets, true))
    }

    fn pass(doc: &Document, targets: &[Target<'_>], strict: bool) -> Option<Done> {
        let mut run = Self {
            work: doc.clone(),
            edits: Vec::new(),
            inverse: Vec::new(),
            touched: Touched::default(),
            kept: BTreeSet::new(),
            strict,
        };
        for (inverse, own, later) in targets {
            for edit in *inverse {
                if let Some(edit) = run.adjust(edit, own, later) {
                    run.apply(edit);
                }
            }
        }
        validate_references(&run.work).ok()?;
        run.inverse.reverse();
        Some(Done {
            document: run.work,
            edits: run.edits,
            inverse: run.inverse,
            touched: run.touched,
            kept: run.kept,
        })
    }

    fn apply(&mut self, edit: Edit) {
        if self.strict {
            let mut probe = self.work.clone();
            let fits = apply_one(&mut probe, &edit, &mut Touched::default()).is_ok()
                && validate_references(&probe).is_ok();
            if !fits {
                self.kept.insert(kept(&edit));
                return;
            }
        }
        match apply_one(&mut self.work, &edit, &mut self.touched) {
            Ok(inverse) => {
                self.edits.push(edit);
                self.inverse.push(inverse);
            }
            Err(_) => {
                self.kept.insert(kept(&edit));
            }
        }
    }

    fn keep(&mut self, what: Kept) {
        self.kept.insert(what);
    }

    fn parent_exists(&self, parent: &Parent) -> bool {
        match parent {
            Parent::Page(page) => self.work.pages.iter().any(|p| &p.id == page),
            Parent::Node(node) => find(&self.work, node).is_some(),
        }
    }

    fn child_count(&self, parent: &Parent) -> usize {
        match parent {
            Parent::Page(page) => self
                .work
                .pages
                .iter()
                .find(|p| &p.id == page)
                .map_or(0, |p| p.children.len()),
            Parent::Node(node) => find(&self.work, node).map_or(0, |n| n.children().len()),
        }
    }

    /// `edit`, one edit of an inverse, as it should apply to the document now; None to leave it out.
    fn adjust(&mut self, edit: &Edit, own: &Footprint, later: &Footprint) -> Option<Edit> {
        match edit {
            Edit::Set { node: before } => {
                let id = before.id();
                if own.inserted.contains(id) {
                    // Made by the change itself: its removal follows.
                    return None;
                }
                let keys = own.props.get(id)?;
                let current = find(&self.work, id)?;
                if current.kind() != before.kind() {
                    self.keep(Kept::Node(id.clone()));
                    return None;
                }
                let theirs = later.props.get(id);
                let old = before.property_patch();
                let mut restore = BTreeMap::new();
                let mut kept_any = false;
                for key in keys {
                    if theirs.is_some_and(|t| t.contains(key)) {
                        kept_any = true;
                        continue;
                    }
                    let value = old.properties().get(key).cloned().unwrap_or(Json::Null);
                    restore.insert(key.clone(), value);
                }
                let node = (!restore.is_empty())
                    .then(|| NodePatch::new(restore).ok())
                    .flatten()
                    .map(|patch| current.patched(&patch).ok());
                if kept_any {
                    self.keep(Kept::Node(id.clone()));
                }
                match node? {
                    Some(node) => Some(Edit::Set {
                        node: Box::new(node),
                    }),
                    None => {
                        self.keep(Kept::Node(id.clone()));
                        None
                    }
                }
            }
            Edit::Remove { id } => {
                let node = find(&self.work, id)?;
                let mut ids = Vec::new();
                subtree_ids(node, &mut ids);
                if ids.iter().any(|i| later.touches_node(i)) {
                    self.keep(Kept::Node(id.clone()));
                    return None;
                }
                Some(edit.clone())
            }
            Edit::Insert { to, node } => {
                if !self.parent_exists(&to.parent) {
                    self.keep(Kept::Node(node.id().clone()));
                    return None;
                }
                let index = to.index.min(self.child_count(&to.parent));
                Some(Edit::Insert {
                    to: Place {
                        parent: to.parent.clone(),
                        index,
                    },
                    node: node.clone(),
                })
            }
            Edit::Move { id, to } => {
                let from = locate(&self.work, id)?;
                if later.moved.contains(id) || !self.parent_exists(&to.parent) {
                    self.keep(Kept::Node(id.clone()));
                    return None;
                }
                let same_parent = !from.is_component_root() && from.place().parent == to.parent;
                let len = self.child_count(&to.parent) - usize::from(same_parent);
                Some(Edit::Move {
                    id: id.clone(),
                    to: Place {
                        parent: to.parent.clone(),
                        index: to.index.min(len),
                    },
                })
            }
            Edit::SetComponent { id, component } => {
                // Putting back another tree removes the one there now: not when someone built on it since
                // (changed, moved or drew into a node of it). Making it is not building on it.
                let built_on = self.work.components.get(id).is_some_and(|c| {
                    if component.as_ref().is_some_and(|next| next.root == c.root) {
                        return false;
                    }
                    let mut ids: Vec<NodeId> = Vec::new();
                    subtree_ids(&c.root, &mut ids);
                    ids.iter().any(|i| {
                        later.props.contains_key(i)
                            || later.moved.contains(i)
                            || later.parents.contains(i)
                    })
                });
                if later.components.contains(id) || built_on {
                    self.keep(Kept::Component(id.clone()));
                    return None;
                }
                Some(edit.clone())
            }
            Edit::SetVariable { id, .. } => {
                if later.variables.contains(id) {
                    self.keep(Kept::Variable(id.clone()));
                    return None;
                }
                Some(edit.clone())
            }
            Edit::InsertPage { index, page } => {
                if self.work.pages.iter().any(|p| p.id == page.id) {
                    self.keep(Kept::Page(page.id.clone()));
                    return None;
                }
                Some(Edit::InsertPage {
                    index: (*index).min(self.work.pages.len()),
                    page: page.clone(),
                })
            }
            Edit::RemovePage { id } => {
                let page = self.work.pages.iter().find(|p| &p.id == id)?;
                let mut ids = Vec::new();
                page.children.iter().for_each(|n| subtree_ids(n, &mut ids));
                let changed = later.pages_set.contains(id) || later.pages_moved.contains(id);
                if changed || ids.iter().any(|i| later.touches_node(i)) {
                    self.keep(Kept::Page(id.clone()));
                    return None;
                }
                Some(edit.clone())
            }
            Edit::SetPage { page } => {
                self.work.pages.iter().find(|p| p.id == page.id)?;
                if later.pages_set.contains(&page.id) {
                    self.keep(Kept::Page(page.id.clone()));
                    return None;
                }
                Some(edit.clone())
            }
            Edit::MovePage { id, index } => {
                self.work.pages.iter().find(|p| &p.id == id)?;
                if later.pages_moved.contains(id) {
                    self.keep(Kept::Page(id.clone()));
                    return None;
                }
                Some(Edit::MovePage {
                    id: id.clone(),
                    index: (*index).min(self.work.pages.len() - 1),
                })
            }
            Edit::SetShell { .. } => {
                if later.shell {
                    self.keep(Kept::Shell);
                    return None;
                }
                Some(edit.clone())
            }
        }
    }
}

/// What an edit that no longer applies leaves as it is.
fn kept(edit: &Edit) -> Kept {
    match edit {
        Edit::Insert { node, .. } => Kept::Node(node.id().clone()),
        Edit::Set { node } => Kept::Node(node.id().clone()),
        Edit::Remove { id } | Edit::Move { id, .. } => Kept::Node(id.clone()),
        Edit::SetComponent { id, .. } => Kept::Component(id.clone()),
        Edit::SetVariable { id, .. } => Kept::Variable(id.clone()),
        Edit::InsertPage { page, .. } | Edit::SetPage { page } => Kept::Page(page.id.clone()),
        Edit::RemovePage { id } | Edit::MovePage { id, .. } => Kept::Page(id.clone()),
        Edit::SetShell { .. } => Kept::Shell,
    }
}
