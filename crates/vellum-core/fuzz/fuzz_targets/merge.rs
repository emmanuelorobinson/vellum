//! Fuzz target for diff and merge (guide Prompt 15). Run from `crates/vellum-core` with
//! `cargo +nightly fuzz run merge` (needs `cargo install cargo-fuzz`).
//!
//! The input picks a fixture document and two random branches of it, made through the op layer by the same
//! code the property tests use (`tests/common/mutate.rs`). Properties: merge(b, x, b) == x, merge(b, b, y) == y,
//! the merge is a valid document, a clean merge does not depend on which side is ours, and every diff between
//! the documents, planned, leads where it should. `tests/diff_merge.rs`'s `fuzz_smoke` runs the same checks on a
//! fixed corpus on every `cargo test`.
#![no_main]

#[path = "../../tests/common/mutate.rs"]
mod mutate;

use std::path::Path;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use vellum_types::Document;

fn bases() -> &'static [Document] {
    static BASES: OnceLock<Vec<Document>> = OnceLock::new();
    BASES.get_or_init(|| {
        mutate::bases(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/documents"))
    })
}

fuzz_target!(|data: &[u8]| {
    let (base, ours, theirs) = mutate::case(bases(), data);
    mutate::check_diff(&base, &ours);
    mutate::check_diff(&ours, &theirs);
    mutate::check_merge(&base, &ours, &theirs);
});
