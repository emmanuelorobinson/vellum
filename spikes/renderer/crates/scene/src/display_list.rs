//! The display list: a flat `f32` buffer of commands, the cheapest thing to hand across the
//! Rust/JS boundary (one `Float32Array` view over WASM memory, no per-node marshalling).
//!
//! Layout: `[opcode, args...]`. A paint is encoded inline as `[blend, kind, kind args...]`.
//! The TypeScript reader in `web/src/renderer-a.js` must match this file.

use crate::documents::{Color, Document, Node, Paint, Stroke};

pub mod op {
    pub const SAVE: f32 = 0.0;
    pub const RESTORE: f32 = 1.0;
    /// dx dy
    pub const TRANSLATE: f32 = 2.0;
    /// degrees, about (cx, cy): deg cx cy
    pub const ROTATE: f32 = 3.0;
    /// x y w h radius
    pub const CLIP_RRECT: f32 = 4.0;
    /// alpha (0..1)
    pub const SAVE_LAYER_ALPHA: f32 = 5.0;
    /// x y w h radius, paint
    pub const FILL_RRECT: f32 = 10.0;
    /// x y w h radius width r g b a
    pub const STROKE_RRECT: f32 = 11.0;
    /// x y w h, paint
    pub const FILL_OVAL: f32 = 12.0;
    /// x y w h width r g b a
    pub const STROKE_OVAL: f32 = 13.0;
    /// n x0 y0 ... x(n-1) y(n-1), paint
    pub const FILL_POLY: f32 = 14.0;
    /// image x y w h smooth(0|1)
    pub const IMAGE: f32 = 15.0;
    /// string x y size r g b a
    pub const TEXT: f32 = 16.0;
    /// font size r g b a n (glyph x y)*n; font indexes `FONT_FILES`, positions are baseline origins
    pub const GLYPHS: f32 = 17.0;
}

pub mod paint_kind {
    /// r g b a
    pub const SOLID: f32 = 0.0;
    /// x0 y0 x1 y1 n (pos r g b a)*n
    pub const LINEAR: f32 = 1.0;
    /// cx cy radius n (pos r g b a)*n
    pub const RADIAL: f32 = 2.0;
}

pub mod blend {
    pub const SRC_OVER: u8 = 0;
    pub const MULTIPLY: u8 = 1;
    pub const SCREEN: u8 = 2;
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplayList {
    pub ops: Vec<f32>,
    pub strings: Vec<String>,
}

impl DisplayList {
    pub fn build(doc: &Document) -> Self {
        let mut list = DisplayList::default();
        for node in &doc.nodes {
            list.node(node);
        }
        list
    }

    /// The string table as one UTF-8 buffer plus `[offset, len]` pairs, for the JS side.
    pub fn string_table(&self) -> (Vec<u8>, Vec<u32>) {
        let mut bytes = Vec::new();
        let mut index = Vec::with_capacity(self.strings.len() * 2);
        for s in &self.strings {
            index.push(bytes.len() as u32);
            index.push(s.len() as u32);
            bytes.extend_from_slice(s.as_bytes());
        }
        (bytes, index)
    }

    fn push(&mut self, values: &[f32]) {
        self.ops.extend_from_slice(values);
    }

    fn color(&mut self, c: Color) {
        self.push(&[c.r, c.g, c.b, c.a]);
    }

    fn paint(&mut self, paint: &Paint, blend: u8) {
        self.push(&[blend as f32]);
        match paint {
            Paint::Solid(c) => {
                self.push(&[paint_kind::SOLID]);
                self.color(*c);
            }
            Paint::Linear { from, to, stops } => {
                self.push(&[
                    paint_kind::LINEAR,
                    from.0,
                    from.1,
                    to.0,
                    to.1,
                    stops.len() as f32,
                ]);
                self.stops(stops);
            }
            Paint::Radial {
                center,
                radius,
                stops,
            } => {
                self.push(&[
                    paint_kind::RADIAL,
                    center.0,
                    center.1,
                    *radius,
                    stops.len() as f32,
                ]);
                self.stops(stops);
            }
        }
    }

