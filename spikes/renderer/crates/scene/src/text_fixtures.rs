//! The 30 text fixtures (guide Prompt 5): Latin, Arabic, CJK, emoji, mixed bidi and wrapping, each a
//! 256 x 256 document on white, laid out by [`TextEngine`] and drawn by every renderer path.
//!
//! TEST DATA: the strings are fixture text, not product copy.

use crate::documents::{Color, Document, Node, Paint};
use crate::fixtures::FIXTURE_SIZE;
use crate::text::{TextAlign, TextBlock, TextEngine};

pub const TEXT_FIXTURE_COUNT: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextCategory {
    Latin,
    Arabic,
    Cjk,
    Emoji,
    Bidi,
    Wrapping,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextFixture {
    pub name: &'static str,
    pub category: TextCategory,
    pub text: &'static str,
    pub size: f32,
    pub align: TextAlign,
    /// Box width; most fixtures use the canvas minus a 16 px margin each side.
    pub width: f32,
}

const INK: Color = Color::rgba(0.07, 0.09, 0.15, 1.0);
const WHITE: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
const MARGIN: f32 = 16.0;
const FULL: f32 = FIXTURE_SIZE as f32 - 2.0 * MARGIN;

const fn f(
    name: &'static str,
    category: TextCategory,
    text: &'static str,
    size: f32,
    align: TextAlign,
    width: f32,
) -> TextFixture {
    TextFixture {
        name,
        category,
        text,
        size,
        align,
        width,
    }
}

use TextAlign::{Center, End, Start};
use TextCategory::{Arabic, Bidi, Cjk, Emoji, Latin, Wrapping};

const FIXTURES: [TextFixture; TEXT_FIXTURE_COUNT] = [
    f("latin-short", Latin, "Hello, canvas", 16.0, Start, FULL),
    f(
        "latin-small",
        Latin,
        "Layers, frames and components at 11 px",
        11.0,
        Start,
        FULL,
    ),
    f("latin-large", Latin, "Vellum", 48.0, Start, FULL),
    f(
        "latin-kerning",
        Latin,
        "AV Ta To Wa fi ffl Yo",
        24.0,
        Start,
        FULL,
    ),
    f(
        "latin-centered",
        Latin,
        "Centered heading\nand a second line",
        18.0,
        Center,
        FULL,
    ),
    f(
        "latin-accents",
        Latin,
        "Café, naïve, Ångström, Øresund, Łódź",
        15.0,
        Start,
        FULL,
    ),
    f("arabic-short", Arabic, "مرحبا بالعالم", 20.0, Start, FULL),
    f(
        "arabic-wrap",
        Arabic,
        "هذا نص عربي طويل يلتف على عدة أسطر داخل الإطار لاختبار تقسيم الأسطر",
        16.0,
        Start,
        FULL,
    ),
    f(
        "arabic-digits",
        Arabic,
        "الطبقة ١٢ من ٣٤٥",
        18.0,
        Start,
        FULL,
    ),
    f(
        "arabic-tashkeel",
        Arabic,
        "بِسْمِ ٱللَّٰهِ ٱلرَّحْمَٰنِ",
        22.0,
        Start,
        FULL,
    ),
    f("arabic-large", Arabic, "تصميم", 44.0, Center, FULL),
    f(
        "cjk-chinese",
        Cjk,
        "你好，世界。设计画布",
        18.0,
        Start,
        FULL,
    ),
    f(
        "cjk-wrap",
        Cjk,
        "这是一段较长的中文文本，用于测试在固定宽度内自动换行的效果和标点符号的处理。",
        16.0,
        Start,
        FULL,
    ),
    f(
        "cjk-japanese",
        Cjk,
        "こんにちは、デザインの世界",
        17.0,
        Start,
        FULL,
    ),
    f(
        "cjk-punctuation",
        Cjk,
        "「图层」、《组件》；颜色：红色！",
        16.0,
        Start,
        FULL,
    ),
    f(
        "cjk-mixed-latin",
        Cjk,
        "Vellum 是一个设计画布 2026 年",
        16.0,
        Start,
        FULL,
    ),
    f("emoji-single", Emoji, "🎨", 64.0, Center, FULL),
    f(
        "emoji-inline",
        Emoji,
        "Ship it 🚀 then rest 🌙",
        18.0,
        Start,
        FULL,
    ),
    f("emoji-zwj", Emoji, "👩‍💻 👨‍👩‍👧 🏳️‍🌈", 28.0, Start, FULL),
    f("emoji-modifiers", Emoji, "👍🏽 👋🏿 🇯🇵 🇧🇷", 28.0, Start, FULL),
    f(
        "bidi-latin-arabic",
        Bidi,
        "The word سلام means peace",
        16.0,
        Start,
        FULL,
    ),
    f(
        "bidi-arabic-latin",
        Bidi,
        "افتح ملف design.vellum الآن",
        17.0,
        Start,
        FULL,
    ),
    f("bidi-hebrew", Bidi, "שלום עולם", 22.0, Start, FULL),
    f(
        "bidi-hebrew-numbers",
        Bidi,
        "גרסה 2.4 מתוך 10",
        18.0,
        Start,
        FULL,
    ),
    f("bidi-rtl-end", Bidi, "Layer 3 الطبقة", 18.0, End, FULL),
    f(
        "bidi-wrap",
        Bidi,
        "Mixed text with عربي and עברית wrapping across several narrow lines",
        15.0,
        Start,
        FULL,
    ),
    f(
        "bidi-emoji-cjk",
        Bidi,
        "مرحبا 你好 hello 👋",
        18.0,
        Start,
        FULL,
    ),
    f(
        "wrap-paragraph",
        Wrapping,
        "Every change is an operation with an origin. Agents and people edit the same document, and the file on disk is the truth.",
        14.0,
        Start,
        FULL,
    ),
    f(
        "wrap-narrow",
        Wrapping,
        "Narrow columns force a break after nearly every word here",
        14.0,
        Start,
        96.0,
    ),
    f(
        "wrap-hard-breaks",
        Wrapping,
        "First paragraph.\nSecond paragraph wraps onto another line.\nThird.",
        14.0,
        Start,
        FULL,
    ),
];

pub fn text_fixture_spec(n: usize) -> TextFixture {
    assert!(
        (1..=TEXT_FIXTURE_COUNT).contains(&n),
        "text fixture {n} out of range"
    );
    FIXTURES[n - 1]
}

/// Text fixture `n` (1-based), laid out with `engine`.
pub fn text_fixture(n: usize, engine: &mut TextEngine) -> Document {
    let spec = text_fixture_spec(n);
    let block = TextBlock {
        text: spec.text,
        x: MARGIN,
        y: MARGIN,
        size: spec.size,
        line_height: (spec.size * 1.4).round(),
        width: spec.width,
        align: spec.align,
        color: INK,
    };
    let size = FIXTURE_SIZE as f32;
    Document {
        width: size,
        height: size,
        nodes: vec![Node::Frame {
            x: 0.0,
            y: 0.0,
            w: size,
            h: size,
            radius: 0.0,
            fill: Paint::Solid(WHITE),
            clip: false,
            opacity: 1.0,
            children: engine.layout(&block),
        }],
        images: Vec::new(),
    }
}
