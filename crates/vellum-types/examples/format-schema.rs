//! Prints the JSON Schema of the `.vellum` document to standard output (guide Prompt 8).
//!
//! `scripts/gen-format.mjs` runs this and writes `docs/format/vellum.schema.json`; CI fails when the
//! committed copy differs. Run it by hand with `cargo run -p vellum-types --example format-schema`.

fn main() {
    let schema = schemars::schema_for!(vellum_types::Document);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("a schema always serialises")
    );
}
