//! Invariant 1 (One Write Path), checked by the compiler: outside `vellum-core::ops` nothing can build the
//! `Commit` that `DocStore::apply` takes, so adapters, tools and scripts cannot reach it. Each case in
//! `tests/write-path/` must fail to compile with the message saved beside it.
//!
//! Invariant 11 the same way: outside `vellum-core::permissions` nothing can build or copy a `Permit`
//! (`tests/permit/`).

#[test]
fn doc_store_apply_is_reachable_only_from_ops() {
    trybuild::TestCases::new().compile_fail("tests/write-path/*.rs");
}

mod permissions {
    #[test]
    fn permits_come_only_from_the_broker() {
        trybuild::TestCases::new().compile_fail("tests/permit/*.rs");
    }
}
