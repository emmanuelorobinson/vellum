//! Parallel agents: Split, Side by side, Explore and spawned helpers (ARCHITECTURE §7.5; DESIGN.md "Composer",
//! C05, C07; guide Prompt 39).
//!
//! One person's turn can have several **lanes**: each lane is its own engine run by its own agent (with its own
//! leases and hue), and every lane's changes carry the same turn, so "Undo this turn" takes all of them back.
//!
//! - *Split* divides an approved plan's items into lanes of about equal size, in plan order.
//! - *Side by side* gives every lane the whole brief and its own frame, for the person to compare (options A, B…).
//! - *Explore* builds one result, then N−1 successive variants that change only Layout or only Style.
//! - *Spawned* lanes are helpers an agent started with `spawn_agents`, each with an explicit task.
//!
//! Everything here is pure: the split, the words each lane is told, and the [`ParallelEvent`]s the session log
//! keeps, which [`parallel_state`] replays.
#[cfg(test)]
mod tests;

use std::fmt;

use vellum_types::{AgentId, NodeId, TurnId};

use crate::plans::{Placeholder, Plan, build_prompt};
use crate::ports::{EventBody, SessionEvent};

/// The most agents one turn runs at once (DESIGN.md "Parallel (1–6 agents)").
pub const MAX_LANES: usize = 6;
/// Explore makes 2 to 6 results, the first included (DESIGN.md "Explore (off, or 2–6)").
pub const MIN_VARIANTS: usize = 2;
pub const MAX_VARIANTS: usize = 6;

/// What Explore varies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Layout,
    Style,
}

/// How a turn's lanes came to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Split,
    SideBySide,
    Explore(Axis),
    /// Helpers an agent started with `spawn_agents`.
    Spawned,
}
impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Split => "split",
            Self::SideBySide => "side_by_side",
            Self::Explore(Axis::Layout) => "explore_layout",
            Self::Explore(Axis::Style) => "explore_style",
            Self::Spawned => "spawned",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Split,
            Self::SideBySide,
            Self::Explore(Axis::Layout),
            Self::Explore(Axis::Style),
            Self::Spawned,
        ]
        .into_iter()
        .find(|mode| mode.as_str() == name)
    }
}

/// How a lane ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaneEnd {
    Done,
    Failed,
    /// A person stopped it: Stop this agent, Stop, Stop all or Pause all.
    Stopped,
}
impl LaneEnd {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Done, Self::Failed, Self::Stopped]
            .into_iter()
            .find(|end| end.as_str() == name)
    }
}

/// A count of lanes or variants out of range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParallelError {
    Lanes(usize),
    Variants(usize),
}
impl fmt::Display for ParallelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lanes(n) => write!(f, "{n} agents at once; choose 1 to {MAX_LANES}."),
            Self::Variants(n) => write!(
                f,
                "{n} variants; Explore makes {MIN_VARIANTS} to {MAX_VARIANTS}."
            ),
        }
    }
}
impl std::error::Error for ParallelError {}

pub fn check_lanes(n: usize) -> Result<(), ParallelError> {
    if (1..=MAX_LANES).contains(&n) {
        Ok(())
    } else {
        Err(ParallelError::Lanes(n))
    }
}

pub fn check_variants(n: usize) -> Result<(), ParallelError> {
    if (MIN_VARIANTS..=MAX_VARIANTS).contains(&n) {
        Ok(())
    } else {
        Err(ParallelError::Variants(n))
    }
}

/// Divides `items` into at most `lanes` contiguous lanes, in order, whose sizes differ by at most one; the first
/// lanes take the extra items. Never an empty lane.
pub fn split(items: &[u32], lanes: usize) -> Vec<Vec<u32>> {
    let lanes = lanes.min(items.len());
    if lanes == 0 {
        return vec![];
    }
    let (size, extra) = (items.len() / lanes, items.len() % lanes);
    let mut out = Vec::with_capacity(lanes);
    let mut at = 0;
    for n in 0..lanes {
        let take = size + usize::from(n < extra);
        out.push(items[at..at + take].to_vec());
        at += take;
    }
    out
}

