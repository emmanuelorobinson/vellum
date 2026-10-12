//! One contract suite per port. Each is a function that takes an implementation and panics, naming the rule,
//! when the implementation breaks it. See the crate docs.

pub mod assets;
pub mod clock;
pub mod codec;
pub mod doc_store;
pub mod fonts;
pub mod layout;
pub mod path;
pub mod script;
pub mod shader;
pub mod spatial;
pub mod text;

/// Two numbers are the same, give or take rounding.
pub(crate) fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6
}
