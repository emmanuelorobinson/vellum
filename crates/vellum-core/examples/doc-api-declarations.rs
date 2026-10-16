//! Prints the versioned registry's declaration file; scripts/doc-api-gen.mjs owns file I/O.
fn main() {
    print!("{}", vellum_core::doc_api::declarations());
}
