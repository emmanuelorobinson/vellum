//! TEST DATA: production text behavior, independent of installed fonts and network.
use vellum_core::ports::*;
use vellum_text_parley::{FontTier, ParleyFonts, ParleyText};
use vellum_types::{FontStyle, TextAlign, TextResize, VerticalAlign};

fn text(s: &str) -> RichText { RichText::plain(s, "Inter", 20.0) }
fn engine() -> ParleyText { ParleyText::new(ParleyFonts::new()) }

#[test]
fn text_contract() { vellum_contracts::suites::text::run(&engine()); }

#[test]
fn rich_runs_preserve_style_link_decorations_and_render_inputs() {
    let e = engine();
    let mut t = text("office ");
    let mut second = text("link").runs.remove(0);
    second.size = 30.0; second.weight = 700; second.style = FontStyle::Italic;
    second.underline = true; second.strikethrough = true;
    second.link = Some("https://example.test/TEST-DATA".into());
    t.runs.push(second);
    let l = e.layout(&t, 500.0);
    let runs: Vec<_> = l.lines.iter().flat_map(|l| &l.glyph_runs).collect();
    let decorated = runs.iter().find(|r| r.source_run == 1).unwrap();
    assert_eq!(decorated.size, 30.0);
    assert_eq!(decorated.link, t.runs[1].link);
    assert!(decorated.underline.as_ref().unwrap().thickness > 0.0);
    assert!(decorated.strikethrough.as_ref().unwrap().thickness > 0.0);
    assert!(!decorated.normalized_coords.is_empty(), "variable Inter coordinates must reach the renderer");
    assert!(decorated.skew.is_some(), "roman-only Inter needs synthetic italic");
    assert!(runs.iter().filter(|r| r.source_run == 0).all(|r| r.underline.is_none() && r.link.is_none()));
    for r in runs {
        assert!(e.fonts().data(r.font).is_some());
        for g in &r.glyphs { assert!(g.id != 0); assert!(t.text().is_char_boundary(g.cluster)); assert!(g.y.is_finite()); }
    }
    let mut undecorated = t.clone();
    undecorated.runs[1].underline = false; undecorated.runs[1].strikethrough = false; undecorated.runs[1].link = None;
    assert_eq!(e.measure(&t, Extent::UNBOUNDED), e.measure(&undecorated, Extent::UNBOUNDED));
}

#[test]
fn spacing_line_height_alignment_and_justification() {
    let e = engine();
    let mut t = text("abc");
    let w = e.measure(&t, Extent::UNBOUNDED).width;
    t.runs[0].letter_spacing = 3.0;
    assert!(e.measure(&t, Extent::UNBOUNDED).width > w + 5.0);
    t.runs[0].line_height = Some(40.0);
    t.runs[0].text = "abc\ndef".into();
    assert_eq!(e.layout(&t, 300.0).height, 80.0);
    for (align, factor) in [(TextAlign::Left, 0.0),(TextAlign::Center,0.5),(TextAlign::Right,1.0)] {
        t.align = align;
        let l = e.layout(&t, 300.0);
        assert!((l.lines[0].x - (300.0-l.lines[0].width)*factor).abs() < 0.01);
    }
    t = text("one two three four five six seven eight nine ten");
    let natural = e.layout(&t, 150.0);
    t.align = TextAlign::Justify;
    let justified = e.layout(&t, 150.0);
    assert!(justified.lines.len() > 1);
    assert!((justified.lines[0].width - 150.0).abs() < 0.01);
    assert!(justified.lines[0].glyph_runs.iter().flat_map(|r| &r.glyphs).last().unwrap().x > natural.lines[0].glyph_runs.iter().flat_map(|r| &r.glyphs).last().unwrap().x);
    assert!(justified.lines.last().unwrap().width < 150.0);
}

