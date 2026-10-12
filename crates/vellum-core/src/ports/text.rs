//! Text shaping and paragraph layout. Today: Parley. Alternatives: cosmic-text, Skia Paragraph.

use vellum_types::{FontStyle, TextAlign};

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
}

/// Text ready to measure: styled runs and how lines sit in the box.
#[derive(Debug, Clone, PartialEq)]
pub struct RichText {
    /// The runs, in reading order.
    pub runs: Vec<StyledRun>,
    /// Where each line sits between the box edges.
    pub align: TextAlign,
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
            }],
            align: TextAlign::Left,
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
}

/// Measures and lays out text.
pub trait TextEngine {
    /// How much room `text` takes when wrapped inside `constraints`. Only the width constrains; an unbounded
    /// width never wraps except at hard breaks. Agrees with [`TextEngine::layout`] at the same width.
    fn measure(&self, text: &RichText, constraints: Extent) -> TextMetrics;
    /// The glyphs of `text` wrapped at `width` (`f64::INFINITY` for never).
    fn layout(&self, text: &RichText, width: f64) -> TextLayout;
}
