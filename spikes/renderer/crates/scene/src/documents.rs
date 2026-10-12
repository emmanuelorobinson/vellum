//! Plain-data documents. A document is a list of top-level nodes plus the images they reference.

/// Straight-alpha colour, each channel in 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    Solid(Color),
    Linear {
        from: (f32, f32),
        to: (f32, f32),
        stops: Vec<(f32, Color)>,
    },
    Radial {
        center: (f32, f32),
        radius: f32,
        stops: Vec<(f32, Color)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Frame {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        fill: Paint,
        clip: bool,
        opacity: f32,
        children: Vec<Node>,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        rotation: f32,
        fill: Option<Paint>,
        stroke: Option<Stroke>,
        blend: u8,
    },
    Ellipse {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Option<Paint>,
        stroke: Option<Stroke>,
        blend: u8,
    },
    Polygon {
        points: Vec<(f32, f32)>,
        fill: Paint,
    },
    Text {
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        text: String,
    },
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        image: u32,
        smooth: bool,
    },
}

impl Node {
    /// This node plus all of its descendants.
    pub fn count(&self) -> usize {
        match self {
            Node::Frame { children, .. } => 1 + children.iter().map(Node::count).sum::<usize>(),
            _ => 1,
        }
    }
}

/// Unpremultiplied RGBA8 pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub width: f32,
    pub height: f32,
    pub nodes: Vec<Node>,
    pub images: Vec<ImageData>,
}

impl Document {
    pub fn node_count(&self) -> usize {
        self.nodes.iter().map(Node::count).sum()
    }
}

/// A small deterministic generator (xorshift32). Spike documents need variety, not quality.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 10_000.0
    }
}

pub(crate) fn procedural_images() -> Vec<ImageData> {
    (0..4u32)
        .map(|k| {
            let (width, height) = (64u32, 64u32);
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for y in 0..height {
                for x in 0..width {
                    let check = ((x / (4 << k)) + (y / (4 << k))) % 2 == 0;
                    let (r, g, b) = match k {
                        0 => (x * 4, y * 4, 160),
                        1 => {
                            if check {
                                (240, 240, 240)
                            } else {
                                (30, 30, 60)
                            }
                        }
                        2 => ((x * y) % 256, 128, 255 - y * 4),
                        _ => {
                            if check {
                                (200, 80, 40)
                            } else {
                                (40, 160, 120)
                            }
                        }
                    };
                    rgba.extend_from_slice(&[r as u8, g as u8, b as u8, 255]);
                }
            }
            ImageData {
                width,
                height,
                rgba,
            }
        })
        .collect()
}

const WORDS: [&str; 12] = [
    "Header", "Pricing", "Settings", "Profile", "Checkout", "Overview", "Activity", "Billing",
    "Team", "Search", "Inbox", "Reports",
];

