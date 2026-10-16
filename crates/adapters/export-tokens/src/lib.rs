//! `TokenFormat` implementations (ARCHITECTURE §3.5): CSS variables, Tailwind v4 `@theme` and W3C Design Tokens
//! (DTCG) JSON. Each only converts between text and a [`vellum_core::tokens::TokenSet`]; the document, the
//! diff and the file are the caller's. No vendor crate: the formats are small enough to write by hand, and the
//! stylesheet reader is the one part that must be forgiving about what real files hold.

pub mod tokens;

pub use tokens::{CssVars, DtcgJson, TailwindV4, format_by_id, formats};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
