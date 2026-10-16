//! cargo +nightly fuzz run script_header (cargo-fuzz required).
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|source: &str| {
    let _ = vellum_core::script_nodes::Header::parse(source);
});
