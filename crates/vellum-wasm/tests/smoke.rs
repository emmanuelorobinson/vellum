#[test]
fn placeholder_export_reports_the_format_version() {
    assert_eq!(
        vellum_wasm::vellum_format_version(),
        vellum_types::FORMAT_VERSION
    );
}
