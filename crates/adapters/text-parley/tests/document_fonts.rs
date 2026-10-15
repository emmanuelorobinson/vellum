//! Custom fonts travel with the document and are served by `FontSource` (guide Prompt 24, ARCHITECTURE §4.3).
//! TEST DATA: the WOFF and WOFF2 files in `tests/fonts/` are real Inter Latin subsets (see its README).

use vellum_contracts::fakes::InMemoryAssets;
use vellum_core::ports::{AssetStore, FontSource};
use vellum_text_parley::{
    FontError, FontFormat, FontTier, ImportError, ParleyFonts, decode_font, sniff_font,
};
use vellum_types::{AssetId, Document, FontFace, FontStyle};

const WOFF: &[u8] = include_bytes!("fonts/inter-latin-400-normal.woff");
const WOFF2: &[u8] = include_bytes!("fonts/inter-latin-400-normal.woff2");
const TTF: &[u8] = include_bytes!("../fonts/InterVariable.ttf");

fn id_of(hash: &vellum_core::ports::AssetHash) -> AssetId {
    AssetId::new(hash.as_str()).expect("a hash is an id")
}

#[test]
fn the_container_is_read_from_the_first_four_bytes() {
    assert_eq!(sniff_font(TTF), Some(FontFormat::Ttf));
    assert_eq!(sniff_font(WOFF), Some(FontFormat::Woff));
    assert_eq!(sniff_font(WOFF2), Some(FontFormat::Woff2));
    assert_eq!(sniff_font(b"OTTO\0\0\0\0"), Some(FontFormat::Otf));
    assert_eq!(sniff_font(b"true\0\0\0\0"), Some(FontFormat::Ttf));
    assert_eq!(sniff_font(b"ttcf\0\0\0\0"), Some(FontFormat::Collection));
    for not_a_font in [&b""[..], b"abc", b"<svg/>", b"\x89PNG\r\n\x1a\n"] {
        assert_eq!(sniff_font(not_a_font), None, "{not_a_font:?}");
    }
}

#[test]
fn woff_and_woff2_unpack_to_the_same_font_a_ttf_passes_through() {
    let (format, from_woff) = decode_font(WOFF).expect("woff");
    assert_eq!(format, FontFormat::Woff);
    let (format, from_woff2) = decode_font(WOFF2).expect("woff2");
    assert_eq!(format, FontFormat::Woff2);
    for sfnt in [&from_woff, &from_woff2] {
        assert_eq!(
            sniff_font(sfnt),
            Some(FontFormat::Ttf),
            "an unpacked web font is a plain sfnt"
        );
        assert!(sfnt.len() > 10_000, "{} bytes", sfnt.len());
    }
    let (format, same) = decode_font(TTF).expect("ttf");
    assert_eq!(format, FontFormat::Ttf);
    assert_eq!(same, TTF, "a ttf is used as it is");
}

#[test]
fn bytes_that_are_not_a_font_or_are_cut_short_are_errors_never_panics() {
    assert_eq!(
        decode_font(b"hello, not a font"),
        Err(FontError::InvalidFont)
    );
    assert_eq!(decode_font(b""), Err(FontError::InvalidFont));
    for whole in [WOFF, WOFF2] {
        // Every cut of a valid file, to the byte for the first kilobyte and then in coarse steps.
        let mut lengths: Vec<usize> = (0..1024).collect();
        lengths.extend((1024..whole.len()).step_by(997));
        for len in lengths {
            assert!(
                decode_font(&whole[..len]).is_err(),
                "{len} of {} bytes was accepted",
                whole.len()
            );
        }
        // And one flipped byte at a time near the start, where the headers are.
        for at in 0..200 {
            let mut broken = whole.to_vec();
            broken[at] ^= 0xff;
            let _ = decode_font(&broken);
        }
    }
}

