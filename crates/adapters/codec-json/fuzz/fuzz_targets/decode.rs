//! Fuzz target for the decoder (guide Prompt 9). Run from `crates/adapters/codec-json` with
//! `cargo +nightly fuzz run decode` (needs `cargo install cargo-fuzz`). Seed it with `fixtures/documents/`.
//!
//! Property: the decoder never panics; what it reads can be written, and writing it is stable.
#![no_main]

use libfuzzer_sys::fuzz_target;
use vellum_codec_json::{decode, encode};

fuzz_target!(|data: &[u8]| {
    let Ok(doc) = decode(data) else { return };
    let saved = encode(&doc).expect("a document that was read can be written");
    let again = decode(&saved).expect("what was written reads");
    assert_eq!(encode(&again).expect("encode"), saved, "saving is not stable");
});
