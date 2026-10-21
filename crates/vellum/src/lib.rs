//! The `vellum` composition root as a library: what the binary runs, reachable from integration tests.
pub mod cli;
pub mod content;
pub mod daemon;
pub mod mcp;
pub mod orchestrate;
pub mod permissions;
pub mod sessions;
pub mod tools;

pub mod installed_engines;

mod export_scene;

pub mod asset_settings;
