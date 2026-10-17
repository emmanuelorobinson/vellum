//! Importer: PNG, JPEG and WEBP as image nodes (ARCHITECTURE §3.6).
//!
//! Reads the file's size from its header and keeps the original bytes untouched as the asset; it does not decode
//! pixels, so a file with a good header and bad data imports and then fails to draw. Pictures larger than
//! [`MAX_SIDE`] are laid out smaller on the canvas (the asset keeps every pixel) and a note says so.
use serde_json::json;
use vellum_core::ports::{ImportError, ImportInput, Imported, ImportedAsset, Importer, asset_ref};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The longest side a picture is laid out at. Bigger ones keep their proportions.
pub const MAX_SIDE: f64 = 1600.0;
/// The largest side the header may claim; anything bigger is not a picture a design tool opens.
const LIMIT: u32 = 16_384;

pub struct ImageImporter;

#[derive(Clone, Copy)]
struct Header {
    mime: &'static str,
    width: u32,
    height: u32,
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn le16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(
        b.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn le24(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 3)?;
    Some(u32::from(s[0]) | u32::from(s[1]) << 8 | u32::from(s[2]) << 16)
}

fn png(b: &[u8]) -> Option<Header> {
    if !b.starts_with(b"\x89PNG\r\n\x1a\n") || b.get(12..16)? != b"IHDR" {
        return None;
    }
    Some(Header {
        mime: "image/png",
        width: be32(b, 16)?,
        height: be32(b, 20)?,
    })
}

fn jpeg(b: &[u8]) -> Option<Header> {
    if !b.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut at = 2;
    loop {
        while *b.get(at)? != 0xff {
            at += 1;
        }
        while *b.get(at)? == 0xff {
            at += 1;
        }
        let marker = *b.get(at)?;
        at += 1;
        match marker {
            // Markers without a length: stuffed zero, restarts, start and end of image, temporary.
            0x00 | 0x01 | 0xd0..=0xd8 => {}
            0xd9 | 0xda => return None,
            // Start of frame (not the DHT, JPG and DAC markers that sit among them).
            0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc) => {
                let height = u32::from(u16::from_be_bytes(b.get(at + 3..at + 5)?.try_into().ok()?));
                let width = u32::from(u16::from_be_bytes(b.get(at + 5..at + 7)?.try_into().ok()?));
                return Some(Header {
                    mime: "image/jpeg",
                    width,
                    height,
                });
            }
            _ => {
                let length = usize::from(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?));
                if length < 2 {
                    return None;
                }
                at += length;
            }
        }
    }
}

fn webp(b: &[u8]) -> Option<Header> {
    if b.get(..4)? != b"RIFF" || b.get(8..12)? != b"WEBP" {
        return None;
    }
    let (width, height) = match b.get(12..16)? {
        b"VP8 " => {
            if b.get(23..26)? != [0x9d, 0x01, 0x2a] {
                return None;
            }
            (le16(b, 26)? & 0x3fff, le16(b, 28)? & 0x3fff)
        }
        b"VP8L" => {
            if *b.get(20)? != 0x2f {
                return None;
            }
            let bits = u32::from_le_bytes(b.get(21..25)?.try_into().ok()?);
            ((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1)
        }
        b"VP8X" => (le24(b, 24)? + 1, le24(b, 27)? + 1),
        _ => return None,
    };
    Some(Header {
        mime: "image/webp",
        width,
        height,
    })
}

fn header(bytes: &[u8]) -> Option<Header> {
    png(bytes).or_else(|| jpeg(bytes)).or_else(|| webp(bytes))
}

/// The layer name for a file: its name without folders and the last extension.
fn layer_name(file: &str) -> String {
    let file = file.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = match file.rfind('.') {
        Some(0) | None => file,
        Some(dot) => &file[..dot],
    };
    if stem.trim().is_empty() {
        "Image".to_owned()
    } else {
        stem.to_owned()
    }
}

impl Importer for ImageImporter {
    fn id(&self) -> &'static str {
        "image"
    }

    fn recognizes(&self, input: &ImportInput<'_>) -> bool {
        header(input.bytes).is_some()
    }

    fn import(&self, input: &ImportInput<'_>) -> Result<Imported, ImportError> {
        let found = header(input.bytes).ok_or_else(|| {
            ImportError::new(
                "This is not a PNG, JPEG or WEBP picture, or its header is damaged. Other formats are not imported.",
            )
        })?;
        if found.width == 0 || found.height == 0 || found.width > LIMIT || found.height > LIMIT {
            return Err(ImportError::new(format!(
                "The picture claims to be {} by {} pixels; pictures must be between 1 and {LIMIT} pixels on each side.",
                found.width, found.height
            )));
        }
        let (w, h) = (f64::from(found.width), f64::from(found.height));
        let shrink = (MAX_SIDE / w.max(h)).min(1.0);
        let mut notes = Vec::new();
        if shrink < 1.0 {
            notes.push(format!(
                "The picture is {} by {} pixels. It is laid out at {} by {}; the stored file keeps every pixel.",
                found.width,
                found.height,
                (w * shrink).round(),
                (h * shrink).round()
            ));
        }
        Ok(Imported {
            nodes: vec![json!({
                "type": "image",
                "name": layer_name(input.name),
                "width": (w * shrink).round(),
                "height": (h * shrink).round(),
                "asset": asset_ref(0),
                "fit": "fill",
            })],
            assets: vec![ImportedAsset {
                mime: found.mime.to_owned(),
                bytes: input.bytes.to_vec(),
            }],
            notes,
        })
    }
}
