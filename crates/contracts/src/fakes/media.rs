//! TEST DATA. Fixed results; no downloads, inference or document mutation.
use vellum_core::ports::*;
pub struct MemoryImageOps;
impl ImageOps for MemoryImageOps {
    fn remove_background(&self, b: &[u8]) -> Result<ImageBytes, ImageWorkError> {
        if b.is_empty() {
            Err(ImageWorkError::InvalidInput)
        } else {
            Ok(ImageBytes {
                mime: "image/png".into(),
                bytes: b.to_vec(),
            })
        }
    }
    fn trace(&self, b: &[u8]) -> Result<String, ImageWorkError> {
        if b.is_empty() {
            Err(ImageWorkError::InvalidInput)
        } else {
            Ok("<svg width=\"1\" height=\"1\"><path d=\"M0 0H1V1H0Z\"/></svg>".into())
        }
    }
}
pub struct MemorySegmentation;
impl SegmentationModel for MemorySegmentation {
    fn mask(&self, b: &[u8], w: u32, h: u32) -> Result<Vec<f32>, ImageWorkError> {
        if w == 0 || h == 0 || b.len() != w as usize * h as usize * 4 {
            Err(ImageWorkError::InvalidInput)
        } else {
            Ok(vec![0.5; w as usize * h as usize])
        }
    }
}
pub struct MemoryGenerator(pub ImageBytes);
impl ImageGenerator for MemoryGenerator {
    fn provider(&self) -> &str {
        "memory"
    }
    fn origin(&self) -> &str {
        "https://example.com"
    }
    fn generate(&self, r: &GenerateImage) -> Result<ImageBytes, ImageWorkError> {
        if r.prompt.trim().is_empty() || r.model.trim().is_empty() {
            Err(ImageWorkError::InvalidInput)
        } else {
            Ok(self.0.clone())
        }
    }
}
pub struct MemoryFigma;
impl FigmaSource for MemoryFigma {
    fn import_file(
        &self,
        key: &str,
        allow: &dyn Fn(&str) -> Result<(), ImageWorkError>,
    ) -> Result<Imported, ImageWorkError> {
        if key.is_empty() {
            return Err(ImageWorkError::InvalidInput);
        }
        allow("https://api.figma.com")?;
        Ok(Imported {
            nodes: vec![
                serde_json::json!({"type":"rect","name":"TEST DATA","width":20,"height":30}),
            ],
            ..Default::default()
        })
    }
}

pub struct MemoryMediaHttp(pub FetchResponse);
impl MediaHttp for MemoryMediaHttp {
    fn exchange(&self, request: &MediaRequest) -> Result<FetchResponse, FetchError> {
        let rest = request
            .url
            .strip_prefix("https://")
            .ok_or(FetchError::Blocked)?;
        if rest
            .split('/')
            .next()
            .is_none_or(|h| h.is_empty() || h.contains('@'))
        {
            return Err(FetchError::Blocked);
        }
        if self.0.body.len() > request.max_bytes {
            return Err(FetchError::TooLarge);
        }
        Ok(self.0.clone())
    }
}
