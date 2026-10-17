//! TEST DATA. Broken implementations prove the export contracts reject dishonest adapters.
use vellum_contracts::suites::export;
use vellum_core::{export::*, ports::*};
struct BrokenCode;
impl CodeExporter for BrokenCode {
    fn id(&self) -> &'static str {
        "html-css"
    }
    fn export(
        &self,
        _: &ExportScene,
        _: &CodeOptions,
        _: &dyn ExportRenderer,
    ) -> Result<ExportBundle, ExportError> {
        Ok(ExportBundle {
            files: vec![ExportFile {
                name: "index.html".into(),
                mime: "text/html".into(),
                bytes: b"<!doctype html><html>".to_vec(),
            }],
            warnings: vec![],
        })
    }
}
struct BrokenImage;
impl ImageExporter for BrokenImage {
    fn id(&self) -> &'static str {
        "png"
    }
    fn export(
        &self,
        _: &ExportScene,
        _: &ImageOptions,
        _: &dyn ExportRenderer,
    ) -> Result<ExportBundle, ExportError> {
        Ok(ExportBundle {
            files: vec![ExportFile {
                name: "test.png".into(),
                mime: "image/png".into(),
                bytes: vec![1],
            }],
            warnings: vec![],
        })
    }
}
#[test]
fn contracts_catch_ignored_scaffold_and_scale() {
    assert!(std::panic::catch_unwind(|| export::code(&BrokenCode)).is_err());
    assert!(std::panic::catch_unwind(|| export::image(&BrokenImage)).is_err());
}
