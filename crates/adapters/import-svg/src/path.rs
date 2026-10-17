//! SVG path data: parsed to absolute moves, lines, cubics and closes (quadratics and arcs become cubics), then
//! transformed, measured and written back.
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    M(f64, f64),
    L(f64, f64),
    C(f64, f64, f64, f64, f64, f64),
    Z,
}

/// `[a b c d e f]`: x' = a x + c y + e, y' = b x + d y + f.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine(pub [f64; 6]);

impl Affine {
    pub const IDENTITY: Affine = Affine([1., 0., 0., 1., 0., 0.]);

    pub fn translate(x: f64, y: f64) -> Self {
        Self([1., 0., 0., 1., x, y])
    }
    pub fn scale(x: f64, y: f64) -> Self {
        Self([x, 0., 0., y, 0., 0.])
    }
    /// `self` applied after `other`: (self * other)(p) = self(other(p)).
    pub fn then_after(self, other: Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = other.0;
        Affine([
            a * a2 + c * b2,
            b * a2 + d * b2,
            a * c2 + c * d2,
            b * c2 + d * d2,
            a * e2 + c * f2 + e,
            b * e2 + d * f2 + f,
        ])
    }
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }
    /// The factor lengths are scaled by on average: the square root of the area factor.
    pub fn mean_scale(&self) -> f64 {
        let [a, b, c, d, ..] = self.0;
        (a * d - b * c).abs().sqrt()
    }
    /// No rotation or skew: axis-aligned shapes stay axis-aligned.
    pub fn is_axis_aligned(&self) -> bool {
        self.0[1].abs() < 1e-9 && self.0[2].abs() < 1e-9
    }
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|v| v.is_finite())
    }
}

/// A `transform` attribute. Unknown functions make the whole attribute unreadable (`None`).
pub fn parse_transform(text: &str) -> Option<Affine> {
    let mut total = Affine::IDENTITY;
    let mut rest = text.trim();
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let close = rest.find(')')?;
        if close < open {
            return None;
        }
        let name = rest[..open].trim();
        let args: Vec<f64> = rest[open + 1..close]
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<f64>().ok())
            .collect::<Option<_>>()?;
        let m = match (name, args.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => Affine([*a, *b, *c, *d, *e, *f]),
            ("translate", [x]) => Affine::translate(*x, 0.),
            ("translate", [x, y]) => Affine::translate(*x, *y),
            ("scale", [s]) => Affine::scale(*s, *s),
            ("scale", [x, y]) => Affine::scale(*x, *y),
            ("rotate", [a]) => rotation(*a),
            ("rotate", [a, cx, cy]) => Affine::translate(*cx, *cy)
                .then_after(rotation(*a))
                .then_after(Affine::translate(-cx, -cy)),
            ("skewX", [a]) => Affine([1., 0., a.to_radians().tan(), 1., 0., 0.]),
            ("skewY", [a]) => Affine([1., a.to_radians().tan(), 0., 1., 0., 0.]),
            _ => return None,
        };
        // Later functions apply first: the list is read left to right as nested transforms.
        total = total.then_after(m);
        rest = rest[close + 1..].trim_start_matches(|c: char| c == ',' || c.is_whitespace());
    }
    Some(total).filter(Affine::is_finite)
}

fn rotation(degrees: f64) -> Affine {
    let (s, c) = degrees.to_radians().sin_cos();
    Affine([c, s, -s, c, 0., 0.])
}

struct Scanner<'a> {
    b: &'a [u8],
    at: usize,
}

