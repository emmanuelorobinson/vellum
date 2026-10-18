//! Plans and placeholders (ARCHITECTURE §7.5; DESIGN.md "Plan card", "Agent Presence").
//!
//! A request that will make more than one frame or more than about twenty changes is first answered with a plan:
//! the frames it will create or change, each with a page and a one-line intent. The person approves it (Build),
//! asks again (Edit request), or stops. Build places a **placeholder** for every new frame: a reservation held by
//! the building agent, with planned bounds found by `find_space`, that every agent sees through `get_app_state`.
//! Placeholders are session state, never file content, so an abandoned plan leaves nothing in the file. Items
//! tick off as the agent reports them done (`report_progress`).
//!
//! Everything here is pure. The session log keeps the plan as [`PlanEvent`]s; [`plan_state`] replays them.
#[cfg(test)]
mod tests;

use crate::ports::{EventBody, Rect, SessionEvent};
use std::collections::BTreeSet;
use std::fmt;
use vellum_types::{AgentId, NodeId, PageId, TurnId};

/// The most items one plan holds.
pub const ITEM_LIMIT: usize = 100;
/// The longest item name, in characters.
pub const NAME_LIMIT: usize = 100;
/// The longest one-line intent, in characters.
pub const INTENT_LIMIT: usize = 500;
/// The longest plan title, in characters.
pub const TITLE_LIMIT: usize = 200;
/// The largest planned width or height, in canvas units.
pub const MAX_SIDE: f64 = 100_000.0;

/// One frame the plan creates or changes.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanItem {
    /// Its number in the plan, from 1. Stays the same when the person reorders or removes items.
    pub id: u32,
    /// The frame's name.
    pub name: String,
    /// The page it goes on.
    pub page: PageId,
    /// What it is for, in one line.
    pub intent: String,
    /// The planned size of a new frame.
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// The existing frame it changes; `None` for a new frame.
    pub frame: Option<NodeId>,
}
impl PlanItem {
    /// Whether the item makes a new frame (and so gets a placeholder).
    pub fn is_new(&self) -> bool {
        self.frame.is_none()
    }
}

/// What an engine proposes before building.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// "Onboarding · 4 screens".
    pub title: String,
    pub items: Vec<PlanItem>,
    /// The engine's estimate of how many changes the work takes, when it gave one.
    pub changes: Option<u32>,
}
impl Plan {
    pub fn item(&self, id: u32) -> Option<&PlanItem> {
        self.items.iter().find(|item| item.id == id)
    }
}

/// When a request needs a plan. The owner chose three frames (DESIGN-DECISIONS, Prompt 51).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanSettings {
    /// A request making or changing at least this many frames is planned. A person may raise it to skip plans
    /// for requests under a size they choose.
    pub min_frames: usize,
    /// A request estimated at more changes than this is planned, whatever its frame count.
    pub max_unplanned_changes: u32,
}
impl Default for PlanSettings {
    fn default() -> Self {
        Self {
            min_frames: 3,
            max_unplanned_changes: 20,
        }
    }
}
impl PlanSettings {
    /// Whether `plan` is over the threshold, so the person approves it before anything is built.
    pub fn needs_plan(&self, plan: &Plan) -> bool {
        plan.items.len() >= self.min_frames.max(1)
            || plan.changes.unwrap_or(0) > self.max_unplanned_changes
    }
}

