//! `TextEngine`.

use vellum_core::ports::{Extent, RichText, TextEngine};

use super::close;

const SENTENCE: &str = "one two three four five six seven";

fn text(s: &str, size: f64) -> RichText {
    RichText::plain(s, "TEST DATA Sans", size)
}

/// Measuring agrees with laying out, wrapping narrows the lines and never widens them, hard breaks break, and
/// the lines of a layout tile the text.
pub fn run(engine: &dyn TextEngine) {
    let widths = [40.0, 80.0, 160.0, 400.0, f64::INFINITY];
    let mut last_lines = usize::MAX;
    for width in widths {
        let rich = text(SENTENCE, 10.0);
        let metrics = engine.measure(&rich, Extent::with_width(width));
        let layout = engine.layout(&rich, width);

        assert_eq!(
            metrics.lines,
            layout.lines.len(),
            "TextEngine: measure and layout count different lines at {width}"
        );
        assert!(
            close(metrics.width, layout.width),
            "TextEngine: measure and layout differ in width at {width}"
        );
        assert!(
            close(metrics.height, layout.height),
            "TextEngine: measure and layout differ in height at {width}"
        );
        assert_eq!(
            metrics,
            engine.measure(&rich, Extent::with_width(width)),
            "TextEngine: measuring twice gave two answers"
        );
        assert!(
            metrics.lines <= last_lines,
            "TextEngine: a wider box ({width}) needed more lines ({}) than a narrower one ({last_lines})",
            metrics.lines
        );
        last_lines = metrics.lines;

        if width.is_finite() {
            for (n, line) in layout.lines.iter().enumerate() {
                assert!(
                    line.width <= width + 1e-6,
                    "TextEngine: line {n} is {} wide in a box {width} wide, and none of these words is that long",
                    line.width
                );
            }
        }

        let whole = rich.text();
        let mut at = 0;
        let mut top = 0.0;
        for (n, line) in layout.lines.iter().enumerate() {
            assert_eq!(
                line.range.start,
                at,
                "TextEngine: line {n} does not start where line {} ended",
                n.wrapping_sub(1)
            );
            assert!(line.range.end >= line.range.start && line.range.end <= whole.len());
            assert!(
                close(line.y, top),
                "TextEngine: line {n} starts at {} but the lines above end at {top}",
                line.y
            );
            assert!(line.height > 0.0, "TextEngine: line {n} has no height");
            for run in &line.glyph_runs {
                for glyph in &run.glyphs {
                    assert!(
                        (line.range.start..=line.range.end).contains(&glyph.cluster),
                        "TextEngine: a glyph on line {n} comes from outside the line"
                    );
                }
            }
            at = line.range.end;
            top += line.height;
        }
        assert_eq!(
            at,
            whole.len(),
            "TextEngine: the lines stop before the end of the text"
        );
        assert!(
            close(top, layout.height),
            "TextEngine: the lines add up to {top}, not the layout's {}",
            layout.height
        );
    }
    assert_eq!(
        engine
            .measure(&text(SENTENCE, 10.0), Extent::UNBOUNDED)
            .lines,
        1,
        "TextEngine: text with no hard break and no width limit must be one line"
    );
    assert!(
        engine
            .measure(&text(SENTENCE, 10.0), Extent::with_width(40.0))
            .lines
            > 1,
        "TextEngine: seven words did not wrap in a box 40 wide"
    );

    let one = engine.measure(&text("a", 10.0), Extent::UNBOUNDED);
    let two = engine.measure(&text("a\nb", 10.0), Extent::UNBOUNDED);
    let three = engine.measure(&text("a\n\nb", 10.0), Extent::UNBOUNDED);
    assert_eq!(
        (one.lines, two.lines, three.lines),
        (1, 2, 3),
        "TextEngine: hard breaks must break lines"
    );

    let short = engine.measure(&text("aa", 10.0), Extent::UNBOUNDED);
    let long = engine.measure(&text("aaaa", 10.0), Extent::UNBOUNDED);
    assert!(
        long.width > short.width,
        "TextEngine: more letters did not make the line wider"
    );

    let small = engine.measure(&text("Hello", 10.0), Extent::UNBOUNDED);
    let big = engine.measure(&text("Hello", 20.0), Extent::UNBOUNDED);
    assert!(
        (big.width / small.width - 2.0).abs() < 0.3
            && (big.height / small.height - 2.0).abs() < 0.3,
        "TextEngine: doubling the font size should about double the text ({small:?} then {big:?})"
    );

    let nothing = engine.measure(&text("", 10.0), Extent::UNBOUNDED);
    assert!(
        close(nothing.width, 0.0),
        "TextEngine: empty text has width {}",
        nothing.width
    );
}

/// Style ownership is retained across equal-sized runs; box policy never truncates content.
pub fn rich(engine: &dyn TextEngine) {
    use vellum_types::{TextResize, VerticalAlign};
    let mut t = text("TEST DATA ", 12.0);
    let mut b = text("link", 12.0).runs.remove(0);
    b.link = Some("https://example.test".into());
    b.underline = true;
    b.strikethrough = true;
    t.runs.push(b);
    let l = engine.layout(&t, 200.0);
    assert!(
        l.lines
            .iter()
            .flat_map(|l| &l.glyph_runs)
            .any(|r| r.source_run == 1
                && r.link == t.runs[1].link
                && r.underline.is_some()
                && r.strikethrough.is_some()),
        "TextEngine: rich style was dropped"
    );
    assert!(
        l.lines
            .iter()
            .flat_map(|l| &l.glyph_runs)
            .filter(|r| r.source_run == 0)
            .all(|r| r.link.is_none() && r.underline.is_none()),
        "TextEngine: style leaked between runs"
    );
    t.resize = TextResize::AutoWidth;
    let l = engine.layout_box(
        &t,
        Extent {
            width: 20.0,
            height: 100.0,
        },
    );
    assert_eq!(l.layout.lines.len(), 1, "TextEngine: auto-width wrapped");
    t.resize = TextResize::Fixed;
    t.vertical_align = VerticalAlign::Bottom;
    let l = engine.layout_box(
        &t,
        Extent {
            width: 200.0,
            height: 100.0,
        },
    );
    assert!(
        (l.layout.lines[0].y + l.layout.height - 100.0).abs() < 0.001,
        "TextEngine: vertical alignment was dropped"
    );
}
