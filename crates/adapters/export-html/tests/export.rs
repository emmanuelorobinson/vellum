//! TEST DATA. Deterministic, offline code exports and hostile document content.
use vellum_core::{export::*, ports::*};
use vellum_export_html::Html;

struct Renderer;
impl ExportRenderer for Renderer {
    fn render(&self, _: &[ExportFrame], format: &str) -> Result<Vec<u8>, ExportError> {
        assert_eq!(format, "svg");
        Ok(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec())
    }
}
fn scene() -> ExportScene {
    ExportScene {
        frames: vec![ExportFrame::empty("frame00001", "<script> & \"", 120., 80.)],
    }
}
#[test]
fn both_formats_are_self_contained_deterministic_and_scaffold_is_optional() {
    for format in ["html-css", "html-tailwind"] {
        let html = Html::new(format).unwrap();
        let mut options = CodeOptions::default();
        let a = html.export(&scene(), &options, &Renderer).unwrap();
        assert_eq!(a, html.export(&scene(), &options, &Renderer).unwrap());
        let text = a.text().unwrap();
        assert!(text.starts_with("<!doctype html>"));
        assert!(!text.contains("<script>"));
        assert!(text.contains("&lt;script&gt;"));
        assert!(!text.contains("https://"));
        options.scaffold = false;
        assert!(
            !html
                .export(&scene(), &options, &Renderer)
                .unwrap()
                .text()
                .unwrap()
                .contains("<html")
        );
    }
}
#[test]
fn inline_and_file_assets_and_names_are_bounded_to_the_bundle() {
    let html = Html::new("html-css").unwrap();
    let mut s = scene();
    s.frames[0].fallback = true;
    let inline = html.export(&s, &CodeOptions::default(), &Renderer).unwrap();
    assert_eq!(inline.files.len(), 1);
    assert!(
        inline
            .text()
            .unwrap()
            .contains("data:image/svg+xml;base64,")
    );
    let files = html
        .export(
            &s,
            &CodeOptions {
                assets: AssetMode::Files,
                ..Default::default()
            },
            &Renderer,
        )
        .unwrap();
    assert_eq!(files.files.len(), 2);
    assert!(files.text().unwrap().contains("assets/frame-1.svg"));
    assert_eq!(
        files.files[1].bytes,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"
    );
    assert!(!files.warnings.is_empty(), "a visual fallback is explicit");
}

#[test]
fn code_export_contract() {
    for id in ["html-css", "html-tailwind"] {
        vellum_contracts::suites::export::code(&Html::new(id).unwrap());
    }
}
