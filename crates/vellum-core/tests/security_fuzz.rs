//! TEST DATA: fixed regression corpus for the exact oracle used by cargo-fuzz.
#[path = "common/op_application.rs"]
mod op_application;
#[test]
fn op_application_fuzz_smoke() {
    for seed in 0_u64..64 {
        op_application::check(&seed.to_le_bytes().repeat(8));
    }
}
