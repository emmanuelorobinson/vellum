use vellum_core::ports::{
    Extent, FontHandle, Glyph, GlyphRun, RichText, TextEngine, TextLayout, TextLine, TextMetrics,
};
use vellum_types::TextAlign;

/// Every glyph is 0.6 em wide, plus letter spacing, and a line is 1.2 em tall unless the run sets a line
/// height. Lines break at spaces (a space stays on the line it ends and takes no width there) and at `\n`; a
/// word wider than the box overflows it. Text with runs but no characters is one empty line.
#[derive(Debug, Clone, Copy)]
pub struct FixedMetricText {
    font: FontHandle,
}

impl FixedMetricText {
    /// The engine, drawing with font handle 0.
    pub fn new() -> Self {
        Self {
            font: FontHandle(0),
        }
    }
}

impl Default for FixedMetricText {
    fn default() -> Self {
        Self::new()
    }
}

struct Ch {
    ch: char,
    byte: usize,
    advance: f64,
    size: f64,
    line_height: f64,
}

fn chars(text: &RichText) -> Vec<Ch> {
    let mut out = Vec::new();
    let mut byte = 0;
    for run in &text.runs {
        for ch in run.text.chars() {
            out.push(Ch {
                ch,
                byte,
                advance: 0.6 * run.size + run.letter_spacing,
                size: run.size,
                line_height: run.line_height.unwrap_or(1.2 * run.size),
            });
            byte += ch.len_utf8();
        }
    }
    out
}

/// Char index ranges of the lines.
fn break_lines(chars: &[Ch], width: f64) -> Vec<(usize, usize)> {
    let n = chars.len();
    let mut lines = Vec::new();
    let (mut start, mut i) = (0, 0);
    let mut full = 0.0;
    while i < n {
        if chars[i].ch == '\n' {
            lines.push((start, i + 1));
            (start, i, full) = (i + 1, i + 1, 0.0);
            continue;
        }
        let mut j = i;
        while j < n && chars[j].ch != ' ' && chars[j].ch != '\n' {
            j += 1;
        }
        let word: f64 = chars[i..j].iter().map(|c| c.advance).sum();
        let mut k = j;
        while k < n && chars[k].ch == ' ' {
            k += 1;
        }
        let spaces: f64 = chars[j..k].iter().map(|c| c.advance).sum();
        if start < i && full + word > width + 1e-9 {
            lines.push((start, i));
            (start, full) = (i, 0.0);
        }
        full += word + spaces;
        i = k;
    }
    if start < n || n == 0 || chars.last().is_some_and(|c| c.ch == '\n') {
        lines.push((start, n));
    }
    lines
}

impl TextEngine for FixedMetricText {
    fn measure(&self, text: &RichText, constraints: Extent) -> TextMetrics {
        let layout = self.layout(text, constraints.width);
        TextMetrics {
            width: layout.width,
            height: layout.height,
            lines: layout.lines.len(),
        }
    }

    fn layout(&self, text: &RichText, width: f64) -> TextLayout {
        let Some(first) = text.runs.first() else {
            return TextLayout {
                width: 0.0,
                height: 0.0,
                lines: vec![],
            };
        };
        let chars = chars(text);
        let total_bytes = text.runs.iter().map(|r| r.text.len()).sum::<usize>();
        let spans = break_lines(&chars, width);

        let ink_of = |from: usize, to: usize| -> f64 {
            let mut end = to;
            while end > from && matches!(chars[end - 1].ch, ' ' | '\n') {
                end -= 1;
            }
            chars[from..end].iter().map(|c| c.advance).sum()
        };
        let widest = spans.iter().map(|&(a, b)| ink_of(a, b)).fold(0.0, f64::max);
        let box_width = if width.is_finite() { width } else { widest };

        let mut lines = Vec::new();
        let mut top = 0.0;
        for (from, to) in spans {
            let ink = ink_of(from, to);
            let height = chars[from..to]
                .iter()
                .map(|c| c.line_height)
                .fold(0.0, f64::max)
                .max(if from < to {
                    0.0
                } else {
                    first.line_height.unwrap_or(1.2 * first.size)
                });
            let x = match text.align {
                TextAlign::Center => (box_width - ink) / 2.0,
                TextAlign::Right => box_width - ink,
                TextAlign::Left | TextAlign::Justify => 0.0,
            };
            let mut runs: Vec<GlyphRun> = Vec::new();
            let mut pen = 0.0;
            for c in &chars[from..to] {
                if matches!(c.ch, '\n') {
                    continue;
                }
                if runs.last().is_none_or(|r| r.size != c.size) {
                    runs.push(GlyphRun {
                        font: self.font,
                        size: c.size,
                        glyphs: vec![],
                    });
                }
                if let Some(run) = runs.last_mut() {
                    run.glyphs.push(Glyph {
                        id: c.ch as u32,
                        cluster: c.byte,
                        x: pen,
                        advance: c.advance,
                    });
                }
                pen += c.advance;
            }
            let start_byte = chars.get(from).map_or(total_bytes, |c| c.byte);
            let end_byte = chars.get(to).map_or(total_bytes, |c| c.byte);
            lines.push(TextLine {
                range: start_byte..end_byte,
                x,
                y: top,
                width: ink,
                height,
                baseline: 0.8 * height,
                glyph_runs: runs,
            });
            top += height;
        }
        TextLayout {
            width: widest,
            height: top,
            lines,
        }
    }
}
