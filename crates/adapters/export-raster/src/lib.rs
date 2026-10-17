//! Image export packaging over the full renderer supplied by the host. No disk or graphics context here.
use std::io::{Cursor, Write};
use vellum_core::{export::*, ports::*};
use zip::{ZipWriter, write::SimpleFileOptions};
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub struct Images {
    format: &'static str,
    mime: &'static str,
    extension: &'static str,
}
impl Images {
    pub fn new(format: &str) -> Option<Self> {
        let (format, mime, extension) = match format {
            "png" => ("png", "image/png", "png"),
            "jpeg" => ("jpeg", "image/jpeg", "jpg"),
            "webp" => ("webp", "image/webp", "webp"),
            "svg" => ("svg", "image/svg+xml", "svg"),
            "pdf" => ("pdf", "application/pdf", "pdf"),
            _ => return None,
        };
        Some(Self {
            format,
            mime,
            extension,
        })
    }
}
impl ImageExporter for Images {
    fn id(&self) -> &'static str {
        self.format
    }
    fn export(
        &self,
        scene: &ExportScene,
        options: &ImageOptions,
        renderer: &dyn ExportRenderer,
    ) -> Result<ExportBundle, ExportError> {
        if !(1..=3).contains(&options.scale) {
            return Err(ExportError::new("Scale is 1, 2 or 3"));
        }
        let scale = if matches!(self.format, "pdf" | "svg") {
            1.
        } else {
            f64::from(options.scale)
        };
        scene.check(scale)?;
        let frames: Vec<_> = scene.frames.iter().map(|f| f.at_scale(scale)).collect();
        let file = |name: String, bytes: Vec<u8>| ExportFile {
            name,
            mime: self.mime.into(),
            bytes,
        };
        let mut files = vec![];
        if self.format == "pdf" {
            files.push(file(
                format!(
                    "export{}.pdf",
                    file_stem("", 0, &options.suffix).trim_start_matches("frame-1")
                ),
                renderer.render(&frames, "pdf")?,
            ));
        } else {
            for (i, frame) in frames.iter().enumerate() {
                files.push(file(
                    format!(
                        "{}.{}",
                        file_stem(&frame.name, i, &options.suffix),
                        self.extension
                    ),
                    renderer.render(std::slice::from_ref(frame), self.format)?,
                ));
            }
        }
        if files.iter().map(|f| f.bytes.len()).sum::<usize>() > 32 * 1024 * 1024 {
            return Err(ExportError::new("Export exceeds the 32 MiB bundle budget"));
        }
        if files.len() > 1 && matches!(self.format, "png" | "jpeg" | "webp") {
            let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
            for f in files {
                archive
                    .start_file(
                        f.name,
                        SimpleFileOptions::default()
                            .last_modified_time(zip::DateTime::default())
                            .compression_method(zip::CompressionMethod::Stored)
                            .unix_permissions(0o644),
                    )
                    .map_err(ExportError::new)?;
                archive.write_all(&f.bytes).map_err(ExportError::new)?;
            }
            files = vec![ExportFile {
                name: "export.zip".into(),
                mime: "application/zip".into(),
                bytes: archive.finish().map_err(ExportError::new)?.into_inner(),
            }];
        }
        Ok(ExportBundle {
            files,
            warnings: vec![],
        })
    }
}