#[test]
fn a_file_that_says_it_unpacks_to_gigabytes_is_refused_before_it_is_unpacked() {
    for magic in [b"wOFF", b"wOF2"] {
        let mut header = WOFF2[..48].to_vec();
        header[..4].copy_from_slice(magic);
        header[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(decode_font(&header), Err(FontError::TooLarge), "{magic:?}");
    }
}

#[test]
fn importing_reads_the_family_weight_and_style_from_the_font_itself() {
    for (bytes, format) in [(WOFF, FontFormat::Woff), (WOFF2, FontFormat::Woff2)] {
        let fonts = ParleyFonts::new();
        let imported = fonts
            .import_font(FontTier::Document, bytes)
            .expect("import");
        assert_eq!(imported.format, format);
        assert_eq!(
            imported.face,
            FontFace {
                family: "Inter".into(),
                weight: 400,
                style: FontStyle::Normal
            }
        );
        assert!(!imported.handles.is_empty());
    }
}

#[test]
fn an_imported_font_is_served_ahead_of_the_bundled_one_of_the_same_family() {
    let fonts = ParleyFonts::new();
    let bundled = fonts
        .resolve("Inter", 400, FontStyle::Normal)
        .expect("bundled");
    assert_eq!(fonts.data(bundled).unwrap().bytes.len(), TTF.len());

    let imported = fonts
        .import_font(FontTier::Document, WOFF2)
        .expect("import");
    let now = fonts
        .resolve("Inter", 400, FontStyle::Normal)
        .expect("document");
    assert_eq!(
        fonts.data(now).unwrap().bytes.as_ref(),
        imported.sfnt.as_ref(),
        "a family in the document wins over the bundled one"
    );
    assert!(
        fonts.data(bundled).is_some(),
        "handles issued earlier stay readable"
    );
}

#[test]
fn the_document_says_what_family_weight_and_style_a_file_is() {
    // The document's FontFace is what text runs ask for, whatever the file calls itself.
    let assets = InMemoryAssets::new();
    let hash = assets.put(WOFF2, "font/woff2").unwrap();
    let mut doc = Document::new("TEST DATA: brand font");
    doc.fonts.insert(
        id_of(&hash),
        FontFace {
            family: "TEST DATA Brand".into(),
            weight: 700,
            style: FontStyle::Italic,
        },
    );
    let fonts = ParleyFonts::new();
    let loaded = fonts.load_document_fonts(&doc, &assets);
    assert_eq!(loaded.loaded, vec![id_of(&hash)]);
    assert!(loaded.problems.is_empty(), "{:?}", loaded.problems);

    let exact = fonts
        .resolve("TEST DATA Brand", 700, FontStyle::Italic)
        .expect("the family the document names");
    assert!(fonts.data(exact).is_some());
    assert_eq!(
        fonts.resolve("TEST DATA Brand", 400, FontStyle::Normal),
        Some(exact),
        "the nearest weight of the same family, never another family"
    );
    assert_eq!(
        fonts
            .resolve("Inter", 400, FontStyle::Normal)
            .map(|h| h != exact),
        Some(true)
    );

    let again = fonts.load_document_fonts(&doc, &assets);
    assert_eq!(again.loaded, vec![id_of(&hash)]);
    assert_eq!(
        fonts.resolve("TEST DATA Brand", 700, FontStyle::Italic),
        Some(exact),
        "loading the same document again must not add a second copy"
    );
}

#[test]
fn a_font_that_cannot_be_loaded_is_named_and_the_others_still_load() {
    let assets = InMemoryAssets::new();
    let good = assets.put(WOFF, "font/woff").unwrap();
    let junk = assets.put(b"not a font at all", "font/ttf").unwrap();
    let gone = vellum_core::ports::AssetHash::new("cd".repeat(32)).unwrap();
    let face = |family: &str| FontFace {
        family: family.into(),
        weight: 400,
        style: FontStyle::Normal,
    };
    let mut doc = Document::new("TEST DATA: broken fonts");
    doc.fonts.insert(id_of(&good), face("TEST DATA Good"));
    doc.fonts.insert(id_of(&junk), face("TEST DATA Junk"));
    doc.fonts.insert(id_of(&gone), face("TEST DATA Gone"));

    let fonts = ParleyFonts::new();
    let report = fonts.load_document_fonts(&doc, &assets);
    assert_eq!(report.loaded, vec![id_of(&good)]);
    assert_eq!(report.problems.len(), 2, "{:?}", report.problems);
    let by_family = |family: &str| {
        report
            .problems
            .iter()
            .find(|p| p.family == family)
            .unwrap_or_else(|| panic!("no problem for {family}: {:?}", report.problems))
    };
    assert!(by_family("TEST DATA Gone").reason.contains("missing"));
    assert!(by_family("TEST DATA Junk").reason.contains("not a font"));
    assert!(
        fonts
            .resolve("TEST DATA Junk", 400, FontStyle::Normal)
            .is_none()
    );
    assert!(
        fonts
            .resolve("TEST DATA Gone", 400, FontStyle::Normal)
            .is_none()
    );
    assert!(
        fonts
            .resolve("TEST DATA Good", 400, FontStyle::Normal)
            .is_some()
    );
}

#[test]
fn importing_into_a_document_stores_the_file_and_returns_the_shell_to_apply() {
    let assets = InMemoryAssets::new();
    let fonts = ParleyFonts::new();
    let doc = Document::new("TEST DATA: import");
    let import = fonts
        .import_into_document(&doc, &assets, WOFF2)
        .expect("import");
    assert!(!import.already);
    assert_eq!(
        assets
            .get(&vellum_core::ports::AssetHash::new(import.asset.as_str()).unwrap())
            .unwrap(),
        WOFF2,
        "the file is stored as it came, so it travels with the document unchanged"
    );
    assert_eq!(import.shell.fonts.get(&import.asset), Some(&import.face));
    assert_eq!(import.shell.meta, doc.meta);
    assert!(fonts.resolve("Inter", 400, FontStyle::Normal).is_some());

    // Applying the shell, then importing the same file again, changes nothing.
    let mut next = doc.clone();
    next.fonts = import.shell.fonts.clone();
    let again = fonts
        .import_into_document(&next, &assets, WOFF2)
        .expect("again");
    assert!(again.already);
    assert_eq!(again.shell.fonts, next.fonts);
}

#[test]
fn a_second_file_for_the_same_family_weight_and_style_is_refused() {
    let assets = InMemoryAssets::new();
    let fonts = ParleyFonts::new();
    let mut doc = Document::new("TEST DATA: duplicate");
    let first = fonts.import_into_document(&doc, &assets, WOFF2).unwrap();
    doc.fonts = first.shell.fonts.clone();
    match fonts.import_into_document(&doc, &assets, WOFF) {
        Err(ImportError::Duplicate { existing, family }) => {
            assert_eq!(existing, first.asset);
            assert_eq!(family, "Inter");
        }
        other => panic!("two files for one face were accepted: {other:?}"),
    }
}

#[test]
fn a_file_that_is_not_a_font_is_refused_and_not_stored() {
    let assets = InMemoryAssets::new();
    let fonts = ParleyFonts::new();
    let doc = Document::new("TEST DATA: not a font");
    assert!(matches!(
        fonts.import_into_document(&doc, &assets, b"%PDF-1.7 definitely not a font"),
        Err(ImportError::Font(FontError::InvalidFont))
    ));
    use vellum_core::ports::AssetCollect;
    assert!(
        assets.list().unwrap().is_empty(),
        "junk must not enter the document's assets"
    );
}

#[test]
fn a_font_imported_into_the_document_is_not_registered_twice_when_the_document_loads() {
    let assets = InMemoryAssets::new();
    let fonts = ParleyFonts::new();
    let mut doc = Document::new("TEST DATA: import then load");
    let import = fonts.import_into_document(&doc, &assets, WOFF2).unwrap();
    doc.fonts = import.shell.fonts.clone();
    let before = fonts.resolve("Inter", 400, FontStyle::Normal).unwrap();
    let loaded = fonts.load_document_fonts(&doc, &assets);
    assert_eq!(loaded.loaded, vec![import.asset]);
    assert_eq!(
        fonts.resolve("Inter", 400, FontStyle::Normal),
        Some(before),
        "load added a second copy of a font import had already registered"
    );
}