/// What is wrong with a plan, or with a tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    NoTitle,
    NoItems,
    TooManyItems,
    DuplicateItem(u32),
    NoName(u32),
    NoIntent(u32),
    /// A new frame without a usable width and height.
    NoSize(u32),
    /// The item's field is longer than its limit.
    TooLong(u32, &'static str),
    /// The title is longer than [`TITLE_LIMIT`].
    TitleTooLong,
    /// No item has this number.
    UnknownItem(u32),
}
impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTitle => f.write_str("Give the plan a title."),
            Self::NoItems => f.write_str("A plan has at least one item."),
            Self::TooManyItems => write!(f, "A plan has at most {ITEM_LIMIT} items."),
            Self::DuplicateItem(id) => write!(f, "Two items are numbered {id}."),
            Self::NoName(id) => write!(f, "Give item {id} a name."),
            Self::NoIntent(id) => write!(f, "Say in one line what item {id} is for."),
            Self::NoSize(id) => write!(
                f,
                "Item {id} is a new frame: give its width and height, above 0 and at most {MAX_SIDE}."
            ),
            Self::TooLong(id, field) => {
                let limit = if *field == "name" {
                    NAME_LIMIT
                } else {
                    INTENT_LIMIT
                };
                write!(
                    f,
                    "Item {id}'s {field} is too long; keep it under {limit} characters."
                )
            }
            Self::TitleTooLong => {
                write!(
                    f,
                    "The title is too long; keep it under {TITLE_LIMIT} characters."
                )
            }
            Self::UnknownItem(id) => write!(f, "The plan has no item {id}."),
        }
    }
}
impl std::error::Error for PlanError {}

fn side(value: Option<f64>) -> bool {
    value.is_some_and(|v| v.is_finite() && v > 0.0 && v <= MAX_SIDE)
}

/// Checks a plan as proposed or as the person edited it. Pages and frames are checked against the document by
/// the host, which has it.
pub fn check(plan: &Plan) -> Result<(), PlanError> {
    if plan.title.trim().is_empty() {
        return Err(PlanError::NoTitle);
    }
    if plan.title.chars().count() > TITLE_LIMIT {
        return Err(PlanError::TitleTooLong);
    }
    if plan.items.is_empty() {
        return Err(PlanError::NoItems);
    }
    if plan.items.len() > ITEM_LIMIT {
        return Err(PlanError::TooManyItems);
    }
    let mut seen = BTreeSet::new();
    for item in &plan.items {
        if !seen.insert(item.id) {
            return Err(PlanError::DuplicateItem(item.id));
        }
        if item.name.trim().is_empty() {
            return Err(PlanError::NoName(item.id));
        }
        if item.name.chars().count() > NAME_LIMIT {
            return Err(PlanError::TooLong(item.id, "name"));
        }
        if item.intent.trim().is_empty() {
            return Err(PlanError::NoIntent(item.id));
        }
        if item.intent.chars().count() > INTENT_LIMIT {
            return Err(PlanError::TooLong(item.id, "intent"));
        }
        if item.is_new() && !(side(item.width) && side(item.height)) {
            return Err(PlanError::NoSize(item.id));
        }
    }
    Ok(())
}

/// A planned frame on the canvas before it exists: the frame id the agent will create it with, where it will go,
/// and who holds it. Session state; the document never holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Placeholder {
    /// The plan item it stands for.
    pub item: u32,
    /// The id reserved for the frame.
    pub frame: NodeId,
    /// The item's name, which the frame will carry: the placeholder's label.
    pub name: String,
    pub page: PageId,
    /// Planned position and size, clear of the page's frames and every other placeholder.
    pub bounds: Rect,
    /// The agent building it.
    pub agent: AgentId,
    /// The turn whose plan it belongs to.
    pub turn: TurnId,
}

/// What the session log records about a turn's plan. The event's origin carries the turn.
#[derive(Clone, Debug, PartialEq)]
pub enum PlanEvent {
    /// The engine proposed a plan over the threshold; the turn waits for the person. A later proposal in the same
    /// turn (after Edit request) replaces it.
    Proposed(Plan),
    /// The person pressed Build with this plan (as they edited it); its placeholders were placed.
    Built {
        plan: Plan,
        placeholders: Vec<Placeholder>,
    },
    /// The agent reported these items done.
    Ticked(Vec<u32>),
    /// The turn finished; what is left of the placeholders is gone.
    Finished,
    /// The plan was stopped or failed; its placeholders are gone and nothing was left in the file.
    Abandoned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanStatus {
    /// Waiting for the person.
    Proposed,
    Building,
    Finished,
    Abandoned,
}

/// A turn's plan as the Plan card shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanState {
    pub plan: Plan,
    pub status: PlanStatus,
    /// Items reported done.
    pub done: BTreeSet<u32>,
    /// Placeholders still standing.
    pub placeholders: Vec<Placeholder>,
}
impl PlanState {
    pub fn proposed(plan: Plan) -> Self {
        Self {
            plan,
            status: PlanStatus::Proposed,
            done: BTreeSet::new(),
            placeholders: vec![],
        }
    }

