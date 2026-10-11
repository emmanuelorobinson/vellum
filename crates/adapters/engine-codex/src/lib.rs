//! AgentEngine: the user's installed Codex.
//!
//! Shell only: no behaviour yet. Real code lands in the prompt that owns this crate.

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
