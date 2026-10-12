//! Pixel comparison for golden images (guide §11 rule 18: golden images only where pixels are the
//! behaviour). A comparison yields numbers (differing pixels, max and p95 delta) and a diff image;
//! [`assert_images_match`] writes expected, actual and diff PNGs as artifacts when it fails.

use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

/// Straight-alpha RGBA8 pixels, row-major, top row first.
#[derive(Clone, PartialEq, Eq)]
pub struct Rgba8Image {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl fmt::Debug for Rgba8Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rgba8Image({}x{})", self.width, self.height)
    }
}

impl Rgba8Image {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, PixelDiffError> {
        let expected = width as usize * height as usize * 4;
        if rgba.len() != expected {
            return Err(PixelDiffError::BadLength {
                expected,
                actual: rgba.len(),
            });
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.rgba
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        assert!(
            x < self.width && y < self.height,
            "pixel ({x}, {y}) outside {}x{}",
            self.width,
            self.height
        );
        (y as usize * self.width as usize + x as usize) * 4
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let o = self.offset(x, y);
        [
            self.rgba[o],
            self.rgba[o + 1],
            self.rgba[o + 2],
            self.rgba[o + 3],
        ]
    }

    pub fn set(&mut self, x: u32, y: u32, px: [u8; 4]) {
        let o = self.offset(x, y);
        self.rgba[o..o + 4].copy_from_slice(&px);
    }

    /// Reads an 8-bit PNG of any colour type and expands it to RGBA8.
    pub fn read_png(path: &Path) -> Result<Self, PixelDiffError> {
        let file = File::open(path).map_err(|e| PixelDiffError::io(path, e))?;
        let mut decoder = png::Decoder::new(BufReader::new(file));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder
            .read_info()
            .map_err(|e| PixelDiffError::png(path, e))?;
        let size = reader
            .output_buffer_size()
            .ok_or_else(|| PixelDiffError::png(path, "image too large"))?;
        let mut buf = vec![0; size];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|e| PixelDiffError::png(path, e))?;
        buf.truncate(info.buffer_size());
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf,
            png::ColorType::Rgb => buf
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&[r, g, b]| [r, g, b, 255])
                .collect(),
            png::ColorType::GrayscaleAlpha => buf
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[g, a]| [g, g, g, a])
                .collect(),
            png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::Indexed => {
                return Err(PixelDiffError::png(path, "indexed PNG was not expanded"));
            }
        };
        Self::new(info.width, info.height, rgba)
    }

    pub fn write_png(&self, path: &Path) -> Result<(), PixelDiffError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| PixelDiffError::io(parent, e))?;
        }
        let file = File::create(path).map_err(|e| PixelDiffError::io(path, e))?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| PixelDiffError::png(path, e))?;
        writer
            .write_image_data(&self.rgba)
            .map_err(|e| PixelDiffError::png(path, e))?;
        writer.finish().map_err(|e| PixelDiffError::png(path, e))
    }
}

/// How different two images may be and still match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// A pixel differs when any channel (alpha included) differs by more than this.
    pub channel: u8,
    /// The share of differing pixels (0..=1) that still passes.
    pub max_differing_fraction: f64,
}

impl Tolerance {
    pub const EXACT: Tolerance = Tolerance {
        channel: 0,
        max_differing_fraction: 0.0,
    };
    /// The colour of a differing pixel in the diff image.
    pub const DIFF_MARK: [u8; 4] = [255, 0, 255, 255];
}

/// Numbers from one comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub width: u32,
    pub height: u32,
    pub total_pixels: u64,
    /// Pixels whose largest channel delta is above the channel tolerance.
    pub differing_pixels: u64,
    /// The largest channel delta anywhere.
    pub max_channel_delta: u8,
    /// The 95th percentile of each pixel's largest channel delta.
    pub p95_pixel_delta: u8,
    /// The mean of each pixel's largest channel delta.
    pub mean_pixel_delta: f64,
    pub passed: bool,
}

impl Report {
    pub fn differing_fraction(&self) -> f64 {
        if self.total_pixels == 0 {
            0.0
        } else {
            self.differing_pixels as f64 / self.total_pixels as f64
        }
    }
}

#[derive(Debug, Clone)]
pub struct Comparison {
    pub report: Report,
    /// Differing pixels in [`Tolerance::DIFF_MARK`]; the rest is the expected image, greyed and faded.
    pub diff: Rgba8Image,
}

