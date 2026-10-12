//! The JSON codec passes the `Codec` contract suite (guide Prompt 10), on the fixture documents.

use std::path::PathBuf;

use vellum_codec_json::JsonCodec;
use vellum_types::Document;

fn fixture(name: &str) -> Document {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/documents")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    JsonCodec::new()
        .decode(&bytes)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn json_codec_passes_the_codec_contract() {
    let docs: Vec<Document> = [
        "minimal.vellum",
        "shapes.vellum",
        "content.vellum",
        "design-system.vellum",
        "library.lib.vellum",
    ]
    .into_iter()
    .map(fixture)
    .collect();
    vellum_contracts::suites::codec::run(&JsonCodec::new(), &docs);
}

#[test]
fn json_codec_passes_the_contract_on_the_shared_sample() {
    vellum_contracts::suites::codec::run(
        &JsonCodec::new(),
        &[vellum_contracts::samples::document()],
    );
}