/// One card: a frame with ten descendants (11 nodes).
fn card(rng: &mut Rng, x: f32, y: f32) -> Node {
    let hue = rng.unit();
    let accent = Color::rgba(
        0.2 + 0.6 * hue,
        0.3 + 0.4 * (1.0 - hue),
        0.8 - 0.5 * hue,
        1.0,
    );
    let word = WORDS[(rng.next() % WORDS.len() as u32) as usize];
    let children = vec![
        Node::Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 36.0,
            radius: 0.0,
            rotation: 0.0,
            fill: Some(Paint::Linear {
                from: (0.0, 0.0),
                to: (200.0, 0.0),
                stops: vec![
                    (0.0, accent),
                    (1.0, Color::rgba(accent.r, accent.g, accent.b, 0.4)),
                ],
            }),
            stroke: None,
            blend: 0,
        },
        Node::Text {
            x: 12.0,
            y: 24.0,
            size: 14.0,
            color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            text: word.into(),
        },
        Node::Image {
            x: 12.0,
            y: 48.0,
            w: 56.0,
            h: 56.0,
            image: rng.next() % 4,
            smooth: true,
        },
        Node::Text {
            x: 80.0,
            y: 62.0,
            size: 12.0,
            color: Color::rgba(0.1, 0.1, 0.12, 1.0),
            text: format!("{word} item"),
        },
        Node::Text {
            x: 80.0,
            y: 80.0,
            size: 11.0,
            color: Color::rgba(0.4, 0.4, 0.45, 1.0),
            text: "Updated just now".into(),
        },
        Node::Rect {
            x: 80.0,
            y: 90.0,
            w: 100.0 * (0.3 + 0.7 * rng.unit()),
            h: 6.0,
            radius: 3.0,
            rotation: 0.0,
            fill: Some(Paint::Solid(accent)),
            stroke: None,
            blend: 0,
        },
        Node::Ellipse {
            x: 168.0,
            y: 8.0,
            w: 20.0,
            h: 20.0,
            fill: Some(Paint::Solid(Color::rgba(1.0, 1.0, 1.0, 0.85))),
            stroke: None,
            blend: 0,
        },
        Node::Rect {
            x: 12.0,
            y: 116.0,
            w: 80.0,
            h: 24.0,
            radius: 6.0,
            rotation: 0.0,
            fill: Some(Paint::Solid(accent)),
            stroke: None,
            blend: 0,
        },
        Node::Text {
            x: 24.0,
            y: 132.0,
            size: 12.0,
            color: Color::rgba(1.0, 1.0, 1.0, 1.0),
            text: "Open".into(),
        },
        Node::Polygon {
            points: vec![(176.0, 120.0), (188.0, 140.0), (164.0, 140.0)],
            fill: Paint::Solid(Color::rgba(accent.r, accent.g, accent.b, 0.6)),
        },
    ];
    Node::Frame {
        x,
        y,
        w: 200.0,
        h: 152.0,
        radius: 8.0,
        fill: Paint::Solid(Color::rgba(0.97, 0.97, 0.98, 1.0)),
        clip: true,
        opacity: 1.0,
        children,
    }
}

/// A document with exactly `nodes` nodes, laid out as a grid of cards. Cards hold 11 nodes; the
/// remainder is filled with plain rects so the count is exact.
pub fn generate(nodes: usize) -> Document {
    let mut rng = Rng(0x9E37_79B9 ^ nodes as u32);
    let cards = nodes / 11;
    let columns = (cards as f32).sqrt().ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(cards + 11);
    for i in 0..cards {
        let (col, row) = (i % columns, i / columns);
        out.push(card(
            &mut rng,
            24.0 + col as f32 * 224.0,
            24.0 + row as f32 * 176.0,
        ));
    }
    let rows = cards.div_ceil(columns).max(1);
    for k in 0..(nodes - cards * 11) {
        out.push(Node::Rect {
            x: 24.0 + k as f32 * 16.0,
            y: 24.0 + rows as f32 * 176.0,
            w: 12.0,
            h: 12.0,
            radius: 0.0,
            rotation: 0.0,
            fill: Some(Paint::Solid(Color::rgba(0.5, 0.5, 0.5, 1.0))),
            stroke: None,
            blend: 0,
        });
    }
    Document {
        width: 24.0 + columns as f32 * 224.0,
        height: 24.0 + (rows + 1) as f32 * 176.0,
        nodes: out,
        images: procedural_images(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_documents_have_exact_node_counts() {
        for n in [1_000, 5_000, 10_000] {
            assert_eq!(generate(n).node_count(), n);
        }
    }

    #[test]
    fn generation_is_deterministic() {
        assert_eq!(generate(1_000), generate(1_000));
    }

    #[test]
    fn documents_hold_frames_text_rects_and_images() {
        let doc = generate(1_000);
        let Node::Frame { children, .. } = &doc.nodes[0] else {
            panic!("first node is a frame")
        };
        assert!(children.iter().any(|c| matches!(c, Node::Text { .. })));
        assert!(children.iter().any(|c| matches!(c, Node::Rect { .. })));
        assert!(children.iter().any(|c| matches!(c, Node::Image { .. })));
    }
}
