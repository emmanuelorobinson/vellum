//! Export contracts: offline data, deterministic output, valid selections, scale limits and clipboard MIME.
use vellum_core::{export::*, ports::*};
#[derive(Default)]
pub struct Render;
impl ExportRenderer for Render {
    fn render(&self, frames: &[ExportFrame], format: &str) -> Result<Vec<u8>, ExportError> {
        Ok(format!(
            "{format}:{}",
            frames
                .iter()
                .map(|f| f.id.clone())
                .collect::<Vec<_>>()
                .join(",")
        )
        .into_bytes())
    }
}
pub fn scene() -> ExportScene {
    ExportScene {
        frames: vec![ExportFrame::empty("frame00001", "TEST DATA", 12., 8.)],
    }
}
pub fn code(format: &dyn CodeExporter) {
    assert!(!format.id().is_empty());
    let first = format
        .export(&scene(), &CodeOptions::default(), &Render)
        .unwrap();
    assert_eq!(
        first,
        format
            .export(&scene(), &CodeOptions::default(), &Render)
            .unwrap()
    );
    assert!(first.text().unwrap().contains("<!doctype html>"));
    assert_eq!(
        first.clipboard().unwrap()["text/html"],
        first.files[0].bytes
    );
    let fragment = format
        .export(
            &scene(),
            &CodeOptions {
                scaffold: false,
                ..Default::default()
            },
            &Render,
        )
        .unwrap();
    assert!(!fragment.text().unwrap().contains("<html"));
    assert!(
        format
            .export(
                &ExportScene { frames: vec![] },
                &CodeOptions::default(),
                &Render
            )
            .is_err()
    );
}
pub fn image(format: &dyn ImageExporter) {
    assert!(!format.id().is_empty());
    let first = format
        .export(&scene(), &ImageOptions::default(), &Render)
        .unwrap();
    assert_eq!(
        first,
        format
            .export(&scene(), &ImageOptions::default(), &Render)
            .unwrap()
    );
    assert_eq!(first.files.len(), 1);
    assert!(!first.files[0].bytes.is_empty());
    assert!(!first.files[0].name.contains('/'));
    for scale in [0, 4, 255] {
        assert!(
            format
                .export(
                    &scene(),
                    &ImageOptions {
                        scale,
                        ..Default::default()
                    },
                    &Render
                )
                .is_err()
        );
    }
    assert!(
        format
            .export(
                &ExportScene { frames: vec![] },
                &ImageOptions::default(),
                &Render
            )
            .is_err()
    );
    for scale in 1..=3 {
        assert!(
            format
                .export(
                    &scene(),
                    &ImageOptions {
                        scale,
                        ..Default::default()
                    },
                    &Render
                )
                .is_ok()
        );
    }
}
