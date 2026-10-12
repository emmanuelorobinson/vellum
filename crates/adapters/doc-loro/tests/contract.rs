//! The Loro store passes the `DocStore` contract suite and Invariant 3's property test (guide Prompt 14).

use vellum_contracts::suites::doc_store;
use vellum_doc_loro::LoroDocStore;

#[test]
fn loro_store_passes_the_doc_store_contract() {
    doc_store::run(LoroDocStore::new);
}

#[test]
fn undo_is_yours_with_three_actors_interleaved() {
    doc_store::undo_is_yours(LoroDocStore::new, 0..200);
}

#[test]
fn crate_identifies_itself() {
    assert_eq!(vellum_doc_loro::CRATE_NAME, "vellum-doc-loro");
}
