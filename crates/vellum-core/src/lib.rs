//! Document semantics: ops, guard, resolver, layout, scene, display list, diff/merge, export, and the ports (traits).
//!
//! So far: the ports (Prompt 10), the ops that are the one write path (Prompts 11-12), and the Guard with its
//! leases (Prompt 13). Real code for the rest lands in the prompt that owns it.
//!
//! This crate is pure (ARCHITECTURE §2.1): no file system, network, clock or randomness, and no dependency but
//! `vellum-types`. `scripts/check-deps.sh` enforces that.

pub mod guard;
pub mod lease;
pub mod ops;
pub mod ports;

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
