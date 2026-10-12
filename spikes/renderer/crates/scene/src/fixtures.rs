//! The 20 non-text pixel fixtures. Each is a 256 x 256 document on a white background. Fixture 1 is
//! the one the gpu-tagged test renders through both browser paths.

use crate::documents::{Color, Document, Node, Paint, Stroke, procedural_images};

pub const FIXTURE_COUNT: usize = 20;
pub const FIXTURE_SIZE: u32 = 256;

const WHITE: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
const BLUE: Color = Color::rgba(0.15, 0.39, 0.92, 1.0);
const RED: Color = Color::rgba(0.86, 0.15, 0.15, 1.0);
const GREEN: Color = Color::rgba(0.09, 0.64, 0.29, 1.0);
const AMBER: Color = Color::rgba(0.96, 0.62, 0.04, 1.0);
const INK: Color = Color::rgba(0.07, 0.09, 0.15, 1.0);

fn rect(x: f32, y: f32, w: f32, h: f32, fill: Paint) -> Node {
    Node::Rect {
        x,
        y,
        w,
        h,
        radius: 0.0,
        rotation: 0.0,
        fill: Some(fill),
        stroke: None,
        blend: 0,
    }
}

fn background(children: Vec<Node>) -> Node {
    Node::Frame {
        x: 0.0,
        y: 0.0,
        w: FIXTURE_SIZE as f32,
        h: FIXTURE_SIZE as f32,
        radius: 0.0,
        fill: Paint::Solid(WHITE),
        clip: false,
        opacity: 1.0,
        children,
    }
}

