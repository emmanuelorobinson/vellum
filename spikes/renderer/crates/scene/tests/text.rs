//! Text fixtures and Parley layout (guide Prompt 5).
//!
//! The spec tests need nothing. The layout tests are tagged lane `fonts` (`#[ignore]`): they need the
//! five Noto fonts from `fonts.json`, fetched once with `node scripts/fetch-fonts.mjs` (network), in
//! `spikes/renderer/fonts/` or `$VELLUM_SPIKE_FONTS`. Run them with `cargo test -- --ignored`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use spike_scene::{
    DisplayList, FONT_FILES, Node, TEXT_FIXTURE_COUNT, TextCategory, TextEngine, op, text_fixture,
    text_fixture_spec,
};

const LANE: &str = "lane fonts: needs the fixture fonts (node scripts/fetch-fonts.mjs)";

fn fonts() -> Vec<Vec<u8>> {
    let dir = std::env::var_os("VELLUM_SPIKE_FONTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fonts"));
    FONT_FILES
        .iter()
        .map(|file| {
            std::fs::read(dir.join(file))
                .unwrap_or_else(|e| panic!("{}: {e}; run node scripts/fetch-fonts.mjs", file))
        })
        .collect()
}

fn font_index(file: &str) -> u32 {
    FONT_FILES.iter().position(|f| *f == file).unwrap() as u32
}

/// A glyph run as (font, glyph ids, positions).
type Run = (u32, Vec<u32>, Vec<(f32, f32)>);

/// Every glyph run of a laid-out fixture.
fn runs(doc: &spike_scene::Document) -> Vec<Run> {
    fn walk(node: &Node, out: &mut Vec<Run>) {
        match node {
            Node::Frame { children, .. } => children.iter().for_each(|c| walk(c, out)),
            Node::Glyphs { font, glyphs, .. } => out.push((
                *font,
                glyphs.iter().map(|g| g.0).collect(),
                glyphs.iter().map(|g| (g.1, g.2)).collect(),
            )),
            _ => {}
        }
    }
    let mut out = Vec::new();
    doc.nodes.iter().for_each(|n| walk(n, &mut out));
    out
}

#[test]
fn there_are_thirty_named_fixtures_covering_every_category() {
    assert_eq!(TEXT_FIXTURE_COUNT, 30);
    let mut names = BTreeSet::new();
    let mut categories = BTreeSet::new();
    for n in 1..=TEXT_FIXTURE_COUNT {
        let spec = text_fixture_spec(n);
        assert!(!spec.text.is_empty(), "fixture {n} has no text");
        assert!(names.insert(spec.name), "duplicate name {}", spec.name);
        categories.insert(spec.category);
    }
    for c in [
        TextCategory::Latin,
        TextCategory::Arabic,
        TextCategory::Cjk,
        TextCategory::Emoji,
        TextCategory::Bidi,
        TextCategory::Wrapping,
    ] {
        assert!(categories.contains(&c), "no fixture for {c:?}");
    }
}

#[test]
fn font_files_match_fonts_json() {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../fonts.json"))
        .expect("fonts.json");
    let mut at = 0;
    for file in FONT_FILES {
        let found = json[at..]
            .find(&format!("\"file\": \"{file}\""))
            .unwrap_or_else(|| panic!("{file} missing or out of order in fonts.json"));
        at += found + 1;
    }
    assert_eq!(json.matches("\"file\":").count(), FONT_FILES.len());
}

#[test]
#[ignore = "lane fonts: needs the fixture fonts (node scripts/fetch-fonts.mjs)"]
fn every_fixture_lays_out_without_missing_glyphs_inside_the_canvas() {
    let _ = LANE;
    let mut engine = TextEngine::new(&fonts());
    for n in 1..=TEXT_FIXTURE_COUNT {
        let spec = text_fixture_spec(n);
        let doc = text_fixture(n, &mut engine);
        let runs = runs(&doc);
        assert!(!runs.is_empty(), "{}: no glyph runs", spec.name);
        for (font, ids, positions) in &runs {
            assert!((*font as usize) < FONT_FILES.len());
            assert!(
                ids.iter().all(|&id| id != 0),
                "{}: .notdef glyph in font {}",
                spec.name,
                FONT_FILES[*font as usize]
            );
            for &(x, y) in positions {
                assert!(
                    (0.0..256.0).contains(&x) && (0.0..256.0).contains(&y),
                    "{}: glyph at ({x}, {y}) is outside the 256 x 256 canvas",
                    spec.name
                );
            }
        }
    }
}

#[test]
#[ignore = "lane fonts: needs the fixture fonts (node scripts/fetch-fonts.mjs)"]
fn layout_is_deterministic_across_engines() {
    let fonts = fonts();
    let (mut a, mut b) = (TextEngine::new(&fonts), TextEngine::new(&fonts));
    for n in 1..=TEXT_FIXTURE_COUNT {
        let doc = text_fixture(n, &mut a);
        assert_eq!(doc, text_fixture(n, &mut b), "fixture {n}");
        assert_eq!(
            doc,
            text_fixture(n, &mut a),
            "fixture {n} (same engine twice)"
        );
    }
}

#[test]
#[ignore = "lane fonts: needs the fixture fonts (node scripts/fetch-fonts.mjs)"]
fn each_script_is_shaped_with_its_own_font() {
    let mut engine = TextEngine::new(&fonts());
    let expect = [
        (TextCategory::Arabic, "NotoSansArabic-Regular.ttf"),
        (TextCategory::Cjk, "NotoSansSC-Regular.otf"),
        (TextCategory::Emoji, "NotoColorEmoji.ttf"),
    ];
    for n in 1..=TEXT_FIXTURE_COUNT {
        let spec = text_fixture_spec(n);
        let used: BTreeSet<u32> = runs(&text_fixture(n, &mut engine))
            .iter()
            .map(|r| r.0)
            .collect();
        for (category, file) in expect {
            if spec.category == category {
                assert!(
                    used.contains(&font_index(file)),
                    "{}: does not use {file}",
                    spec.name
                );
            }
        }
        if spec.category == TextCategory::Bidi {
            assert!(
                used.len() >= 2,
                "{}: a bidi fixture uses one font",
                spec.name
            );
        }
    }
}

#[test]
#[ignore = "lane fonts: needs the fixture fonts (node scripts/fetch-fonts.mjs)"]
fn wrapping_fixtures_break_into_several_lines() {
    let mut engine = TextEngine::new(&fonts());
    for n in 1..=TEXT_FIXTURE_COUNT {
        let spec = text_fixture_spec(n);
        if spec.category != TextCategory::Wrapping {
            continue;
        }
        let baselines: BTreeSet<i32> = runs(&text_fixture(n, &mut engine))
            .iter()
            .flat_map(|r| r.2.iter().map(|p| p.1.round() as i32))
            .collect();
        assert!(
            baselines.len() >= 3,
            "{}: {} lines",
            spec.name,
            baselines.len()
        );
    }
}

#[test]
#[ignore = "lane fonts: needs the fixture fonts (node scripts/fetch-fonts.mjs)"]
fn glyph_runs_reach_the_display_list() {
    let mut engine = TextEngine::new(&fonts());
    let doc = text_fixture(1, &mut engine);
    let list = DisplayList::build(&doc);
    assert!(list.ops.contains(&op::GLYPHS));
}
