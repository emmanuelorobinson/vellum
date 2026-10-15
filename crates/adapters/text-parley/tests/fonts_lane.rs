//! TEST DATA. Tagged fonts lane: five checksum-verified Prompt 5 fonts, network once to fetch them.
//! See docs/runbooks/text.md. No GPU, browser, system fonts or network needed during these tests.
use std::{fs, path::PathBuf};
use vellum_core::ports::*;
use vellum_text_parley::{FontTier, ParleyFonts, ParleyText};
use vellum_types::{FontStyle, TextAlign};
const FILES: [&str; 5] = [
    "NotoSans-Regular.ttf",
    "NotoSansArabic-Regular.ttf",
    "NotoSansHebrew-Regular.ttf",
    "NotoSansSC-Regular.otf",
    "NotoColorEmoji.ttf",
];
fn fonts() -> (ParleyFonts, Vec<FontHandle>) {
    let fonts = ParleyFonts::new();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../spikes/renderer/fonts");
    let handles = FILES
        .iter()
        .map(|name| {
            fonts
                .register(
                    FontTier::GoogleCache,
                    fs::read(root.join(name))
                        .expect("fetch fonts: node spikes/renderer/scripts/fetch-fonts.mjs"),
                )
                .unwrap()[0]
        })
        .collect();
    (fonts, handles)
}
#[test]
#[ignore = "fonts lane: run fetch-fonts.mjs once, then cargo test -p vellum-text-parley --test fonts_lane -- --ignored"]
fn prompt5_glyphs_positions_fonts_and_line_counts_match() {
    let (fonts, handles) = fonts();
    let e = ParleyText::new(fonts);
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/text/prompt5.json")).unwrap();
    let fixtures = fixtures["fixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 30);
    for f in fixtures {
        let name = f["name"].as_str().unwrap();
        let s = f["text"].as_str().unwrap();
        let mut rich = RichText::plain(s, "Noto Sans", f["size"].as_f64().unwrap());
        rich.runs[0].line_height = Some(f["lineHeight"].as_f64().unwrap());
        // Prompt 5 uses direction-relative Start/End. The document format uses physical Left/Right.
        let rtl = matches!(
            name,
            "arabic-short"
                | "arabic-wrap"
                | "arabic-digits"
                | "arabic-tashkeel"
                | "bidi-arabic-latin"
                | "bidi-hebrew"
                | "bidi-hebrew-numbers"
                | "bidi-emoji-cjk"
        );
        rich.align = match f["align"].as_str().unwrap() {
            "Center" => TextAlign::Center,
            "End" if !rtl => TextAlign::Right,
            "Start" if rtl => TextAlign::Right,
            _ => TextAlign::Left,
        };
        let l = e.layout(&rich, f["width"].as_f64().unwrap());
        assert_eq!(
            l.lines.len(),
            f["lines"].as_u64().unwrap() as usize,
            "{name}: lines"
        );
        assert!(l.problems.is_empty(), "{name}: {:?}", l.problems);
        let actual: Vec<_> = l
            .lines
            .iter()
            .flat_map(|line| {
                line.glyph_runs.iter().flat_map(move |r| {
                    r.glyphs.iter().map(move |g| {
                        (
                            r.font,
                            r.size,
                            g.id,
                            line.x + g.x,
                            line.y + line.baseline + g.y,
                        )
                    })
                })
            })
            .collect();
        let handles = &handles;
        let expected: Vec<_> = f["runs"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|run| {
                run["glyphs"].as_array().unwrap().iter().map(move |g| {
                    (
                        handles[run["font"].as_u64().unwrap() as usize],
                        run["size"].as_f64().unwrap(),
                        g[0].as_u64().unwrap() as u32,
                        g[1].as_f64().unwrap(),
                        g[2].as_f64().unwrap(),
                    )
                })
            })
            .collect();
        assert_eq!(actual.len(), expected.len(), "{name}: glyph count");
        for (a, b) in actual.iter().zip(expected.iter()) {
            assert_eq!((a.0, a.1, a.2), (b.0, b.1, b.2), "{name}: font/size/glyph");
            assert!(
                (a.3 - b.3).abs() < 0.001 && (a.4 - b.4).abs() < 0.001,
                "{name}: position {a:?} != {b:?}"
            );
        }
    }
}
#[test]
#[ignore = "fonts lane: needs checksum-verified Prompt 5 fonts"]
fn bidi_cjk_emoji_clusters_and_font_contract() {
    let (fonts, handles) = fonts();
    vellum_contracts::suites::fonts::run(
        &fonts,
        "Noto Sans",
        "Noto Sans Arabic",
        "TEST DATA unknown",
    );
    let e = ParleyText::new(fonts);
    let s = "abc سلام xyz";
    let l = e.layout(&RichText::plain(s, "Noto Sans", 20.0), 400.0);
    let arabic: Vec<_> = l.lines[0]
        .glyph_runs
        .iter()
        .filter(|r| r.font == handles[1])
        .flat_map(|r| r.glyphs.iter().map(|g| g.cluster))
        .collect();
    assert!(
        arabic.len() > 1 && arabic.windows(2).any(|w| w[0] > w[1]),
        "RTL glyphs must keep logical byte offsets in visual order: {arabic:?}"
    );
    for (s, handle) in [
        ("你好世界你好世界", handles[3]),
        ("👩‍💻", handles[4]),
        ("👍🏽", handles[4]),
    ] {
        let l = e.layout(&RichText::plain(s, "Noto Sans", 20.0), 40.0);
        assert!(l.problems.is_empty(), "{s}: {:?}", l.problems);
        assert!(
            l.lines
                .iter()
                .flat_map(|l| &l.glyph_runs)
                .all(|r| r.font == handle)
        );
        assert!(
            l.lines
                .iter()
                .flat_map(|l| &l.glyph_runs)
                .flat_map(|r| &r.glyphs)
                .all(|g| s.is_char_boundary(g.cluster) && g.id != 0)
        );
        if s.starts_with('你') {
            assert!(l.lines.len() > 1);
            assert!(l.lines.iter().all(|l| l.width <= 40.01));
        } else {
            assert_eq!(l.lines.len(), 1);
            let gs: Vec<_> = l.lines[0]
                .glyph_runs
                .iter()
                .flat_map(|r| &r.glyphs)
                .collect();
            assert_eq!(gs.len(), 1);
            assert_eq!(gs[0].cluster, 0);
        }
    }
    assert!(
        e.fonts()
            .resolve("Noto Sans", 400, FontStyle::Normal)
            .is_some()
    );
}
