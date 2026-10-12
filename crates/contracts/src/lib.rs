//! Contract test suites for every Rust port, plus in-memory fakes (ARCHITECTURE §2.4).
//!
//! A suite is a generic function that takes an implementation of one port and panics, with a message that
//! names the rule, when the implementation breaks it. An adapter calls its suite from its own tests, so a new
//! adapter is not mergeable until it passes. Each port also has a fake here: in memory, deterministic, used in
//! unit tests and to build other modules before the real adapter exists. Every fake passes its own suite
//! (`tests/fakes_pass_their_suites.rs`), and every suite has a test that a deliberately broken
//! implementation fails it (`tests/suites_catch_broken_implementations.rs`).
//!
//! What a suite asks is only what every sound implementation does, so the suites for ports whose fake does
//! little (`PathOps`, `ScriptRuntime`, `ShaderCompiler`) are short. The prompts that build the real adapters
//! add the behavioural checks that need a real implementation.

pub mod fakes;
pub mod samples;
pub mod suites;

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
