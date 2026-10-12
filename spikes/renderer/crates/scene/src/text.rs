//! Text layout with Parley, as Prompt 19 will do it: Rust shapes, applies bidi and breaks lines, and
//! hands every renderer positioned glyphs. Renderers never lay out text themselves, so the only thing
//! that can differ between the paths is how a glyph is rasterized.
//!
//! No system fonts: the engine registers exactly the files in [`FONT_FILES`], in that order, and the
//! order is both the font stack and the font index a `Node::Glyphs` carries.

use parley::fontique::Blob;
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontFamilyName, Layout, LayoutContext,
    LineHeight, PositionedLayoutItem, StyleProperty,
};

use crate::documents::{Color, Node};

/// The fixture fonts, in font-stack order (see `fonts.json`).
pub const FONT_FILES: [&str; 5] = [
    "NotoSans-Regular.ttf",
    "NotoSansArabic-Regular.ttf",
    "NotoSansHebrew-Regular.ttf",
    "NotoSansSC-Regular.otf",
    "NotoColorEmoji.ttf",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

/// One paragraph to lay out: text, size and the box it wraps in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBlock<'a> {
    pub text: &'a str,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub line_height: f32,
    /// The box width: lines wrap at it and alignment is relative to it. Parley aligns inside the
    /// break width, so an unbounded width would push end-aligned (and RTL) text off the canvas.
    pub width: f32,
    pub align: TextAlign,
    pub color: Color,
}

/// Owns Parley's contexts and the registered fonts. Build once, lay out many times.
pub struct TextEngine {
    font_cx: FontContext,
    layout_cx: LayoutContext<[u8; 4]>,
    families: Vec<String>,
    blob_ids: Vec<u64>,
    last_line_count: usize,
}

impl TextEngine {
    /// `fonts[i]` is the file `FONT_FILES[i]`.
    pub fn new(fonts: &[Vec<u8>]) -> Self {
        let mut font_cx = FontContext::new();
        let mut families = Vec::new();
        let mut blob_ids = Vec::new();
        for bytes in fonts {
            let blob = Blob::new(std::sync::Arc::new(bytes.clone()));
            blob_ids.push(blob.id());
            for (family, _) in font_cx.collection.register_fonts(blob, None) {
                if let Some(name) = font_cx.collection.family_name(family)
                    && !families.iter().any(|f| f == name)
                {
                    families.push(name.to_owned());
                }
            }
        }
        Self {
            font_cx,
            layout_cx: LayoutContext::new(),
            families,
            blob_ids,
            last_line_count: 0,
        }
    }

    /// The registered family names, in stack order.
    pub fn families(&self) -> &[String] {
        &self.families
    }

    /// Lines in the block most recently laid out.
    pub fn last_line_count(&self) -> usize {
        self.last_line_count
    }

    /// Lays out one block and returns one `Node::Glyphs` per glyph run, positions in document space.
    pub fn layout(&mut self, block: &TextBlock<'_>) -> Vec<Node> {
        let stack: Vec<FontFamilyName<'_>> = self
            .families
            .iter()
            .map(|f| FontFamilyName::named(f))
            .collect();
        let brush = to_rgba8(block.color);
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, block.text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::List(stack.into())));
        builder.push_default(StyleProperty::FontSize(block.size));
        builder.push_default(LineHeight::Absolute(block.line_height));
        builder.push_default(StyleProperty::Brush(brush));
        let mut layout: Layout<[u8; 4]> = builder.build(block.text);
        layout.break_all_lines(Some(block.width));
        let alignment = match block.align {
            TextAlign::Start => Alignment::Start,
            TextAlign::Center => Alignment::Center,
            TextAlign::End => Alignment::End,
        };
        layout.align(alignment, AlignmentOptions::default());

        self.last_line_count = layout.len();
        let mut nodes = Vec::new();
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let font = run.run().font();
                let index = self
                    .blob_ids
                    .iter()
                    .position(|id| *id == font.data.id())
                    .expect("Parley only uses registered fonts") as u32;
                let glyphs = run
                    .positioned_glyphs()
                    .map(|g| (g.id, block.x + g.x, block.y + g.y))
                    .collect();
                nodes.push(Node::Glyphs {
                    font: index,
                    size: run.run().font_size(),
                    color: block.color,
                    glyphs,
                });
            }
        }
        nodes
    }
}

fn to_rgba8(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a].map(|v| (v * 255.0).round() as u8)
}
