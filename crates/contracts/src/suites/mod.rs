//! One contract suite per port. Each is a function that takes an implementation and panics, naming the rule,
//! when the implementation breaks it. See the crate docs.

pub mod assets;
pub mod clock;
pub mod codec;
pub mod content;
pub mod doc_store;
pub mod document_files;
pub mod engine;
pub mod files;
pub mod fonts;
pub mod import;
pub mod layout;
pub mod model;
pub mod path;
pub mod permissions;
pub mod script;
pub mod sessions;
pub mod shader;
pub mod spatial;
pub mod spec;
pub mod target;
pub mod text;
pub mod tokens;
pub mod watch;

/// Two numbers are the same, give or take rounding.
pub(crate) fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6
}
pub mod export;
pub mod sync;

pub mod browser;
