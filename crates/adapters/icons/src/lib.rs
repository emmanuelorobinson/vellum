//! IconLibrary: searchable icon sets (ARCHITECTURE §3.6).
//!
//! Six sets ship inside the binary as generated data (`data/*.json`, made by `scripts/icons-gen.mjs` from the
//! open-source packages named in each file and in THIRD_PARTY_NOTICES.md): Lucide, Feather, Phosphor and Material
//! Symbols in three styles. Each parses on first use. [`Library::from_records`] makes a set from any list of
//! icons, which is how a project's own SVG folder joins them (the composition root reads the files).
//!
//! Icons are SVG path data in the set's own view box. Stroke sets (Lucide, Feather) take their line width from
//! the weight: 100 is 0.5 units and 700 is 3.5, with 400, the sets' own 2, in the middle.
mod catalog;
mod library;
mod search;

pub use catalog::Catalog;
pub use library::{Art, Library, Record, SetInfo};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
