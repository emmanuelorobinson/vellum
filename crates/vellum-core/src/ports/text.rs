//! Text shaping and paragraph layout. Today: Parley. Alternatives: cosmic-text, Skia Paragraph.

use vellum_types::{FontStyle, TextAlign, TextResize, VerticalAlign};

use crate::ports::{Extent, FontHandle};

/// One stretch of text in one style, with every variable already resolved to a number.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledRun {
    /// The characters. A `\n` ends the line.
    pub text: String,
    /// Font family.
    pub family: String,
    /// 100 to 900.
    pub weight: u16,
    /// Roman or italic.
    pub style: FontStyle,
    /// Font size in pixels.
    pub size: f64,
    /// Line height in pixels; the font's own when absent.
    pub line_height: Option<f64>,
    /// Extra space after each letter in pixels.
    pub letter_spacing: f64,
    /// Draw an underline using the selected font metrics.
    pub underline: bool,
    /// Draw a strike using the selected font metrics.
    pub strikethrough: bool,
    /// Link target, carried as data; shaping never follows it.
    pub link: Option<String>,
}

/// Text ready to measure: styled runs and how lines sit in the box.
#[derive(Debug, Clone, PartialEq)]
pub struct RichText {
    /// The runs, in reading order.
    pub runs: Vec<StyledRun>,
    /// Where each line sits between the box edges.
    pub align: TextAlign,
    /// Where content sits in a fixed-height box.
    pub vertical_align: VerticalAlign,
    /// Box sizing policy used by `layout_box`.
    pub resize: TextResize,
}

impl RichText {
    /// One run of regular roman text, left aligned.
    pub fn plain(text: impl Into<String>, family: impl Into<String>, size: f64) -> Self {
        Self {
            runs: vec![StyledRun {
                text: text.into(),
                family: family.into(),
                weight: 400,
                style: FontStyle::Normal,
                size,
                line_height: None,
                letter_spacing: 0.0,
                underline: false,
                strikethrough: false,
                link: None,
            }],
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            resize: TextResize::Fixed,
        }
    }

    /// All the characters, runs joined.
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// How much room text takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// The widest line.
    pub width: f64,
    /// All the lines.
    pub height: f64,
    /// How many lines.
    pub lines: usize,
}

/// One glyph, placed on its line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    /// The glyph in its font.
    pub id: u32,
    /// Byte offset of the character it came from, in `RichText::text()`.
    pub cluster: usize,
    /// Distance from the left edge of the line to the glyph's origin.
    pub x: f64,
    /// Offset from the line baseline, including combining-mark positioning.
    pub y: f64,
    /// How far the pen moves after it.
    pub advance: f64,
}

/// Consecutive glyphs in one font at one size.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRun {
    /// The font to draw with.
    pub font: FontHandle,
    /// Font size in pixels.
    pub size: f64,
    /// The glyphs, left to right.
    pub glyphs: Vec<Glyph>,
    /// Index of the original styled run; preserves link and style ownership through bidi/fallback.
    pub source_run: usize,
    /// Normalized variable-font axis coordinates, signed F2Dot14, in font axis order.
    pub normalized_coords: Vec<i16>,
    /// Synthetic bold requested by the shaper.
    pub embolden: bool,
    /// Synthetic italic angle in degrees, if requested by the shaper.
    pub skew: Option<f64>,
    /// Underline relative to the baseline, if requested.
    pub underline: Option<TextDecoration>,
    /// Strike relative to the baseline, if requested.
    pub strikethrough: Option<TextDecoration>,
    /// Link target, never executed by the engine.
    pub link: Option<String>,
}

/// One line of laid-out text.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    /// Byte range of the characters on this line, in `RichText::text()`. A hard break or the space a line
    /// broke at belongs to the line it ends.
    pub range: std::ops::Range<usize>,
    /// Where the line starts, from the left edge of the text box.
    pub x: f64,
    /// Top of the line, from the top of the text box.
    pub y: f64,
    /// Width of the line's ink-bearing characters, without trailing spaces.
    pub width: f64,
    /// Height of the line.
    pub height: f64,
    /// Distance from the top of the line to the baseline.
    pub baseline: f64,
    /// What to draw.
    pub glyph_runs: Vec<GlyphRun>,
}

/// Paragraphs laid out at a width.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    /// The widest line.
    pub width: f64,
    /// All the lines.
    pub height: f64,
    /// Top to bottom.
    pub lines: Vec<TextLine>,
    /// Explicit missing families, substitutes and uncovered characters.
    pub problems: Vec<TextProblem>,
}

/// A decoration drawn from the run's first origin across its advances.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextDecoration {
    /// Signed y offset from the baseline, positive down.
    pub offset: f64,
    /// Stroke thickness in pixels.
    pub thickness: f64,
}

/// A text dependency that could not be satisfied. Byte ranges refer to `RichText::text()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextProblem {
    /// Requested family is absent; the named substitute was used.
    MissingFont { requested: String, substitute: String, range: std::ops::Range<usize> },
    /// No supplied font covers this cluster (a .notdef glyph is drawn).
    MissingGlyph { font: FontHandle, range: std::ops::Range<usize> },
}

/// Box dimensions beside the complete content layout. Overflow is reported without truncation.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBoxLayout {
    /// All glyphs and lines; their positions include vertical alignment.
    pub layout: TextLayout,
    /// Resolved box width.
    pub width: f64,
    /// Resolved box height.
    pub height: f64,
    /// Ink-bearing advances exceed the box width.
    pub overflows_x: bool,
    /// Content line boxes exceed the box height.
    pub overflows_y: bool,
}

/// Measures and lays out text.
pub trait TextEngine {
    /// How much room `text` takes when wrapped inside `constraints`. Only the width constrains; an unbounded
    /// width never wraps except at hard breaks. Agrees with [`TextEngine::layout`] at the same width.
    fn measure(&self, text: &RichText, constraints: Extent) -> TextMetrics;
    /// The glyphs of `text` wrapped at `width` (`f64::INFINITY` for never).
    fn layout(&self, text: &RichText, width: f64) -> TextLayout;

    /// Apply auto-width, auto-height or fixed sizing. Auto-width keeps hard breaks and ignores the
    /// supplied width; auto-height wraps at it. Fixed preserves both dimensions. Vertical alignment
    /// uses spare space only, so overflowing text always begins at the top. No clipping is performed.
    fn layout_box(&self, text: &RichText, extent: Extent) -> TextBoxLayout {
        let wrap = if text.resize == TextResize::AutoWidth { f64::INFINITY } else { extent.width };
        let mut layout = self.layout(text, wrap);
        let width = if text.resize == TextResize::AutoWidth || !extent.width.is_finite() { layout.width } else { extent.width.max(0.0) };
        let height = if text.resize != TextResize::Fixed || !extent.height.is_finite() { layout.height } else { extent.height.max(0.0) };
        let spare = (height - layout.height).max(0.0);
        let offset = match text.vertical_align { VerticalAlign::Top => 0.0, VerticalAlign::Middle => spare / 2.0, VerticalAlign::Bottom => spare };
        for line in &mut layout.lines { line.y += offset; }
        TextBoxLayout { overflows_x: layout.width > width + 1e-6, overflows_y: layout.height > height + 1e-6, layout, width, height }
    }
}
