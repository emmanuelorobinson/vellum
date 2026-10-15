//! WASM-lane harness only. No product exports or document mutations. TEST DATA.
#![allow(unsafe_code)]
use std::cell::RefCell;
use vellum_script_quickjs::QuickJsRuntime;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn vellum_script_now_ms() -> f64;
}
thread_local! { static RESULT: RefCell<String> = const { RefCell::new(String::new()) }; }
#[unsafe(no_mangle)]
pub extern "C" fn script_check() {
    #[cfg(target_arch = "wasm32")]
    let clock = || unsafe { vellum_script_now_ms() as u64 };
    #[cfg(not(target_arch = "wasm32"))]
    let clock = {
        let start = std::time::Instant::now();
        move || start.elapsed().as_millis() as u64
    };
    let rt = QuickJsRuntime::new(clock, 42);
    vellum_contracts::suites::script::run(&rt);
    vellum_testkit::script::check_escapes(&rt);
    RESULT.with_borrow_mut(|out| *out = vellum_testkit::script::transcript(&rt));
}
#[unsafe(no_mangle)]
pub extern "C" fn script_result_ptr() -> *const u8 {
    RESULT.with_borrow(|out| out.as_ptr())
}
#[unsafe(no_mangle)]
pub extern "C" fn script_result_len() -> usize {
    RESULT.with_borrow(|out| out.len())
}
