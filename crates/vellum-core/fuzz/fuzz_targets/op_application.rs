#![no_main]
#[path = "../../tests/common/op_application.rs"]
mod oracle;
libfuzzer_sys::fuzz_target!(|data: &[u8]| oracle::check(data));