    /// Marks `items` done and returns the ones that were not done yet. Their placeholders go: from now on the
    /// frame itself stands there. Refuses whole when an item is not in the plan.
    pub fn tick(&mut self, items: &[u32]) -> Result<Vec<u32>, PlanError> {
        if let Some(unknown) = items.iter().find(|id| self.plan.item(**id).is_none()) {
            return Err(PlanError::UnknownItem(*unknown));
        }
        let mut new = vec![];
        for id in items {
            if self.done.insert(*id) {
                new.push(*id);
            }
        }
        self.placeholders.retain(|p| !self.done.contains(&p.item));
        Ok(new)
    }

    /// Items done and items in the plan ("5 of 12 screens done").
    pub fn progress(&self) -> (usize, usize) {
        (self.done.len(), self.plan.items.len())
    }

    /// Applies one logged event.
    pub fn apply(state: Option<Self>, event: &PlanEvent) -> Option<Self> {
        match event {
            PlanEvent::Proposed(plan) => Some(Self::proposed(plan.clone())),
            PlanEvent::Built { plan, placeholders } => Some(Self {
                plan: plan.clone(),
                status: PlanStatus::Building,
                done: BTreeSet::new(),
                placeholders: placeholders.clone(),
            }),
            PlanEvent::Ticked(items) => state.map(|mut state| {
                // The log only holds ticks the host accepted; anything else is ignored rather than refused.
                let known: Vec<u32> = items
                    .iter()
                    .copied()
                    .filter(|id| state.plan.item(*id).is_some())
                    .collect();
                let _ = state.tick(&known);
                state
            }),
            PlanEvent::Finished | PlanEvent::Abandoned => state.map(|mut state| {
                state.status = if matches!(event, PlanEvent::Finished) {
                    PlanStatus::Finished
                } else {
                    PlanStatus::Abandoned
                };
                state.placeholders.clear();
                state
            }),
        }
    }
}

/// `turn`'s plan, replayed from the session log. `None` when the turn never proposed one.
pub fn plan_state(events: &[SessionEvent], turn: &TurnId) -> Option<PlanState> {
    events
        .iter()
        .filter(|event| event.origin.turn.as_ref() == Some(turn))
        .fold(None, |state, event| match &event.body {
            EventBody::Plan(plan_event) => PlanState::apply(state, plan_event),
            _ => state,
        })
}

fn number(value: f64) -> String {
    // Planned bounds are whole canvas units in practice; print them without a trailing ".0".
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// What the building agent is told after Build: every item, the frame id and bounds of each placeholder, and how
/// to report progress.
pub fn build_prompt(plan: &Plan, placeholders: &[Placeholder]) -> String {
    let mut out = format!(
        "The person approved the plan \"{}\". Build it now, one item at a time. Create each new frame with the id, \
         page, position and size of its placeholder, claim it, fill it, check it, then call report_progress with \
         the item's number in `items`.\n",
        plan.title
    );
    for item in &plan.items {
        let what = match (&item.frame, placeholders.iter().find(|p| p.item == item.id)) {
            (Some(frame), _) => format!("change frame `{frame}`"),
            (None, Some(p)) => format!(
                "new frame `{}` at x {}, y {}, {} × {}",
                p.frame,
                number(p.bounds.x),
                number(p.bounds.y),
                number(p.bounds.width),
                number(p.bounds.height)
            ),
            (None, None) => "new frame".to_owned(),
        };
        out.push_str(&format!(
            "{}. {} (page {}): {what}. {}\n",
            item.id, item.name, item.page, item.intent
        ));
    }
    out
}
