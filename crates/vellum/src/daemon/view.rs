//! What an agent can learn about a document besides its content: who holds it, what is selected, where the
//! canvas is looking, which frames are leased and which agents are paused (`get_app_state`, ARCHITECTURE §7.3).
//!
//! The Guard runs where the document lives, so a live editor reports these itself; for a headless file the
//! daemon's own Guard state answers.

use serde::Serialize;
use vellum_core::ports::TargetMode;
use vellum_types::{AgentId, NodeId, PageId};

use super::live::EditorId;

/// The part of the canvas an editor shows, in canvas coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Viewport {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Visible width.
    pub width: f64,
    /// Visible height.
    pub height: f64,
    /// Zoom, 1.0 is 100%.
    pub zoom: f64,
}

/// A frame an agent holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaseInfo {
    /// The leased frame.
    pub frame: NodeId,
    /// The agent that holds it.
    pub agent: AgentId,
    /// When the lease lapses without activity, UTC milliseconds.
    pub expires_at: i64,
}

/// Which agents a person paused or stopped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paused {
    /// Pause all is on.
    pub all: bool,
    /// Agents paused one by one.
    pub agents: Vec<AgentId>,
    /// Agents stopped.
    pub stopped: Vec<AgentId>,
}

/// What an editor (or the headless Guard) knows about a document's session state.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorView {
    /// The selected nodes. Always empty with no editor.
    pub selection: Vec<NodeId>,
    /// The page on screen, when there is a screen.
    pub page: Option<PageId>,
    /// What the canvas shows, when there is a screen.
    pub viewport: Option<Viewport>,
    /// The live leases.
    pub leases: Vec<LeaseInfo>,
    /// Paused and stopped agents.
    pub paused: Paused,
}

/// One open document as the daemon sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentView {
    /// Live (an editor holds it) or headless.
    pub mode: TargetMode,
    /// The editor that writes it, when live.
    pub writer: Option<EditorId>,
    /// Unsaved changes.
    pub dirty: bool,
    /// Selection, viewport, leases and pauses.
    pub state: EditorView,
}
