//! Native counterpart of script-corpus.wasm. TEST DATA.
#[path = "doc-transcript/mod.rs"]
mod doc_transcript;
fn main() {
    let start = std::time::Instant::now();
    let rt =
        vellum_script_quickjs::QuickJsRuntime::new(move || start.elapsed().as_millis() as u64, 42);
    vellum_testkit::script::check_escapes(&rt);
    print!(
        "{}{}",
        vellum_testkit::script::transcript(&rt),
        doc_transcript::transcript(&rt)
    );
}
