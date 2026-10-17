//! TEST DATA. Selection order, scale, archive packaging, and bounds without a graphics context.
use std::cell::RefCell;
use vellum_core::{export::*, ports::*};
use vellum_export_raster::Images;
#[derive(Default)]
struct Renderer(RefCell<Vec<(String, Vec<String>)>>);
impl ExportRenderer for Renderer {
    fn render(&self, frames: &[ExportFrame], format: &str) -> Result<Vec<u8>, ExportError> {
        self.0
            .borrow_mut()
            .push((format.into(), frames.iter().map(|f| f.id.clone()).collect()));
        Ok(format.as_bytes().to_vec())
    }
}
fn scene() -> ExportScene {
    ExportScene {
        frames: vec![
            ExportFrame::empty("frame00002", "../same", 30., 20.),
            ExportFrame::empty("frame00001", "../same", 10., 15.),
        ],
    }
}
#[test]
fn pdf_is_one_file_in_selection_order_and_rasters_are_a_deterministic_zip() {
    let r = Renderer::default();
    let pdf = Images::new("pdf")
        .unwrap()
        .export(&scene(), &ImageOptions::default(), &r)
        .unwrap();
    assert_eq!(pdf.files.len(), 1);
    assert_eq!(pdf.files[0].mime, "application/pdf");
    assert_eq!(r.0.borrow()[0].1, ["frame00002", "frame00001"]);
    for format in ["png", "jpeg", "webp"] {
        let adapter = Images::new(format).unwrap();
        let a = adapter
            .export(&scene(), &ImageOptions::default(), &r)
            .unwrap();
        assert_eq!(
            a,
            adapter
                .export(&scene(), &ImageOptions::default(), &r)
                .unwrap()
        );
        assert_eq!(a.files[0].mime, "application/zip");
        assert_eq!(&a.files[0].bytes[..4], b"PK\x03\x04");
    }
}
#[test]
fn scale_is_one_to_three_and_invalid_selections_never_reach_the_renderer() {
    let r = Renderer::default();
    let adapter = Images::new("png").unwrap();
    for scale in [0, 4, 255] {
        assert!(
            adapter
                .export(
                    &scene(),
                    &ImageOptions {
                        scale,
                        ..Default::default()
                    },
                    &r
                )
                .is_err()
        );
    }
    assert!(
        adapter
            .export(
                &ExportScene { frames: vec![] },
                &ImageOptions::default(),
                &r
            )
            .is_err()
    );
    let mut s = scene();
    s.frames[0].bounds.width = 8193.;
    assert!(adapter.export(&s, &ImageOptions::default(), &r).is_err());
    assert!(r.0.borrow().is_empty());
    for scale in 1..=3 {
        assert!(
            adapter
                .export(
                    &scene(),
                    &ImageOptions {
                        scale,
                        ..Default::default()
                    },
                    &r
                )
                .is_ok()
        );
    }
}

#[test]
fn image_export_contract() {
    for id in ["png", "jpeg", "webp", "pdf", "svg"] {
        vellum_contracts::suites::export::image(&Images::new(id).unwrap());
    }
}
