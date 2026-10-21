//! Local pixel processing only: bounded decoding, ONNX segmentation, and vtracer tracing.
use image::{ImageFormat, ImageReader, RgbaImage, imageops::FilterType};
use std::{io::Cursor, path::Path};
use vellum_core::ports::*;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const MAX_PIXELS: u64 = 16 * 1024 * 1024;
fn decode(bytes: &[u8]) -> Result<RgbaImage, ImageWorkError> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(ImageWorkError::InvalidInput);
    }
    let mut r = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ImageWorkError::InvalidInput)?;
    if !matches!(
        r.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)
    ) {
        return Err(ImageWorkError::InvalidInput);
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    r.limits(limits);
    let dimensions = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ImageWorkError::InvalidInput)?
        .into_dimensions()
        .map_err(|_| ImageWorkError::InvalidInput)?;
    if u64::from(dimensions.0) * u64::from(dimensions.1) > MAX_PIXELS {
        return Err(ImageWorkError::InvalidInput);
    }
    r.decode()
        .map(|i| i.to_rgba8())
        .map_err(|_| ImageWorkError::InvalidInput)
}
pub fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, ImageWorkError> {
    let image = RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or(ImageWorkError::InvalidInput)?;
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|_| ImageWorkError::InvalidOutput)?;
    Ok(out.into_inner())
}
pub struct LocalImageOps {
    model: Option<Box<dyn SegmentationModel>>,
}
impl LocalImageOps {
    pub fn new(model: Option<Box<dyn SegmentationModel>>) -> Self {
        Self { model }
    }
}
impl ImageOps for LocalImageOps {
    fn remove_background(&self, bytes: &[u8]) -> Result<ImageBytes, ImageWorkError> {
        let model = self.model.as_ref().ok_or(ImageWorkError::NotConfigured)?;
        let mut image = decode(bytes)?;
        let mask = model.mask(image.as_raw(), image.width(), image.height())?;
        if mask.len() != image.width() as usize * image.height() as usize
            || mask
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(ImageWorkError::InvalidOutput);
        }
        for (pixel, alpha) in image.pixels_mut().zip(mask) {
            pixel[3] = (f32::from(pixel[3]) * alpha).round() as u8;
        }
        Ok(ImageBytes {
            mime: "image/png".into(),
            bytes: encode_png(image.width(), image.height(), image.as_raw())?,
        })
    }
    fn trace(&self, bytes: &[u8]) -> Result<String, ImageWorkError> {
        let image = decode(bytes)?;
        if image.width() as u64 * image.height() as u64 > 1024 * 1024 {
            return Err(ImageWorkError::InvalidInput);
        }
        let config = vtracer::Config {
            filter_speckle: 0,
            ..Default::default()
        };
        let input = vtracer::ColorImage {
            width: image.width() as usize,
            height: image.height() as usize,
            pixels: image.into_raw(),
        };
        let svg = vtracer::convert(input, config)
            .map_err(|_| ImageWorkError::InvalidOutput)?
            .to_string();
        if svg.len() > 16 * 1024 * 1024 {
            return Err(ImageWorkError::InvalidOutput);
        }
        Ok(svg)
    }
}
/// Explicit local U2Net-style model: [1,3,320,320] RGB standardized by ImageNet mean/std;
/// first output [1,1,320,320], normalized to a foreground probability mask. No model downloads; inference is self-contained CPU code.
pub struct OnnxSegmentation {
    session: std::sync::Arc<tract_onnx::prelude::TypedRunnableModel>,
}
impl OnnxSegmentation {
    pub fn open(model: &Path) -> Result<Self, ImageWorkError> {
        use tract_onnx::prelude::*;
        if !model.is_absolute() || !model.is_file() {
            return Err(ImageWorkError::NotConfigured);
        }
        if std::fs::metadata(model)
            .map_err(|_| ImageWorkError::Unavailable)?
            .len()
            > 256 * 1024 * 1024
        {
            return Err(ImageWorkError::InvalidInput);
        }
        let session = tract_onnx::onnx()
            .model_for_path(model)
            .and_then(|m| m.with_input_fact(0, f32::fact([1, 3, 320, 320]).into()))
            .and_then(|m| m.into_optimized())
            .and_then(|m| m.into_runnable())
            .map_err(|_| ImageWorkError::Unavailable)?;
        Ok(Self { session })
    }
}

impl SegmentationModel for OnnxSegmentation {
    fn mask(&self, rgba: &[u8], width: u32, height: u32) -> Result<Vec<f32>, ImageWorkError> {
        if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
            return Err(ImageWorkError::InvalidInput);
        }
        let image = RgbaImage::from_raw(width, height, rgba.to_vec())
            .ok_or(ImageWorkError::InvalidInput)?;
        let small = image::imageops::resize(&image, 320, 320, FilterType::Triangle);
        let mut values = vec![0f32; 3 * 320 * 320];
        for (i, p) in small.pixels().enumerate() {
            for c in 0..3 {
                values[c * 320 * 320 + i] =
                    (f32::from(p[c]) / 255. - [0.485, 0.456, 0.406][c]) / [0.229, 0.224, 0.225][c];
            }
        }
        use tract_onnx::prelude::*;
        let tensor = Tensor::from_shape(&[1, 3, 320, 320], &values)
            .map_err(|_| ImageWorkError::InvalidInput)?;
        let outputs = self
            .session
            .run(tvec![tensor.into()])
            .map_err(|_| ImageWorkError::InvalidOutput)?;
        let first = outputs.first().ok_or(ImageWorkError::InvalidOutput)?;
        let tensor = first
            .to_plain_array_view::<f32>()
            .map_err(|_| ImageWorkError::InvalidOutput)?;
        let raw = tensor.as_slice().ok_or(ImageWorkError::InvalidOutput)?;
        if tensor.shape() != [1, 1, 320, 320] || raw.iter().any(|v| !v.is_finite()) {
            return Err(ImageWorkError::InvalidOutput);
        }
        let lo = raw.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = raw.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let pixels: Vec<u8> = raw
            .iter()
            .map(|v| {
                let a = if hi - lo > 1e-6 {
                    (*v - lo) / (hi - lo)
                } else {
                    v.clamp(0., 1.)
                };
                (a * 255.).round() as u8
            })
            .collect();
        let gray =
            image::GrayImage::from_raw(320, 320, pixels).ok_or(ImageWorkError::InvalidOutput)?;
        Ok(
            image::imageops::resize(&gray, width, height, FilterType::Triangle)
                .pixels()
                .map(|p| f32::from(p[0]) / 255.)
                .collect(),
        )
    }
}
