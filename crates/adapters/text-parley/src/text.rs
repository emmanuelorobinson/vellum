use crate::{FontTier, ParleyFonts, fonts::to_style};
use parley::{
    Alignment, AlignmentOptions, FontFamily, FontFamilyName, FontWeight, Layout, LayoutContext,
    LineHeight, PositionedLayoutItem, StyleProperty,
};
use std::cell::RefCell;
use vellum_core::ports::*;
use vellum_types::TextAlign;

/// Reuses Parley's shaping context, sharing font bytes and handles with a FontSource. No IO.
pub struct ParleyText {
    fonts: ParleyFonts,
    layout_cx: RefCell<LayoutContext<usize>>,
}
impl ParleyText {
    pub fn new(fonts: ParleyFonts) -> Self {
        Self {
            fonts,
            layout_cx: RefCell::new(LayoutContext::new()),
        }
    }
    pub fn fonts(&self) -> &ParleyFonts {
        &self.fonts
    }
}
impl Default for ParleyText {
    fn default() -> Self {
        Self::new(ParleyFonts::new())
    }
}
impl TextEngine for ParleyText {
    fn measure(&self, text: &RichText, constraints: Extent) -> TextMetrics {
        let l = self.layout(text, constraints.width);
        TextMetrics {
            width: l.width,
            height: l.height,
            lines: l.lines.len(),
        }
    }
    fn layout(&self, text: &RichText, width: f64) -> TextLayout {
        if text.runs.is_empty() {
            return TextLayout {
                width: 0.0,
                height: 0.0,
                lines: vec![],
                problems: vec![],
            };
        }
        let whole = text.text();
        let mut state = self.fonts.state.borrow_mut();
        let bundled = state
            .families
            .iter()
            .find(|f| f.tier == FontTier::Bundled && f.name == "Inter")
            .expect("bundled font")
            .alias
            .clone();
        let mut problems = vec![];
        let mut stacks = Vec::new();
        let mut at = 0;
        for run in &text.runs {
            let primary = ParleyFonts::alias(&state, &run.family).unwrap_or_else(|| {
                problems.push(TextProblem::MissingFont {
                    requested: run.family.clone(),
                    substitute: "Inter".into(),
                    range: at..at + run.text.len(),
                });
                bundled.clone()
            });
            let mut stack = vec![primary];
            // Coverage fallback is explicit and deterministic: supplied document/cache/system
            // families in tier/registration order, then bundled Inter. Never consult platform defaults.
            let mut fallback: Vec<_> = state
                .families
                .iter()
                .filter(|f| f.tier != FontTier::Bundled)
                .collect();
            fallback.sort_by_key(|f| f.tier);
            for family in fallback {
                if !stack.contains(&family.alias) {
                    stack.push(family.alias.clone());
                }
            }
            if !stack.contains(&bundled) {
                stack.push(bundled.clone());
            }
            stacks.push(stack);
            at += run.text.len();
        }
        let mut cx = self.layout_cx.borrow_mut();
        let mut builder = cx.ranged_builder(&mut state.cx, &whole, 1.0, false);
        // The first style also supplies the strut for empty text and empty lines.
        let first = &text.runs[0];
        builder.push_default(StyleProperty::FontFamily(FontFamily::List(
            stacks[0]
                .iter()
                .map(|s| FontFamilyName::named(s))
                .collect::<Vec<_>>()
                .into(),
        )));
        builder.push_default(StyleProperty::FontSize(first.size as f32));
        builder.push_default(StyleProperty::LineHeight(
            first
                .line_height
                .map_or(LineHeight::NORMAL, |h| LineHeight::Absolute(h as f32)),
        ));
        at = 0;
        for (i, run) in text.runs.iter().enumerate() {
            let range = at..at + run.text.len();
            at = range.end;
            builder.push(
                StyleProperty::FontFamily(FontFamily::List(
                    stacks[i]
                        .iter()
                        .map(|s| FontFamilyName::named(s))
                        .collect::<Vec<_>>()
                        .into(),
                )),
                range.clone(),
            );
            for property in [
                StyleProperty::FontSize(run.size as f32),
                StyleProperty::FontWeight(FontWeight::new(run.weight as f32)),
                StyleProperty::FontStyle(to_style(run.style)),
                StyleProperty::LineHeight(
                    run.line_height
                        .map_or(LineHeight::NORMAL, |h| LineHeight::Absolute(h as f32)),
                ),
                StyleProperty::LetterSpacing(run.letter_spacing as f32),
                StyleProperty::Underline(run.underline),
                StyleProperty::Strikethrough(run.strikethrough),
                StyleProperty::Brush(i),
            ] {
                builder.push(property, range.clone());
            }
        }
        let mut layout: Layout<usize> = builder.build(&whole);
        let width = if width.is_finite() {
            Some(width.max(0.0).min(f32::MAX as f64) as f32)
        } else {
            None
        };
        layout.break_all_lines(width);
        layout.align(
            match text.align {
                TextAlign::Left => Alignment::Left,
                TextAlign::Right => Alignment::Right,
                TextAlign::Center => Alignment::Center,
                TextAlign::Justify => Alignment::Justify,
            },
            AlignmentOptions::default(),
        );
        let mut lines = vec![];
        for line in layout.lines() {
            let m = line.metrics();

            let mut glyph_runs = vec![];
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(item) = item else {
                    continue;
                };
                if whole.is_empty() {
                    continue;
                }
                let run = item.run();
                let style = item.style();
                let source_run = style.brush;
                let font = state
                    .faces
                    .iter()
                    .find(|f| f.blob_id == run.font().data.id() && f.data.index == run.font().index)
                    .expect("only explicit font bytes are available")
                    .handle;
                // Cluster glyphs are counted in visual order, matching positioned_glyphs. A
                // ligature's continuation graphemes yield no glyphs, preserving its logical start.
                let clusters: Vec<_> = run
                    .visual_clusters()
                    .filter(|c| c.style().brush == source_run)
                    .flat_map(|c| {
                        let range = c.text_range();
                        c.glyphs().map(move |_| range.clone())
                    })
                    .collect();
                let glyphs = item
                    .positioned_glyphs()
                    .zip(clusters)
                    .map(|(g, range)| {
                        if g.id == 0
                            && !whole[range.clone()].chars().all(char::is_whitespace)
                            && !problems.contains(&TextProblem::MissingGlyph {
                                font,
                                range: range.clone(),
                            })
                        {
                            problems.push(TextProblem::MissingGlyph {
                                font,
                                range: range.clone(),
                            });
                        }
                        Glyph {
                            id: g.id,
                            cluster: range.start,
                            x: (g.x - m.offset) as f64,
                            y: (g.y - m.baseline) as f64,
                            advance: g.advance as f64,
                        }
                    })
                    .collect();
                let metrics = run.font_metrics();
                glyph_runs.push(GlyphRun {
                    font,
                    size: run.font_size() as f64,
                    glyphs,
                    source_run,
                    normalized_coords: run
                        .normalized_coords()
                        .iter()
                        .map(|c| c.to_bits())
                        .collect(),
                    embolden: run.synthesis().embolden(),
                    skew: run.synthesis().skew().map(f64::from),
                    underline: style.underline.as_ref().map(|d| TextDecoration {
                        offset: d.offset.unwrap_or(metrics.underline_offset) as f64,
                        thickness: d.size.unwrap_or(metrics.underline_size) as f64,
                    }),
                    strikethrough: style.strikethrough.as_ref().map(|d| TextDecoration {
                        offset: d.offset.unwrap_or(metrics.strikethrough_offset) as f64,
                        thickness: d.size.unwrap_or(metrics.strikethrough_size) as f64,
                    }),
                    link: text.runs[source_run].link.clone(),
                });
            }
            let mut range = line.text_range();
            range.start = range.start.min(whole.len());
            range.end = range.end.min(whole.len());
            let content_width = if whole.is_empty() {
                0.0
            } else {
                (line.runs().map(|r| r.advance()).sum::<f32>() - m.hanging_advance).max(0.0) as f64
            };
            lines.push(TextLine {
                range,
                x: m.offset as f64,
                y: m.block_min_coord as f64,
                width: content_width,
                height: m.line_height as f64,
                baseline: (m.baseline - m.block_min_coord) as f64,
                glyph_runs,
            });
        }
        TextLayout {
            width: lines.iter().map(|l| l.width).fold(0.0, f64::max),
            height: layout.height() as f64,
            lines,
            problems,
        }
    }
}