/// "A", "B", … for option 1, 2, … (DESIGN.md C07).
pub fn option_label(option: u32) -> String {
    char::from_u32(u32::from('A') + option.saturating_sub(1))
        .filter(char::is_ascii_uppercase)
        .map_or_else(|| option.to_string(), String::from)
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn area(p: &Placeholder) -> String {
    format!(
        "new frame `{}` named \"{}\" on page {} at x {}, y {}, {} × {}",
        p.frame,
        p.name,
        p.page,
        number(p.bounds.x),
        number(p.bounds.y),
        number(p.bounds.width),
        number(p.bounds.height)
    )
}

/// What a Split lane is told: its share of the plan, and that `lanes` agents build the rest at the same time.
pub fn split_prompt(
    plan: &Plan,
    items: &[u32],
    placeholders: &[Placeholder],
    lanes: usize,
) -> String {
    let share = Plan {
        title: plan.title.clone(),
        items: plan
            .items
            .iter()
            .filter(|item| items.contains(&item.id))
            .cloned()
            .collect(),
        changes: None,
    };
    format!(
        "{}\n{lanes} agents build this plan at the same time; the items above are yours. Leave every other frame \
         and placeholder alone.\n",
        build_prompt(&share, placeholders).trim_end()
    )
}

/// What a Side by side lane is told: the whole brief, and its own frame to build it in.
pub fn side_by_side_prompt(brief: &str, option: u32, lanes: usize, frame: &Placeholder) -> String {
    format!(
        "{brief}\n\n{lanes} agents answer this request side by side so the person can compare; yours is option \"{}\". \
         Build your whole answer inside one {}: create it first with that id, position and size (it may grow), claim \
         it, and change nothing outside it.",
        option_label(option),
        area(frame)
    )
}

/// What the first Explore run is told: build the result in its frame, which the variants then copy.
pub fn explore_first_prompt(brief: &str, variants: usize, frame: &Placeholder) -> String {
    format!(
        "{brief}\n\nBuild the result inside one {}: create it first with that id, position and size (it may grow) and \
         claim it. {} variants of it follow, one at a time.",
        area(frame),
        variants - 1
    )
}

/// What Explore variant `n` of `of` is told: copy the first result and change only `axis`.
pub fn explore_prompt(
    brief: &str,
    axis: Axis,
    n: usize,
    of: usize,
    first: &NodeId,
    frame: &Placeholder,
) -> String {
    let (what, keep) = match axis {
        Axis::Layout => (
            "only its layout: arrangement, order, spacing, sizes and alignment",
            "Keep its colours, typography, radii, strokes and effects, and every text and image.",
        ),
        Axis::Style => (
            "only its style: colours, typography, radii, strokes and effects",
            "Keep its structure, positions and sizes, and every text and image.",
        ),
    };
    format!(
        "Variant {n} of {of} for: {brief}\n\nCopy frame `{first}` with copy_nodes into a {}, then change {what}. \
         {keep} Make it clearly different from the earlier variants. When it is done, call report_progress with a \
         few words on what changed.",
        area(frame)
    )
}

/// What a helper started with `spawn_agents` is told.
pub fn helper_prompt(task: &str, by: &str) -> String {
    format!(
        "{by} started you as a helper for one task: {task}\n\nWork only on that. Claim a frame before you change it, \
         and call report_progress when the task is done."
    )
}

/// One lane as the log keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneRecord {
    pub agent: AgentId,
    /// What the agent is called (its hue, for a built-in agent).
    pub name: String,
    pub model: String,
    /// Split: the plan items it builds.
    pub items: Vec<u32>,
    /// A spawned helper's task.
    pub task: Option<String>,
}

/// One result the person chooses between (Side by side, Explore).
#[derive(Clone, Debug, PartialEq)]
pub struct OptionRecord {
    /// From 1: A, B, …
    pub option: u32,
    pub agent: AgentId,
    /// The frame that holds it.
    pub frame: NodeId,
    /// What changed, in the agent's words (Explore).
    pub label: Option<String>,
}

/// What the session log records about a turn's lanes. The event's origin carries the turn.
#[derive(Clone, Debug, PartialEq)]
pub enum ParallelEvent {
    /// Lanes started: by the person (Split, Side by side, Explore) or by an agent (`spawn_agents`).
    Lanes {
        mode: Mode,
        lanes: Vec<LaneRecord>,
    },
    LaneEnded {
        agent: AgentId,
        end: LaneEnd,
    },
    /// A result ready to compare.
    Option(OptionRecord),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaneState {
    pub record: LaneRecord,
    /// `None` while it works.
    pub end: Option<LaneEnd>,
}

/// A turn's lanes as the Agent panel shows them (C05, C07).
#[derive(Clone, Debug, PartialEq)]
pub struct ParallelState {
    /// How the turn's lanes started (spawned helpers join without changing it).
    pub mode: Mode,
    pub lanes: Vec<LaneState>,
    pub options: Vec<OptionRecord>,
}

/// `turn`'s lanes, replayed from the session log. `None` when it never had any.
pub fn parallel_state(events: &[SessionEvent], turn: &TurnId) -> Option<ParallelState> {
    let mut state: Option<ParallelState> = None;
    for event in events
        .iter()
        .filter(|event| event.origin.turn.as_ref() == Some(turn))
    {
        let EventBody::Parallel(event) = &event.body else {
            continue;
        };
        match event {
            ParallelEvent::Lanes { mode, lanes } => {
                let state = state.get_or_insert_with(|| ParallelState {
                    mode: *mode,
                    lanes: vec![],
                    options: vec![],
                });
                state.lanes.extend(lanes.iter().map(|record| LaneState {
                    record: record.clone(),
                    end: None,
                }));
            }
            ParallelEvent::LaneEnded { agent, end } => {
                if let Some(lane) = state
                    .as_mut()
                    .and_then(|s| s.lanes.iter_mut().find(|l| &l.record.agent == agent))
                {
                    lane.end = Some(*end);
                }
            }
            ParallelEvent::Option(option) => {
                if let Some(state) = state.as_mut() {
                    state.options.push(option.clone());
                }
            }
        }
    }
    state
}
