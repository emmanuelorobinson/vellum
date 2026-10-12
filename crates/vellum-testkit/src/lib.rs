//! Stub model endpoint, fake engines, fixture documents, Chrome layout oracle runner, clocks.
//!
//! So far: the pixel-diff helper for golden images (Prompt 4). The rest lands in the prompts that own it.

pub mod pixel_diff;

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
