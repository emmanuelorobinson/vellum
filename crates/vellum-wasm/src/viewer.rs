//! The viewer module (renderer only, ADR-033): emscripten generates its loader beside this executable's WASM.
fn main() {
    let _ = vellum_wasm::vellum_format_version();
}
