//! Data types, display list, protocols and brand constants.
//!
//! This crate stays on a dependency allowlist: `serde`, `schemars` and `ts-rs` only. Everything the
//! file format, the daemon protocol and the TypeScript side share is defined here, once.
//!
//! Placeholder: the real node, paint, variable and component types arrive in Prompts 7 and 8.

pub mod brand;
mod document;

pub use document::{Document, DocumentMeta};

/// The `.vellum` file format version this build writes. Older versions stay readable forever;
/// a newer one opens read-only.
pub const FORMAT_VERSION: u32 = 1;
