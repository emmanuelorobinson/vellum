//! Document semantics: ops, guard, resolver, layout, scene, display list, diff/merge, export, and the ports (traits).
//!
//! So far: the ports (Prompt 10), the ops that are the one write path (Prompts 11-12), the Guard with its
//! leases (Prompt 13), the history that per-actor undo and "Undo this turn" run on (Prompt 14), diff and
//! three-way merge (Prompt 15), the resolver (variables, themes, components and libraries, Prompts 16–17), shared layout preparation (Prompt 18), scene queries (Prompt 20), layout inspection and display lists (Prompt 21), sessions, usage, estimated cost and the turn state machine (Prompt 34), and the permission broker (Prompt 35).
//! Real code for the rest lands in the prompt that owns it.
//!
//! This crate is pure (ARCHITECTURE §2.1): no file system, network, clock or randomness, and no dependency but
//! `vellum-types` and `serde_json`. `scripts/check-deps.sh` enforces that.

pub mod base64;
pub mod diff;
pub mod display_list;
pub mod doc_api;
pub mod export;
pub mod files;
pub mod guard;
pub mod history;
pub mod layout;
pub mod layout_check;
pub mod lease;
pub mod merge;
pub mod ops;
pub mod permissions;
pub mod ports;
pub mod resolver;
pub mod scene;
pub mod script_nodes;
pub mod sessions;
pub mod turns;

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