fn star(cx: f32, cy: f32, outer: f32, inner: f32, points: usize) -> Vec<(f32, f32)> {
    (0..points * 2)
        .map(|i| {
            let r = if i % 2 == 0 { outer } else { inner };
            let a = std::f32::consts::PI * i as f32 / points as f32 - std::f32::consts::FRAC_PI_2;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

/// Fixture `n` (1-based). Panics outside 1..=20.
pub fn fixture(n: usize) -> Document {
    let children = match n {
        1 => vec![rect(48.0, 48.0, 160.0, 160.0, Paint::Solid(BLUE))],
        2 => vec![Node::Rect {
            x: 32.0,
            y: 56.0,
            w: 192.0,
            h: 144.0,
            radius: 24.0,
            rotation: 0.0,
            fill: Some(Paint::Solid(GREEN)),
            stroke: None,
            blend: 0,
        }],
        3 => vec![Node::Rect {
            x: 40.0,
            y: 40.0,
            w: 176.0,
            h: 176.0,
            radius: 12.0,
            rotation: 0.0,
            fill: None,
            stroke: Some(Stroke {
                color: INK,
                width: 4.0,
            }),
            blend: 0,
        }],
        4 => vec![Node::Ellipse {
            x: 28.0,
            y: 60.0,
            w: 200.0,
            h: 136.0,
            fill: Some(Paint::Solid(RED)),
            stroke: None,
            blend: 0,
        }],
        5 => vec![rect(
            16.0,
            16.0,
            224.0,
            224.0,
            Paint::Linear {
                from: (16.0, 16.0),
                to: (240.0, 240.0),
                stops: vec![(0.0, BLUE), (1.0, AMBER)],
            },
        )],
        6 => vec![rect(
            16.0,
            16.0,
            224.0,
            224.0,
            Paint::Radial {
                center: (128.0, 128.0),
                radius: 112.0,
                stops: vec![(0.0, WHITE), (0.5, GREEN), (1.0, INK)],
            },
        )],
        7 => vec![
            rect(
                32.0,
                32.0,
                128.0,
                128.0,
                Paint::Solid(Color { a: 0.6, ..RED }),
            ),
            rect(
                96.0,
                96.0,
                128.0,
                128.0,
                Paint::Solid(Color { a: 0.6, ..BLUE }),
            ),
        ],
        8 => vec![Node::Frame {
            x: 24.0,
            y: 24.0,
            w: 208.0,
            h: 208.0,
            radius: 0.0,
            fill: Paint::Solid(AMBER),
            clip: false,
            opacity: 0.5,
            children: vec![
                rect(24.0, 24.0, 112.0, 112.0, Paint::Solid(RED)),
                rect(72.0, 72.0, 112.0, 112.0, Paint::Solid(BLUE)),
            ],
        }],
        9 => vec![Node::Rect {
            x: 64.0,
            y: 88.0,
            w: 128.0,
            h: 80.0,
            radius: 0.0,
            rotation: 30.0,
            fill: Some(Paint::Solid(INK)),
            stroke: None,
            blend: 0,
        }],
        10 => vec![Node::Frame {
            x: 40.0,
            y: 40.0,
            w: 176.0,
            h: 176.0,
            radius: 32.0,
            fill: Paint::Solid(Color::rgba(0.9, 0.92, 0.96, 1.0)),
            clip: true,
            opacity: 1.0,
            children: vec![Node::Ellipse {
                x: 88.0,
                y: 88.0,
                w: 160.0,
                h: 160.0,
                fill: Some(Paint::Solid(BLUE)),
                stroke: None,
                blend: 0,
            }],
        }],
        11 => vec![Node::Polygon {
            points: star(128.0, 132.0, 108.0, 44.0, 5),
            fill: Paint::Solid(AMBER),
        }],
        12 => vec![Node::Frame {
            x: 16.0,
            y: 16.0,
            w: 224.0,
            h: 224.0,
            radius: 8.0,
            fill: Paint::Solid(Color::rgba(0.93, 0.93, 0.95, 1.0)),
            clip: true,
            opacity: 1.0,
            children: vec![Node::Frame {
                x: 40.0,
                y: 40.0,
                w: 160.0,
                h: 160.0,
                radius: 16.0,
                fill: Paint::Solid(GREEN),
                clip: true,
                opacity: 1.0,
                children: vec![rect(-20.0, 100.0, 220.0, 100.0, Paint::Solid(INK))],
            }],
        }],
        13 | 14 => {
            let mode = if n == 13 { 1 } else { 2 };
            vec![
                rect(24.0, 24.0, 144.0, 208.0, Paint::Solid(AMBER)),
                Node::Rect {
                    x: 88.0,
                    y: 48.0,
                    w: 144.0,
                    h: 160.0,
                    radius: 0.0,
                    rotation: 0.0,
                    fill: Some(Paint::Solid(Color { a: 0.9, ..BLUE })),
                    stroke: None,
                    blend: mode,
                },
            ]
        }
        15 => vec![Node::Image {
            x: 0.0,
            y: 0.0,
            w: 256.0,
            h: 256.0,
            image: 1,
            smooth: false,
        }],
        16 => vec![Node::Image {
            x: 16.0,
            y: 16.0,
            w: 224.0,
            h: 224.0,
            image: 2,
            smooth: true,
        }],
        17 => (0..12)
            .map(|i| {
                let x = 16.5 + i as f32 * 19.0;
                Node::Rect {
                    x,
                    y: 16.5,
                    w: 1.0 + i as f32,
                    h: 223.0,
                    radius: 0.0,
                    rotation: 0.0,
                    fill: None,
                    stroke: Some(Stroke {
                        color: INK,
                        width: 1.0,
                    }),
                    blend: 0,
                }
            })
            .collect(),
        18 => (0..15 * 15)
            .map(|i| {
                let (c, r) = ((i % 15) as f32, (i / 15) as f32);
                rect(
                    8.0 + c * 16.3,
                    8.0 + r * 16.3,
                    11.7,
                    11.7,
                    Paint::Solid(Color::rgba(c / 15.0, r / 15.0, 0.6, 1.0)),
                )
            })
            .collect(),
        19 => vec![
            rect(0.0, 96.0, 256.0, 64.0, Paint::Solid(INK)),
            rect(
                16.0,
                16.0,
                224.0,
                224.0,
                Paint::Linear {
                    from: (16.0, 0.0),
                    to: (240.0, 0.0),
                    stops: vec![(0.0, Color { a: 0.0, ..RED }), (1.0, RED)],
                },
            ),
        ],
        20 => vec![Node::Ellipse {
            x: 32.0,
            y: 32.0,
            w: 192.0,
            h: 192.0,
            fill: Some(Paint::Solid(Color { a: 0.3, ..GREEN })),
            stroke: Some(Stroke {
                color: GREEN,
                width: 6.0,
            }),
            blend: 0,
        }],
        _ => panic!("fixture {n} does not exist (1..={FIXTURE_COUNT})"),
    };
    Document {
        width: FIXTURE_SIZE as f32,
        height: FIXTURE_SIZE as f32,
        nodes: vec![background(children)],
        images: procedural_images(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DisplayList;

    #[test]
    fn every_fixture_builds_without_text() {
        for n in 1..=FIXTURE_COUNT {
            let list = DisplayList::build(&fixture(n));
            assert!(list.strings.is_empty(), "fixture {n} must not contain text");
        }
    }
}
