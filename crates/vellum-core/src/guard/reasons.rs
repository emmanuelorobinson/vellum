//! The Guard's reasons, in plain language. Each is the string `@vellum/config` exports under `guardReasons`, with
//! the same key; a test reads that file to keep the two the same. `{agent}` is a name ("Teal", "Claude Code"),
//! `{count}` and `{limit}` are numbers.
//!
//! Only [`LEASED`] is in guide Appendix B ("Write rejected"). The others reuse DESIGN.md's verbs ("Pause all →
//! Agents paused", "Stop → Stopped by you"); the rest of their wording is ours, recorded in `reports.md`.

/// Another agent holds the frame. Appendix B, "Write rejected".
pub const LEASED: &str = "Frame leased by {agent}";
/// Pause all is on.
pub const AGENTS_PAUSED: &str = "Agents paused";
/// This agent alone is paused.
pub const AGENT_PAUSED: &str = "{agent} is paused";
/// A person stopped this agent.
pub const STOPPED: &str = "Stopped by you";
/// A person stopped this agent on this frame from its lease tab.
pub const STOPPED_HERE: &str = "Stopped by you on this frame";
/// Too many ops in one transaction.
pub const TOO_MANY_CHANGES: &str = "{count} changes at once · the limit is {limit}";
/// Too many nodes inserted in one transaction.
pub const TOO_MANY_LAYERS: &str = "{count} new layers at once · the limit is {limit}";

/// Every reason by its `@vellum/config` key.
pub const ALL: &[(&str, &str)] = &[
    ("leased", LEASED),
    ("agentsPaused", AGENTS_PAUSED),
    ("agentPaused", AGENT_PAUSED),
    ("stopped", STOPPED),
    ("stoppedHere", STOPPED_HERE),
    ("tooManyChanges", TOO_MANY_CHANGES),
    ("tooManyLayers", TOO_MANY_LAYERS),
];