pub fn compare(
    expected: &Rgba8Image,
    actual: &Rgba8Image,
    tolerance: &Tolerance,
) -> Result<Comparison, PixelDiffError> {
    if (expected.width, expected.height) != (actual.width, actual.height) {
        return Err(PixelDiffError::SizeMismatch {
            expected: (expected.width, expected.height),
            actual: (actual.width, actual.height),
        });
    }
    let total = expected.width as usize * expected.height as usize;
    let mut histogram = [0u64; 256];
    let mut differing = 0u64;
    let mut sum = 0u64;
    let mut diff = Vec::with_capacity(total * 4);
    for (e, a) in expected
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(actual.rgba.as_chunks::<4>().0)
    {
        let delta = (0..4).map(|c| e[c].abs_diff(a[c])).max().unwrap_or(0);
        histogram[delta as usize] += 1;
        sum += u64::from(delta);
        if delta > tolerance.channel {
            differing += 1;
            diff.extend_from_slice(&Tolerance::DIFF_MARK);
        } else {
            let grey = ((u32::from(e[0]) + u32::from(e[1]) + u32::from(e[2])) / 3) as u8;
            let faded = 192 + grey / 4;
            diff.extend_from_slice(&[faded, faded, faded, 255]);
        }
    }
    let max_channel_delta = histogram.iter().rposition(|&n| n > 0).unwrap_or(0) as u8;
    let p95_pixel_delta = percentile(&histogram, total as u64, 0.95);
    let report = Report {
        width: expected.width,
        height: expected.height,
        total_pixels: total as u64,
        differing_pixels: differing,
        max_channel_delta,
        p95_pixel_delta,
        mean_pixel_delta: if total == 0 {
            0.0
        } else {
            sum as f64 / total as f64
        },
        passed: total == 0 || differing as f64 / total as f64 <= tolerance.max_differing_fraction,
    };
    let diff = Rgba8Image::new(expected.width, expected.height, diff)?;
    Ok(Comparison { report, diff })
}

/// The smallest delta d such that at least `q` of all pixels have a delta of d or less.
fn percentile(histogram: &[u64; 256], total: u64, q: f64) -> u8 {
    let needed = (q * total as f64).ceil() as u64;
    let mut seen = 0;
    for (delta, &count) in histogram.iter().enumerate() {
        seen += count;
        if seen >= needed {
            return delta as u8;
        }
    }
    255
}

/// Compares and, on failure, writes `<name>.expected.png`, `<name>.actual.png` and `<name>.diff.png`
/// into `artifact_dir`. A passing comparison writes nothing.
pub fn assert_images_match(
    name: &str,
    expected: &Rgba8Image,
    actual: &Rgba8Image,
    tolerance: &Tolerance,
    artifact_dir: &Path,
) -> Result<Report, PixelDiffError> {
    let Comparison { report, diff } = compare(expected, actual, tolerance)?;
    if report.passed {
        return Ok(report);
    }
    expected.write_png(&artifact_dir.join(format!("{name}.expected.png")))?;
    actual.write_png(&artifact_dir.join(format!("{name}.actual.png")))?;
    let diff_path = artifact_dir.join(format!("{name}.diff.png"));
    diff.write_png(&diff_path)?;
    Err(PixelDiffError::Mismatch {
        report: Box::new(report),
        diff: diff_path,
    })
}

#[derive(Debug)]
pub enum PixelDiffError {
    BadLength {
        expected: usize,
        actual: usize,
    },
    SizeMismatch {
        expected: (u32, u32),
        actual: (u32, u32),
    },
    Mismatch {
        report: Box<Report>,
        diff: PathBuf,
    },
    Io {
        path: PathBuf,
        message: String,
    },
    Png {
        path: PathBuf,
        message: String,
    },
}

impl PixelDiffError {
    fn io(path: &Path, e: std::io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            message: e.to_string(),
        }
    }
    fn png(path: &Path, e: impl fmt::Display) -> Self {
        Self::Png {
            path: path.to_owned(),
            message: e.to_string(),
        }
    }
}

impl fmt::Display for PixelDiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadLength { expected, actual } => {
                write!(f, "pixel buffer is {actual} bytes, expected {expected}")
            }
            Self::SizeMismatch { expected, actual } => {
                write!(
                    f,
                    "image is {}x{}, expected {}x{}",
                    actual.0, actual.1, expected.0, expected.1
                )
            }
            Self::Mismatch { report, diff } => write!(
                f,
                "{} of {} pixels differ (max delta {}, p95 {}); diff image: {}",
                report.differing_pixels,
                report.total_pixels,
                report.max_channel_delta,
                report.p95_pixel_delta,
                diff.display()
            ),
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Png { path, message } => write!(f, "{}: PNG: {message}", path.display()),
        }
    }
}

impl std::error::Error for PixelDiffError {}
