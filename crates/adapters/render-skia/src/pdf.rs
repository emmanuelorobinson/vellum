//! Exact PDF page bounds over the same prepared Skia scenes. Only our own bounded PDF output is parsed.
use super::{Scene, sk};
use vellum_core::{export::ExportFrame, ports::ExportError};

pub(super) fn render(frames: &[ExportFrame], scenes: &[Scene]) -> Result<Vec<u8>, ExportError> {
    // Skia rounds PDF device sizes to integer units at 72 dpi. Keep that raster budget, offset the
    // canvas to the intended top edge, then set the physical MediaBox without raising raster DPI.
    let mut bytes = vec![];
    let mut doc = sk::pdf::new_document(&mut bytes, None);
    for (frame, scene) in frames.iter().zip(scenes) {
        let width = (frame.bounds.width * frame.scale) as f32;
        let height = (frame.bounds.height * frame.scale) as f32;
        let mut open = doc.begin_page((width.ceil(), height.ceil()), None);
        let canvas = open.canvas();
        canvas.translate((0., height.ceil() - height));
        canvas.clip_rect(sk::Rect::from_wh(width, height), None, false);
        scene.draw(canvas);
        doc = open.end_page();
    }
    doc.close();
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(ExportError::new("PDF exceeds the 32 MiB bundle budget"));
    }
    if frames.iter().all(|f| {
        (f.bounds.width * f.scale).fract() == 0. && (f.bounds.height * f.scale).fract() == 0.
    }) {
        return Ok(bytes);
    }
    let mut pdf = lopdf::Document::load_mem(&bytes).map_err(ExportError::new)?;
    let pages = pdf.get_pages();
    if pages.len() != frames.len() {
        return Err(ExportError::new("PDF page count mismatch"));
    }
    for (id, frame) in pages.values().zip(frames) {
        let page = pdf
            .get_object_mut(*id)
            .and_then(lopdf::Object::as_dict_mut)
            .map_err(ExportError::new)?;
        page.set(
            "MediaBox",
            vec![
                lopdf::Object::Integer(0),
                lopdf::Object::Integer(0),
                lopdf::Object::Real((frame.bounds.width * frame.scale) as f32),
                lopdf::Object::Real((frame.bounds.height * frame.scale) as f32),
            ],
        );
    }
    let mut output = vec![];
    pdf.save_to(&mut output).map_err(ExportError::new)?;
    if output.len() > 32 * 1024 * 1024 {
        return Err(ExportError::new("PDF exceeds the 32 MiB bundle budget"));
    }
    Ok(output)
}
