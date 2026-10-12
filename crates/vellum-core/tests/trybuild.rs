//! Invariant 1 (One Write Path), checked by the compiler: outside `vellum-core::ops` nothing can build the
//! `Commit` that `DocStore::apply` takes, so adapters, tools and scripts cannot reach it. Each case in
//! `tests/write-path/` must fail to compile with the message saved beside it.

#[test]
fn doc_store_apply_is_reachable_only_from_ops() {
    trybuild::TestCases::new().compile_fail("tests/write-path/*.rs");
}