impl Scanner<'_> {
    fn skip(&mut self) {
        while self.at < self.b.len()
            && (self.b[self.at].is_ascii_whitespace() || self.b[self.at] == b',')
        {
            self.at += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.b.get(self.at).copied()
    }
    fn number(&mut self) -> Option<f64> {
        self.skip();
        let start = self.at;
        let mut i = self.at;
        if matches!(self.b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let digits = |i: &mut usize| {
            let from = *i;
            while self.b.get(*i).is_some_and(u8::is_ascii_digit) {
                *i += 1;
            }
            *i > from
        };
        let int = digits(&mut i);
        let mut frac = false;
        if self.b.get(i) == Some(&b'.') {
            i += 1;
            frac = digits(&mut i);
        }
        if !int && !frac {
            return None;
        }
        if matches!(self.b.get(i), Some(b'e' | b'E')) {
            let mut j = i + 1;
            if matches!(self.b.get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            if digits(&mut j) {
                i = j;
            }
        }
        self.at = i;
        std::str::from_utf8(&self.b[start..i])
            .ok()?
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
    }
    fn flag(&mut self) -> Option<bool> {
        self.skip();
        let v = match self.b.get(self.at)? {
            b'0' => false,
            b'1' => true,
            _ => return None,
        };
        self.at += 1;
        Some(v)
    }
}

/// A control point remembered for the smooth commands that reflect it.
type Ctrl = Option<(f64, f64)>;

/// Parses path data. SVG draws a path up to its first error, so the segments read so far come back with the
/// error that stopped the parse, if any.
pub fn parse(d: &str) -> (Vec<Seg>, Option<String>) {
    let mut s = Scanner {
        b: d.as_bytes(),
        at: 0,
    };
    let mut out = Vec::new();
    let (mut x, mut y, mut sx, mut sy) = (0., 0., 0., 0.);
    let (mut last_cubic, mut last_quad): (Ctrl, Ctrl) = (None, None);
    let mut cmd = 0u8;
    let mut first = true;
    let err = loop {
        let Some(next) = s.peek() else { break None };
        if next.is_ascii_alphabetic() {
            cmd = next;
            s.at += 1;
            if !b"MmLlHhVvCcSsQqTtAaZz".contains(&cmd) {
                break Some(format!("Unknown path command {:?}.", cmd as char));
            }
        } else if cmd == 0 || cmd == b'Z' || cmd == b'z' {
            break Some("Path data has numbers where a command should be.".into());
        }
        if first && !matches!(cmd, b'M' | b'm') {
            break Some("Path data must start with a move.".into());
        }
        first = false;
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0., 0.) };
        macro_rules! num {
            () => {
                match s.number() {
                    Some(v) => v,
                    None => break Some("Path data ends in the middle of a command.".into()),
                }
            };
        }
        let mut cubic_ctrl = None;
        let mut quad_ctrl = None;
        match cmd.to_ascii_uppercase() {
            b'M' => {
                let (nx, ny) = (ox + num!(), oy + num!());
                out.push(Seg::M(nx, ny));
                (x, y, sx, sy) = (nx, ny, nx, ny);
                // Further pairs are implicit lines.
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => {
                let (nx, ny) = (ox + num!(), oy + num!());
                out.push(Seg::L(nx, ny));
                (x, y) = (nx, ny);
            }
            b'H' => {
                let nx = if rel { x + num!() } else { num!() };
                out.push(Seg::L(nx, y));
                x = nx;
            }
            b'V' => {
                let ny = if rel { y + num!() } else { num!() };
                out.push(Seg::L(x, ny));
                y = ny;
            }
            b'C' => {
                let (x1, y1, x2, y2, nx, ny) = (
                    ox + num!(),
                    oy + num!(),
                    ox + num!(),
                    oy + num!(),
                    ox + num!(),
                    oy + num!(),
                );
                out.push(Seg::C(x1, y1, x2, y2, nx, ny));
                cubic_ctrl = Some((x2, y2));
                (x, y) = (nx, ny);
            }
            b'S' => {
                let (x2, y2, nx, ny) = (ox + num!(), oy + num!(), ox + num!(), oy + num!());
                let (x1, y1) = last_cubic.map_or((x, y), |(cx, cy)| (2. * x - cx, 2. * y - cy));
                out.push(Seg::C(x1, y1, x2, y2, nx, ny));
                cubic_ctrl = Some((x2, y2));
                (x, y) = (nx, ny);
            }
            b'Q' => {
                let (qx, qy, nx, ny) = (ox + num!(), oy + num!(), ox + num!(), oy + num!());
                out.push(quad(x, y, qx, qy, nx, ny));
                quad_ctrl = Some((qx, qy));
                (x, y) = (nx, ny);
            }
            b'T' => {
                let (nx, ny) = (ox + num!(), oy + num!());
                let (qx, qy) = last_quad.map_or((x, y), |(cx, cy)| (2. * x - cx, 2. * y - cy));
                out.push(quad(x, y, qx, qy, nx, ny));
                quad_ctrl = Some((qx, qy));
                (x, y) = (nx, ny);
            }
            b'A' => {
                let (rx, ry, rot) = (num!(), num!(), num!());
                let (Some(large), Some(sweep)) = (s.flag(), s.flag()) else {
                    break Some("An arc has a flag that is not 0 or 1.".into());
                };
                let (nx, ny) = (ox + num!(), oy + num!());
                arc(&mut out, x, y, rx, ry, rot, large, sweep, nx, ny);
                (x, y) = (nx, ny);
            }
            _ => {
                out.push(Seg::Z);
                (x, y) = (sx, sy);
            }
        }
        last_cubic = cubic_ctrl;
        last_quad = quad_ctrl;
    };
    (out, err)
}

fn quad(x0: f64, y0: f64, qx: f64, qy: f64, x: f64, y: f64) -> Seg {
    Seg::C(
        x0 + 2. / 3. * (qx - x0),
        y0 + 2. / 3. * (qy - y0),
        x + 2. / 3. * (qx - x),
        y + 2. / 3. * (qy - y),
        x,
        y,
    )
}

/// Endpoint arc to cubic Béziers (SVG 1.1 implementation notes F.6), one cubic per quarter turn at most.
#[allow(clippy::too_many_arguments)]
pub fn arc(
    out: &mut Vec<Seg>,
    x0: f64,
    y0: f64,
    rx: f64,
    ry: f64,
    rot: f64,
    large: bool,
    sweep: bool,
    x: f64,
    y: f64,
) {
    if (x0 - x).abs() < 1e-12 && (y0 - y).abs() < 1e-12 {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-12 || ry < 1e-12 {
        out.push(Seg::L(x, y));
        return;
    }
    let (sin, cos) = rot.to_radians().sin_cos();
    let (dx, dy) = ((x0 - x) / 2., (y0 - y) / 2.);
    let (x1p, y1p) = (cos * dx + sin * dy, -sin * dx + cos * dy);
    let lambda = (x1p / rx).powi(2) + (y1p / ry).powi(2);
    if lambda > 1. {
        let root = lambda.sqrt();
        rx *= root;
        ry *= root;
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut k = if den == 0. {
        0.
    } else {
        (num / den).max(0.).sqrt()
    };
    if large == sweep {
        k = -k;
    }
    let (cxp, cyp) = (k * rx * y1p / ry, -k * ry * x1p / rx);
    let (cx, cy) = (
        cos * cxp - sin * cyp + (x0 + x) / 2.,
        sin * cxp + cos * cyp + (y0 + y) / 2.,
    );
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let theta = angle(1., 0., (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut delta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if !sweep && delta > 0. {
        delta -= 2. * PI;
    } else if sweep && delta < 0. {
        delta += 2. * PI;
    }
    let parts = (delta.abs() / (PI / 2.) - 1e-9).ceil().max(1.) as usize;
    let step = delta / parts as f64;
    let t = 4. / 3. * (step / 4.).tan();
    let point = |a: f64| {
        (
            cx + rx * a.cos() * cos - ry * a.sin() * sin,
            cy + rx * a.cos() * sin + ry * a.sin() * cos,
        )
    };
    let slope = |a: f64| {
        (
            -rx * a.sin() * cos - ry * a.cos() * sin,
            -rx * a.sin() * sin + ry * a.cos() * cos,
        )
    };
    for i in 0..parts {
        let (a0, a1) = (theta + step * i as f64, theta + step * (i + 1) as f64);
        let (p0, p1) = (point(a0), if i + 1 == parts { (x, y) } else { point(a1) });
        let (d0, d1) = (slope(a0), slope(a1));
        out.push(Seg::C(
            p0.0 + t * d0.0,
            p0.1 + t * d0.1,
            p1.0 - t * d1.0,
            p1.1 - t * d1.1,
            p1.0,
            p1.1,
        ));
    }
}

pub fn transform(segs: &[Seg], m: &Affine) -> Vec<Seg> {
    segs.iter()
        .map(|s| match *s {
            Seg::M(x, y) => {
                let (x, y) = m.apply(x, y);
                Seg::M(x, y)
            }
            Seg::L(x, y) => {
                let (x, y) = m.apply(x, y);
                Seg::L(x, y)
            }
            Seg::C(a, b, c, d, e, f) => {
                let (a, b) = m.apply(a, b);
                let (c, d) = m.apply(c, d);
                let (e, f) = m.apply(e, f);
                Seg::C(a, b, c, d, e, f)
            }
            Seg::Z => Seg::Z,
        })
        .collect()
}

/// The box around the drawn curve: x, y, width, height. Curves are sampled, so it is close, not exact.
pub fn bounds(segs: &[Seg]) -> Option<[f64; 4]> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut add = |x: f64, y: f64| {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    };
    let (mut cx, mut cy) = (0., 0.);
    let mut any = false;
    for s in segs {
        match *s {
            Seg::M(x, y) | Seg::L(x, y) => {
                add(x, y);
                (cx, cy) = (x, y);
                any = true;
            }
            Seg::C(a, b, c, d, e, f) => {
                for i in 1..=16 {
                    let t = f64::from(i) / 16.;
                    let u = 1. - t;
                    add(
                        u * u * u * cx + 3. * u * u * t * a + 3. * u * t * t * c + t * t * t * e,
                        u * u * u * cy + 3. * u * u * t * b + 3. * u * t * t * d + t * t * t * f,
                    );
                }
                (cx, cy) = (e, f);
                any = true;
            }
            Seg::Z => {}
        }
    }
    any.then_some([x0, y0, x1 - x0, y1 - y0])
}

pub fn num(v: f64) -> String {
    let r = (v * 1000.).round() / 1000.;
    if r == 0. { "0".into() } else { format!("{r}") }
}

pub fn write(segs: &[Seg]) -> String {
    let mut out = String::new();
    for s in segs {
        if !out.is_empty() {
            out.push(' ');
        }
        match *s {
            Seg::M(x, y) => out.push_str(&format!("M{} {}", num(x), num(y))),
            Seg::L(x, y) => out.push_str(&format!("L{} {}", num(x), num(y))),
            Seg::C(a, b, c, d, e, f) => out.push_str(&format!(
                "C{} {} {} {} {} {}",
                num(a),
                num(b),
                num(c),
                num(d),
                num(e),
                num(f)
            )),
            Seg::Z => out.push('Z'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_commands_and_implicit_repeats_become_absolute() {
        let (segs, err) = parse("m1 2 3 4 h5 v-1 z");
        assert_eq!(err, None);
        assert_eq!(
            segs,
            vec![
                Seg::M(1., 2.),
                Seg::L(4., 6.),
                Seg::L(9., 6.),
                Seg::L(9., 5.),
                Seg::Z
            ]
        );
    }

    #[test]
    fn numbers_may_run_together() {
        let (segs, err) = parse("M10-5.5.5.5L1e1,2E-1");
        assert_eq!(err, None);
        assert_eq!(
            segs,
            vec![Seg::M(10., -5.5), Seg::L(0.5, 0.5), Seg::L(10., 0.2)]
        );
    }

    #[test]
    fn a_path_is_read_up_to_its_first_error() {
        let (segs, err) = parse("M0 0 L5 5 L7");
        assert_eq!(segs, vec![Seg::M(0., 0.), Seg::L(5., 5.)]);
        assert!(err.is_some());
        assert!(parse("L1 1").1.is_some(), "a path starts with a move");
        assert!(parse("M0 0 X1").1.is_some());
        assert!(parse("M0 0 A1 1 0 2 0 5 5").1.is_some(), "flags are 0 or 1");
    }

    #[test]
    fn smooth_curves_reflect_the_previous_control_point() {
        let (segs, _) = parse("M0 0 C0 10 10 10 10 0 S20 -10 20 0");
        assert_eq!(segs[2], Seg::C(10., -10., 20., -10., 20., 0.));
        let (segs, _) = parse("M0 0 Q5 10 10 0 T20 0");
        let Seg::C(_, _, _, _, x, y) = segs[2] else {
            panic!()
        };
        assert_eq!((x, y), (20., 0.));
    }

    #[test]
    fn a_half_circle_arc_is_two_cubics_ending_where_it_should() {
        let (segs, err) = parse("M0 0 A5 5 0 0 1 10 0");
        assert_eq!(err, None);
        assert_eq!(segs.len(), 3);
        let Seg::C(.., x, y) = segs[2] else { panic!() };
        assert_eq!((x, y), (10., 0.));
        let [bx, by, bw, bh] = bounds(&segs).unwrap();
        assert!(
            (bx - 0.).abs() < 1e-6 && (bw - 10.).abs() < 1e-6,
            "{bx} {bw}"
        );
        assert!(
            (by + 5.).abs() < 0.05 && (bh - 5.).abs() < 0.05,
            "{by} {bh}"
        );
    }

    #[test]
    fn transforms_nest_left_to_right() {
        let m = parse_transform("translate(10 20) scale(2)").unwrap();
        assert_eq!(m.apply(1., 1.), (12., 22.));
        let r = parse_transform("rotate(90 5 5)").unwrap();
        let (x, y) = r.apply(10., 5.);
        assert!((x - 5.).abs() < 1e-9 && (y - 10.).abs() < 1e-9);
        assert!(parse_transform("wobble(3)").is_none());
        assert!(parse_transform("scale(").is_none());
        assert_eq!(parse_transform("").unwrap(), Affine::IDENTITY);
    }

    #[test]
    fn written_paths_round_numbers_and_drop_negative_zero() {
        assert_eq!(write(&[Seg::M(-0.0004, 1.23456), Seg::Z]), "M0 1.235 Z");
    }
}
