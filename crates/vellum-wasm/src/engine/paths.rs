//! Editable paths for the editor's path tool (`@vellum/engine-client` `EditablePath`): cubic contours in the
//! node's viewBox coordinates. A vector's SVG path data is read here; rectangles, ellipses and lines give the
//! outline a person starts from when they edit a shape's points.
use serde_json::{Value, json};
use vellum_types::{Node, Point, Size, Value as Prop};

#[derive(Debug, Clone, PartialEq)]
pub struct PathPoint {
    pub point: Point,
    pub r#in: Option<Point>,
    pub out: Option<Point>,
    pub smooth: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Contour {
    pub points: Vec<PathPoint>,
    pub closed: bool,
}

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn same(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

/// Whether the handles around a point line up (a smooth point), allowing for rounding.
fn collinear(a: Point, at: Point, b: Point) -> bool {
    let (ax, ay, bx, by) = (at.x - a.x, at.y - a.y, b.x - at.x, b.y - at.y);
    let cross = ax * by - ay * bx;
    let scale = (ax.hypot(ay) * bx.hypot(by)).max(1e-12);
    (cross / scale).abs() < 1e-3 && ax * bx + ay * by > 0.
}

fn finish(points: &mut [PathPoint]) {
    for pt in points {
        pt.smooth = matches!((pt.r#in, pt.out), (Some(a), Some(b)) if collinear(a, pt.point, b));
    }
}

/// SVG path data as contours. Arcs (`A`) have no cubic points here yet, so a path with one is not editable.
pub fn parse(d: &str) -> Option<Vec<Contour>> {
    let mut tokens = Vec::new();
    let bytes = d.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            tokens.push(Err(c));
            i += 1;
        } else if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' {
            let start = i;
            i += 1;
            let mut dot = c == '.';
            while i < bytes.len() {
                let n = bytes[i] as char;
                if n.is_ascii_digit() {
                    i += 1;
                } else if n == '.' && !dot {
                    dot = true;
                    i += 1;
                } else if (n == 'e' || n == 'E')
                    && bytes
                        .get(i + 1)
                        .is_some_and(|b| b.is_ascii_digit() || *b == b'-' || *b == b'+')
                {
                    i += 2;
                } else {
                    break;
                }
            }
            tokens.push(Ok(d[start..i].parse::<f64>().ok()?));
        } else {
            i += 1;
        }
    }
    let mut contours: Vec<Contour> = Vec::new();
    let mut current = Contour::default();
    let (mut at, mut start) = (p(0., 0.), p(0., 0.));
    let mut last_control: Option<(char, Point)> = None;
    let mut command = 'M';
    let mut k = 0;
    let push_contour = |current: &mut Contour, contours: &mut Vec<Contour>| {
        if !current.points.is_empty() {
            finish(&mut current.points);
            contours.push(std::mem::take(current));
        }
    };
    while k < tokens.len() {
        if let Err(c) = tokens[k] {
            command = c;
            k += 1;
            if matches!(c, 'Z' | 'z') {
                if let (Some(first), Some(last)) =
                    (current.points.first().cloned(), current.points.last())
                    && current.points.len() > 1
                    && same(first.point, last.point)
                {
                    let last = current.points.pop().expect("a last point");
                    current.points[0].r#in = last.r#in;
                }
                current.closed = true;
                at = start;
                push_contour(&mut current, &mut contours);
                last_control = None;
                continue;
            }
        }
        let rel = command.is_ascii_lowercase();
        let base = if rel { at } else { p(0., 0.) };
        let mut num = |n: usize| -> Option<Vec<f64>> {
            let out = tokens
                .get(k..k + n)?
                .iter()
                .map(|t| t.ok())
                .collect::<Option<Vec<_>>>()?;
            k += n;
            Some(out)
        };
        match command.to_ascii_uppercase() {
            'M' => {
                let v = num(2)?;
                push_contour(&mut current, &mut contours);
                at = p(base.x + v[0], base.y + v[1]);
                start = at;
                current.points.push(PathPoint {
                    point: at,
                    r#in: None,
                    out: None,
                    smooth: false,
                });
                // Further pairs after a move are lines.
                command = if rel { 'l' } else { 'L' };
                last_control = None;
            }
            c @ ('L' | 'H' | 'V') => {
                at = match c {
                    'L' => {
                        let v = num(2)?;
                        p(base.x + v[0], base.y + v[1])
                    }
                    'H' => p(num(1)?[0] + if rel { at.x } else { 0. }, at.y),
                    _ => p(at.x, num(1)?[0] + if rel { at.y } else { 0. }),
                };
                current.points.push(PathPoint {
                    point: at,
                    r#in: None,
                    out: None,
                    smooth: false,
                });
                last_control = None;
            }
            c @ ('C' | 'S' | 'Q' | 'T') => {
                let (c1, c2, end, control) = match c {
                    'C' => {
                        let v = num(6)?;
                        let c2 = p(base.x + v[2], base.y + v[3]);
                        (
                            p(base.x + v[0], base.y + v[1]),
                            c2,
                            p(base.x + v[4], base.y + v[5]),
                            ('C', c2),
                        )
                    }
                    'S' => {
                        let v = num(4)?;
                        let c1 = match last_control {
                            Some(('C', c)) => p(2. * at.x - c.x, 2. * at.y - c.y),
                            _ => at,
                        };
                        let c2 = p(base.x + v[0], base.y + v[1]);
                        (c1, c2, p(base.x + v[2], base.y + v[3]), ('C', c2))
                    }
                    _ => {
                        let (q, end) = if c == 'Q' {
                            let v = num(4)?;
                            (
                                p(base.x + v[0], base.y + v[1]),
                                p(base.x + v[2], base.y + v[3]),
                            )
                        } else {
                            let v = num(2)?;
                            let q = match last_control {
                                Some(('Q', c)) => p(2. * at.x - c.x, 2. * at.y - c.y),
                                _ => at,
                            };
                            (q, p(base.x + v[0], base.y + v[1]))
                        };
                        // A quadratic as the cubic that draws it.
                        (
                            p(at.x + 2. / 3. * (q.x - at.x), at.y + 2. / 3. * (q.y - at.y)),
                            p(
                                end.x + 2. / 3. * (q.x - end.x),
                                end.y + 2. / 3. * (q.y - end.y),
                            ),
                            end,
                            ('Q', q),
                        )
                    }
                };
                current.points.last_mut()?.out = Some(c1);
                current.points.push(PathPoint {
                    point: end,
                    r#in: Some(c2),
                    out: None,
                    smooth: false,
                });
                at = end;
                last_control = Some(control);
            }
            _ => return None,
        }
    }
    push_contour(&mut current, &mut contours);
    Some(contours)
}

fn literal(size: &Size) -> Option<f64> {
    match size {
        Size::Fixed(Prop::Literal(n)) => Some(*n),
        _ => None,
    }
}

/// The editable outline of a node, with its viewBox, in the JSON the editor reads. None for kinds whose points
/// cannot be edited yet.
pub fn editable(node: &Node, width: f64, height: f64) -> Option<Value> {
    let (view_box, contours) = match node {
        Node::Vector(v) => {
            let b = &v.view_box;
            (
                json!({"x": b.x, "y": b.y, "width": b.width, "height": b.height}),
                parse(&v.path)?,
            )
        }
        Node::Rect(_) | Node::Ellipse(_) | Node::Line(_) => {
            let (w, h) = (
                literal(node.width()).unwrap_or(width),
                literal(node.height()).unwrap_or(height),
            );
            let d = match node {
                Node::Rect(_) => format!("M0 0H{w}V{h}H0Z"),
                Node::Line(_) => format!("M0 0L{w} 0"),
                _ => {
                    // Four cubic quarter arcs; k is the usual circle constant.
                    let (rx, ry, k) = (w / 2., h / 2., 0.552_284_749_830_793_4);
                    let (cx, cy) = (rx, ry);
                    format!(
                        "M{} {cy}C{} {} {} {} {cx} {}C{} {} {} {} {} {cy}C{} {} {} {} {cx} {}C{} {} {} {} {} {cy}Z",
                        cx + rx,
                        cx + rx,
                        cy + k * ry,
                        cx + k * rx,
                        cy + ry,
                        cy + ry,
                        cx - k * rx,
                        cy + ry,
                        cx - rx,
                        cy + k * ry,
                        cx - rx,
                        cx - rx,
                        cy - k * ry,
                        cx - k * rx,
                        cy - ry,
                        cy - ry,
                        cx + k * rx,
                        cy - ry,
                        cx + rx,
                        cy - k * ry,
                        cx + rx,
                    )
                }
            };
            (
                json!({"x": 0., "y": 0., "width": w, "height": h}),
                parse(&d)?,
            )
        }
        _ => return None,
    };
    let pt = |q: Point| json!({"x": q.x, "y": q.y});
    Some(json!({
        "viewBox": view_box,
        "contours": contours.iter().map(|c| json!({
            "closed": c.closed,
            "points": c.points.iter().map(|q| json!({
                "point": pt(q.point),
                "in": q.r#in.map(pt),
                "out": q.out.map(pt),
                "smooth": q.smooth,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_curves_and_closing_read_as_cubic_points() {
        let c = parse("M0 0 L10 0 C10 5 5 10 0 10 Z").unwrap();
        assert_eq!(c.len(), 1);
        assert!(c[0].closed);
        assert_eq!(c[0].points.len(), 3);
        assert_eq!(c[0].points[1].out, Some(p(10., 5.)));
        assert_eq!(c[0].points[2].r#in, Some(p(5., 10.)));
        let relative = parse("m1 1 h4 v4 h-4 z").unwrap();
        assert_eq!(relative[0].points[2].point, p(5., 5.));
        // A curve that comes back to the start merges with it: the first point gets the incoming handle.
        let round = parse("M0 0C5 0 5 10 0 10C-5 10 -5 0 0 0Z").unwrap();
        assert_eq!(round[0].points.len(), 2);
        assert_eq!(round[0].points[0].r#in, Some(p(-5., 0.)));
        assert!(
            round[0].points[0].smooth,
            "handles in line make a smooth point"
        );
        let quad = parse("M0 0Q3 3 6 0").unwrap();
        assert_eq!(quad[0].points[0].out, Some(p(2., 2.)));
        assert!(
            parse("M0 0A5 5 0 0 1 10 0").is_none(),
            "arcs are not editable yet"
        );
        assert_eq!(parse("M0 0L1e1 2").unwrap()[0].points[1].point, p(10., 2.));
    }
}