#[test]
fn resize_and_vertical_alignment_report_overflow_without_dropping_text() {
    let e = engine();
    let mut t = text("one two three four five six seven");
    let extent = Extent { width: 80.0, height: 200.0 };
    t.resize = TextResize::AutoWidth;
    let auto = e.layout_box(&t, extent);
    assert_eq!(auto.layout.lines.len(), 1); assert!(auto.width > extent.width);
    t.resize = TextResize::AutoHeight;
    let wrapped = e.layout_box(&t, extent);
    assert!(wrapped.layout.lines.len() > 1); assert_eq!(wrapped.width,80.0); assert_eq!(wrapped.height,wrapped.layout.height);
    t.resize = TextResize::Fixed;
    for (align,factor) in [(VerticalAlign::Top,0.0),(VerticalAlign::Middle,0.5),(VerticalAlign::Bottom,1.0)] {
        t.vertical_align = align;
        let fixed = e.layout_box(&t, extent);
        assert_eq!((fixed.width,fixed.height),(80.0,200.0));
        assert!((fixed.layout.lines[0].y-(200.0-fixed.layout.height)*factor).abs() < 0.01);
    }
    let overflow = e.layout_box(&t, Extent {width:80.0,height:5.0});
    assert!(overflow.overflows_y); assert_eq!(overflow.layout.lines.last().unwrap().range.end,t.text().len());
    assert_eq!(overflow.layout.lines[0].y,0.0);
    let unbreakable = e.layout_box(&text("WWWWWWWWWWWWWW"), Extent {width:10.0,height:100.0});
    assert!(unbreakable.overflows_x);
}

#[test]
fn font_precedence_is_tier_then_style_and_unknown_is_explicit() {
    let fonts = ParleyFonts::new();
    let bundled = fonts.resolve("Inter",400,FontStyle::Normal).unwrap();
    let bytes = include_bytes!("../fonts/InterVariable.ttf").to_vec();
    let system = fonts.register(FontTier::System,bytes.clone()).unwrap()[0];
    let google = fonts.register(FontTier::GoogleCache,bytes.clone()).unwrap()[0];
    assert_eq!(fonts.resolve("inter",900,FontStyle::Italic),Some(google));
    let custom = fonts.register(FontTier::Document,bytes).unwrap()[0];
    assert_eq!(fonts.resolve("INTER",400,FontStyle::Normal),Some(custom));
    assert_ne!(custom,google); assert_ne!(system,bundled);
    assert_eq!(fonts.resolve("TEST DATA missing",400,FontStyle::Normal),None);
    assert!(fonts.register(FontTier::Document,b"invalid font".to_vec()).is_err());
    let e = ParleyText::new(fonts.clone());
    let t = RichText::plain("Hello", "TEST DATA missing",20.0);
    let l = e.layout(&t,200.0);
    assert!(l.problems.iter().any(|p| matches!(p,TextProblem::MissingFont {requested,substitute,..} if requested=="TEST DATA missing" && substitute=="Inter")));
    assert!(l.lines[0].glyph_runs.iter().all(|r| r.font==bundled), "fallback is always bundled Inter, not an unrelated document Inter");
    assert!(fonts.data(custom).is_some());
}

#[test]
fn cached_context_never_leaks_style_or_stales_after_font_registration() {
    let e = engine();
    let t = text("office");
    let a = e.layout(&t,200.0);
    let mut b = text("different"); b.runs[0].size=48.0; b.align=TextAlign::Right;
    e.layout(&b,400.0);
    assert_eq!(a,e.layout(&t,200.0));
    let h = e.fonts().register(FontTier::Document,include_bytes!("../fonts/InterVariable.ttf").to_vec()).unwrap()[0];
    assert!(e.layout(&t,200.0).lines[0].glyph_runs.iter().all(|r|r.font==h));
}

#[test]
fn empty_hard_breaks_clusters_and_unbounded_alignment_are_finite() {
    let e=engine();
    for s in ["", "\n", "a\n", "a\n\nb", "office café e\u{301}", " "] {
        for width in [0.0,20.0,f64::INFINITY] {
            let mut t=text(s); t.align=TextAlign::Right;
            let l=e.layout(&t,width);
            assert!(l.width.is_finite() && l.height.is_finite());
            assert_eq!(l.lines.last().unwrap().range.end,s.len());
            for line in &l.lines { assert!(line.x.is_finite() && line.y.is_finite());
                for r in &line.glyph_runs { for g in &r.glyphs { assert!(s.is_char_boundary(g.cluster)); } }
            }
        }
    }
}
