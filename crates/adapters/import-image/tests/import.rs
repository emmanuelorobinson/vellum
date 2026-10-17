//! TEST DATA. Headers only: the importer reads size and kind, it does not decode pixels.
use serde_json::json;
use vellum_core::ports::{ImportInput, Importer, asset_ref};
use vellum_import_image::ImageImporter;

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend_from_slice(&13u32.to_be_bytes());
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b.extend_from_slice(b"\0\0\0\0IEND\xaeB`\x82");
    b
}

fn jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut b = vec![0xff, 0xd8, 0xff, 0xe0, 0, 16];
    b.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    b.extend_from_slice(&[0xff, 0xc0, 0, 11, 8]);
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&[1, 1, 0x11, 0]);
    b.extend_from_slice(&[0xff, 0xd9]);
    b
}

fn riff(chunk: &[u8]) -> Vec<u8> {
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&(4 + chunk.len() as u32).to_le_bytes());
    b.extend_from_slice(b"WEBP");
    b.extend_from_slice(chunk);
    b
}

fn webp_lossy(w: u16, h: u16) -> Vec<u8> {
    let mut c = b"VP8 ".to_vec();
    c.extend_from_slice(&10u32.to_le_bytes());
    c.extend_from_slice(&[0x10, 0, 0, 0x9d, 0x01, 0x2a]);
    c.extend_from_slice(&w.to_le_bytes());
    c.extend_from_slice(&h.to_le_bytes());
    riff(&c)
}

fn webp_lossless(w: u32, h: u32) -> Vec<u8> {
    let mut c = b"VP8L".to_vec();
    c.extend_from_slice(&5u32.to_le_bytes());
    c.push(0x2f);
    let bits = (w - 1) | ((h - 1) << 14);
    c.extend_from_slice(&bits.to_le_bytes());
    riff(&c)
}

fn webp_extended(w: u32, h: u32) -> Vec<u8> {
    let mut c = b"VP8X".to_vec();
    c.extend_from_slice(&10u32.to_le_bytes());
    c.extend_from_slice(&[0, 0, 0, 0]);
    c.extend_from_slice(&(w - 1).to_le_bytes()[..3]);
    c.extend_from_slice(&(h - 1).to_le_bytes()[..3]);
    riff(&c)
}

fn run(
    bytes: &[u8],
    name: &str,
) -> Result<vellum_core::ports::Imported, vellum_core::ports::ImportError> {
    ImageImporter.import(&ImportInput { bytes, name })
}

#[test]
fn passes_the_importer_contract() {
    vellum_contracts::suites::import::importer(&ImageImporter, &png(32, 16));
}

#[test]
fn every_format_gives_an_image_node_of_its_own_size_and_one_asset() {
    let cases = [
        (png(320, 200), "image/png"),
        (jpeg(320, 200), "image/jpeg"),
        (webp_lossy(320, 200), "image/webp"),
        (webp_lossless(320, 200), "image/webp"),
        (webp_extended(320, 200), "image/webp"),
    ];
    for (bytes, mime) in cases {
        let imported = run(&bytes, "photos/Hero Shot.final.img").expect(mime);
        assert_eq!(
            imported.nodes,
            vec![
                json!({"type":"image","name":"Hero Shot.final","width":320.0,"height":200.0,"asset":asset_ref(0),"fit":"fill"})
            ],
            "{mime}"
        );
        assert_eq!(imported.assets.len(), 1);
        assert_eq!(imported.assets[0].mime, mime);
        assert_eq!(
            imported.assets[0].bytes, bytes,
            "the original bytes are stored untouched"
        );
        assert!(imported.notes.is_empty(), "{:?}", imported.notes);
    }
}

#[test]
fn big_pictures_are_laid_out_smaller_and_the_note_says_so() {
    let imported = run(&png(4000, 2000), "wide.png").unwrap();
    assert_eq!(imported.nodes[0]["width"], json!(1600.0));
    assert_eq!(imported.nodes[0]["height"], json!(800.0));
    assert!(
        imported.notes[0].contains("4000 by 2000"),
        "{:?}",
        imported.notes
    );
    let tall = run(&jpeg(1000, 3200), "tall.jpg").unwrap();
    assert_eq!(tall.nodes[0]["height"], json!(1600.0));
    assert_eq!(tall.nodes[0]["width"], json!(500.0));
}

#[test]
fn a_blank_name_still_names_the_layer() {
    let imported = run(&png(2, 2), "").unwrap();
    assert_eq!(imported.nodes[0]["name"], "Image");
    let imported = run(&png(2, 2), "dir/").unwrap();
    assert_eq!(imported.nodes[0]["name"], "Image");
}

#[test]
fn what_is_not_a_picture_or_has_no_size_is_refused() {
    for bytes in [
        &b"GIF89a\x01\0\x01\0"[..],
        b"<svg xmlns='http://www.w3.org/2000/svg'/>",
        &png(0, 5),
        &png(5, 0),
        &png(100_000, 5),
        &jpeg(0, 0)[..],
        &b"\xff\xd8\xff\xd9"[..],
        &b"RIFF\0\0\0\0WEBPVP8 "[..],
        &b"\x89PNG\r\n\x1a\n"[..],
    ] {
        assert!(run(bytes, "x").is_err(), "{bytes:?}");
    }
    let err = run(b"GIF89a....", "x").unwrap_err().to_string();
    assert!(err.contains("PNG, JPEG or WEBP"), "{err}");
}

#[test]
fn jpegs_with_other_segments_before_the_frame_header_are_read() {
    let mut b = vec![0xff, 0xd8];
    b.extend_from_slice(&[0xff, 0xe1, 0, 6, b'E', b'x', b'i', b'f']);
    b.extend_from_slice(&[0xff, 0xdb, 0, 4, 0, 0]);
    b.extend_from_slice(&[0xff, 0xc2, 0, 11, 8, 0x01, 0x00, 0x02, 0x00, 1, 1, 0x11, 0]);
    let imported = run(&b, "p.jpg").unwrap();
    assert_eq!(imported.nodes[0]["width"], json!(512.0));
    assert_eq!(imported.nodes[0]["height"], json!(256.0));
}