    fn stops(&mut self, stops: &[(f32, Color)]) {
        for (pos, c) in stops {
            self.push(&[*pos]);
            self.color(*c);
        }
    }

    fn stroke(&mut self, s: Stroke) {
        self.push(&[s.width]);
        self.color(s.color);
    }

    fn node(&mut self, node: &Node) {
        match node {
            Node::Frame {
                x,
                y,
                w,
                h,
                radius,
                fill,
                clip,
                opacity,
                children,
            } => {
                self.push(&[op::SAVE, op::TRANSLATE, *x, *y]);
                if *opacity < 1.0 {
                    self.push(&[op::SAVE_LAYER_ALPHA, *opacity]);
                }
                self.push(&[op::FILL_RRECT, 0.0, 0.0, *w, *h, *radius]);
                self.paint(fill, 0);
                if *clip {
                    self.push(&[op::CLIP_RRECT, 0.0, 0.0, *w, *h, *radius]);
                }
                for child in children {
                    self.node(child);
                }
                if *opacity < 1.0 {
                    self.push(&[op::RESTORE]);
                }
                self.push(&[op::RESTORE]);
            }
            Node::Rect {
                x,
                y,
                w,
                h,
                radius,
                rotation,
                fill,
                stroke,
                blend,
            } => {
                let rotated = *rotation != 0.0;
                if rotated {
                    self.push(&[op::SAVE, op::ROTATE, *rotation, x + w / 2.0, y + h / 2.0]);
                }
                if let Some(fill) = fill {
                    self.push(&[op::FILL_RRECT, *x, *y, *w, *h, *radius]);
                    self.paint(fill, *blend);
                }
                if let Some(s) = stroke {
                    self.push(&[op::STROKE_RRECT, *x, *y, *w, *h, *radius]);
                    self.stroke(*s);
                }
                if rotated {
                    self.push(&[op::RESTORE]);
                }
            }
            Node::Ellipse {
                x,
                y,
                w,
                h,
                fill,
                stroke,
                blend,
            } => {
                if let Some(fill) = fill {
                    self.push(&[op::FILL_OVAL, *x, *y, *w, *h]);
                    self.paint(fill, *blend);
                }
                if let Some(s) = stroke {
                    self.push(&[op::STROKE_OVAL, *x, *y, *w, *h]);
                    self.stroke(*s);
                }
            }
            Node::Polygon { points, fill } => {
                self.push(&[op::FILL_POLY, points.len() as f32]);
                for (px, py) in points {
                    self.push(&[*px, *py]);
                }
                self.paint(fill, 0);
            }
            Node::Text {
                x,
                y,
                size,
                color,
                text,
            } => {
                let id = self.strings.len() as f32;
                self.strings.push(text.clone());
                self.push(&[op::TEXT, id, *x, *y, *size]);
                self.color(*color);
            }
            Node::Glyphs {
                font,
                size,
                color,
                glyphs,
            } => {
                self.push(&[op::GLYPHS, *font as f32, *size]);
                self.color(*color);
                self.push(&[glyphs.len() as f32]);
                for (id, x, y) in glyphs {
                    // Glyph ids are below 2^16, exact in f32.
                    self.push(&[*id as f32, *x, *y]);
                }
            }
            Node::Image {
                x,
                y,
                w,
                h,
                image,
                smooth,
            } => {
                self.push(&[
                    op::IMAGE,
                    *image as f32,
                    *x,
                    *y,
                    *w,
                    *h,
                    if *smooth { 1.0 } else { 0.0 },
                ]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;

    #[test]
    fn display_list_is_deterministic_and_balanced() {
        let doc = generate(1_000);
        let a = DisplayList::build(&doc);
        assert_eq!(a, DisplayList::build(&doc));
        assert!(!a.ops.is_empty());
        assert!(!a.strings.is_empty());
    }

    #[test]
    fn string_table_round_trips() {
        let list = DisplayList::build(&generate(1_000));
        let (bytes, index) = list.string_table();
        for (i, s) in list.strings.iter().enumerate() {
            let (o, l) = (index[i * 2] as usize, index[i * 2 + 1] as usize);
            assert_eq!(std::str::from_utf8(&bytes[o..o + l]).unwrap(), s);
        }
    }
}
