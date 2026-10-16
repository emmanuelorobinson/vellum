//! Step descriptors: what the Agent panel shows for each tool call (DESIGN.md "Step Timeline"; ARCHITECTURE §7.6).
//!
//! A step is data, not words. The verb is a code and the target an id and a name, so the panel writes the line
//! ("Designed Step 2 · 31 layers") in its own copy and language. A designed step asks for a thumbnail of the frame
//! it changed; drawing and caching it is the session store's business, not the tool's.

use serde::Serialize;
use serde_json::Value;
use vellum_core::ports::ChangeSet;
use vellum_types::Document;

use super::ToolError;
use super::support::Index;

/// A value as the session log keeps it: compact JSON, cut with a marker.
pub(super) fn preview(value: &Value) -> String {
    vellum_core::sessions::preview(value, vellum_core::sessions::PREVIEW_CHARS)
}

/// The most steps a session keeps before the oldest go. The session store (later) drains them.
pub const STEP_LIMIT: usize = 500;

/// What a step did, for the panel's verb and icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    /// Looked at the design.
    Read,
    /// Opened or created a document.
    Opened,
    /// Checked the layout for problems.
    CheckedLayout,
    /// Looked at a screenshot.
    CheckedScreenshot,
    /// Added layers.
    Designed,
    /// Changed layers' properties.
    Updated,
    /// Copied layers.
    Copied,
    /// Moved layers.
    Moved,
    /// Replaced layers.
    Replaced,
    /// Removed layers.
    Deleted,
    /// Added, changed or removed variables.
    UpdatedVariables,
    /// Made components or instances of them.
    MadeComponent,
    /// Made slots.
    MadeSlot,
    /// Claimed a frame.
    Claimed,
    /// Released a frame.
    Released,
    /// Ran an `execute` script.
    RanScript,
    /// Reported how the work is going.
    Progress,
}

/// Whether the call did what was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// It did.
    Done,
    /// It failed; nothing changed. `error` says why.
    Failed,
}

/// The node a step is about: the top-level frame it worked in, or the node it named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepTarget {
    /// Its id.
    pub id: String,
    /// Its name, for the row.
    pub name: String,
    /// Its kind.
    #[serde(rename = "type")]
    pub kind: String,
}

/// How much a step changed: the "31 layers" of a row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    /// Layers added.
    pub created: usize,
    /// Layers changed.
    pub changed: usize,
    /// Layers removed.
    pub removed: usize,
    /// Variables added, changed or removed.
    pub variables: usize,
}

impl Counts {
    /// The counts of a change set.
    pub fn of(changes: &ChangeSet) -> Self {
        Self {
            created: changes.inserted.len(),
            changed: changes.updated.len(),
            removed: changes.removed.len(),
            variables: changes.variables.len(),
        }
    }
}

/// A picture the panel should show of the frame as it is after the step.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Thumbnail {
    /// The node to draw.
    pub node: String,
    /// At what scale.
    pub scale: f64,
}

/// The scale thumbnails are asked for at: small enough for a row, sharp enough to recognise a screen.
pub const THUMBNAIL_SCALE: f64 = 0.25;

/// One tool call, as the Agent panel shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// The tool called.
    pub tool: String,
    /// What it did.
    pub verb: Verb,
    /// Whether it did.
    pub outcome: Outcome,
    /// The document it worked on, when one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
    /// The node it is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<StepTarget>,
    /// How much it changed.
    pub counts: Counts,
    /// The picture to show, for a step that designed something.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<Thumbnail>,
    /// The script of an `execute` step, shown in full when the row is expanded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// What a progress report said.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Done and total of a progress report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<(u32, u32)>,
    /// Why it failed, in the words the agent was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When it started, UTC milliseconds.
    pub started_at: i64,
    /// How long it took.
    pub duration_ms: i64,
    /// Whether the Guard or the file's state refused it (a failed step that changed nothing because it was not
    /// allowed to).
    #[serde(skip)]
    pub refused: bool,
    /// Its input as JSON, cut for the session log.
    #[serde(skip)]
    pub arguments: String,
    /// Its result or error as JSON, cut for the session log.
    #[serde(skip)]
    pub result: String,
}

/// What a tool says about its own step while it runs; the registry adds the rest.
#[derive(Debug, Clone, Default)]
pub struct StepNote {
    pub(super) document: Option<String>,
    pub(super) target: Option<StepTarget>,
    pub(super) counts: Counts,
    pub(super) script: Option<String>,
    pub(super) message: Option<String>,
    pub(super) progress: Option<(u32, u32)>,
}

impl StepNote {
    /// The node `id` in `doc` as a step target.
    pub(super) fn node(doc: &Document, id: &str) -> Option<StepTarget> {
        let index = Index::build(doc);
        let entry = index.get(id)?;
        Some(StepTarget {
            id: id.to_owned(),
            name: entry.node.name().to_owned(),
            kind: entry.node.kind().as_str().to_owned(),
        })
    }

    /// The top-level node most of `changes` happened in: a page's frame, or a component's root. Removed nodes are
    /// looked up in `before`.
    pub(super) fn frame_of(
        before: &Document,
        after: &Document,
        changes: &ChangeSet,
    ) -> Option<StepTarget> {
        let (now, then) = (Index::build(after), Index::build(before));
        let mut tally: Vec<(String, usize)> = Vec::new();
        let touched = changes
            .inserted
            .iter()
            .chain(&changes.updated)
            .map(|id| (id, &now))
            .chain(changes.removed.iter().map(|id| (id, &then)));
        for (id, index) in touched {
            let Some(at) = index.position(id.as_str()) else {
                continue;
            };
            let top = *index.ancestors_or_self(at).last().expect("itself");
            let top = index.entries[top].node.id().as_str().to_owned();
            match tally.iter_mut().find(|(id, _)| *id == top) {
                Some((_, count)) => *count += 1,
                None => tally.push((top, 1)),
            }
        }
        // The first one counted wins a tie, so the result does not depend on hash order.
        let best = tally.iter().map(|(_, n)| *n).max()?;
        let (id, _) = tally.into_iter().find(|(_, n)| *n == best)?;
        Self::node(after, &id).or_else(|| Self::node(before, &id))
    }
}

impl Step {
    /// The step for a call of `tool` that did `verb`, from what the tool noted and how it ended.
    pub(super) fn new(
        tool: &str,
        verb: Verb,
        note: StepNote,
        result: &Result<Value, ToolError>,
        started_at: i64,
        ended_at: i64,
    ) -> Self {
        let failed = result.as_ref().err();
        let thumbnail = match (&note.target, verb, failed) {
            (Some(target), Verb::Designed | Verb::RanScript, None)
                if note.counts.created + note.counts.changed > 0 =>
            {
                Some(Thumbnail {
                    node: target.id.clone(),
                    scale: THUMBNAIL_SCALE,
                })
            }
            _ => None,
        };
        Self {
            tool: tool.to_owned(),
            verb,
            outcome: if failed.is_some() {
                Outcome::Failed
            } else {
                Outcome::Done
            },
            document: note.document,
            target: note.target,
            counts: note.counts,
            thumbnail,
            script: note.script,
            message: note.message,
            progress: note.progress,
            error: failed.map(ToString::to_string),
            started_at,
            duration_ms: (ended_at - started_at).max(0),
            refused: failed.is_some_and(|e| e.kind() == super::ErrorKind::Refused),
            arguments: String::new(),
            result: preview(&match result {
                Ok(value) => value.clone(),
                Err(error) => error.to_json(),
            }),
        }
    }
}
