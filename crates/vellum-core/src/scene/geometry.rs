//! Convex box geometry, kept in f64 model coordinates. No renderer or vendor dependency.
use crate::ports::Rect;
use vellum_types::Point;
pub(crate) type Polygon = Vec<Point>;
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Affine(pub [f64; 6]);
impl Affine {
    pub const IDENTITY: Self = Self([1., 0., 0., 1., 0., 0.]);
    pub fn point(self, p: Point) -> Point {
        let [a, b, c, d, e, f] = self.0;
        Point {
            x: a * p.x + c * p.y + e,
            y: b * p.x + d * p.y + f,
        }
    }
    pub fn then(self, r: Self) -> Self {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = r.0;
        Self([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ])
    }
    /// The transform that undoes this one; None when it squashes the plane flat.
    pub fn invert(self) -> Option<Self> {
        let [a, b, c, d, e, f] = self.0;
        let det = a * d - b * c;
        if det == 0. || !det.is_finite() {
            return None;
        }
        Some(Self([
            d / det,
            -b / det,
            -c / det,
            a / det,
            (c * f - d * e) / det,
            (b * e - a * f) / det,
        ]))
    }
    pub fn local(r: Rect, rotation: f64, flip_x: bool, flip_y: bool) -> Self {
        let (sin, cos) = rotation.to_radians().sin_cos();
        let sx = if flip_x { -1. } else { 1. };
        let sy = if flip_y { -1. } else { 1. };
        let (a, b, c, d) = (cos * sx, sin * sx, -sin * sy, cos * sy);
        let (cx, cy) = (r.width / 2., r.height / 2.);
        Self([
            a,
            b,
            c,
            d,
            r.x + cx - a * cx - c * cy,
            r.y + cy - b * cx - d * cy,
        ])
    }
}
pub(crate) fn corners(r: Rect) -> Polygon {
    vec![
        Point { x: r.x, y: r.y },
        Point {
            x: r.right(),
            y: r.y,
        },
        Point {
            x: r.right(),
            y: r.bottom(),
        },
        Point {
            x: r.x,
            y: r.bottom(),
        },
    ]
}
pub(crate) fn bounds(p: &[Point]) -> Option<Rect> {
    let first = p.first()?;
    let (mut x, mut y, mut right, mut bottom) = (first.x, first.y, first.x, first.y);
    for p in p {
        x = x.min(p.x);
        y = y.min(p.y);
        right = right.max(p.x);
        bottom = bottom.max(p.y);
    }
    Some(Rect::new(x, y, right - x, bottom - y))
}
fn cross(a: Point, b: Point, p: Point) -> f64 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}
fn area(p: &[Point]) -> f64 {
    let Some(origin) = p.first() else {
        return 0.;
    };
    p.windows(2).map(|e| cross(*origin, e[0], e[1])).sum()
}
/// Rounded clipping polygon. Each circular arc uses segments with at most 0.01 px sagitta
/// (capped at 128 segments per corner for pathological radii). CSS's shared radius reduction applies.
pub(crate) fn rounded(w: f64, h: f64, mut radii: [f64; 4]) -> Polygon {
    let largest = radii.iter().copied().fold(0.0_f64, f64::max);
    if largest == 0. {
        return corners(Rect::new(0., 0., w, h));
    }
    // Normalize before summing: two individually finite radii can overflow when added.
    for r in &mut radii {
        *r /= largest;
    }
    let mut scale = largest;
    for (size, sum) in [
        (w, radii[0] + radii[1]),
        (w, radii[2] + radii[3]),
        (h, radii[0] + radii[3]),
        (h, radii[1] + radii[2]),
    ] {
        if sum > 0. {
            scale = scale.min(size / sum);
        }
    }
    for r in &mut radii {
        *r *= scale;
    }
    let mut out = vec![];
    for (i, r) in radii.into_iter().enumerate() {
        let (x, y) = match i {
            0 => (r, r),
            1 => (w - r, r),
            2 => (w - r, h - r),
            _ => (r, h - r),
        };
        let steps = if r <= 0.01 {
            1
        } else {
            ((std::f64::consts::FRAC_PI_2 / (2. * (1. - 0.01 / r).acos())).ceil() as usize)
                .clamp(1, 128)
        };
        for n in 0..=steps {
            let angle = (180. + i as f64 * 90. + n as f64 * 90. / steps as f64).to_radians();
            out.push(Point {
                x: x + angle.cos() * r,
                y: y + angle.sin() * r,
            });
        }
    }
    out
}
pub(crate) fn contains(poly: &[Point], p: Point) -> bool {
    if poly.is_empty() {
        return false;
    }
    if area(poly) == 0. {
        return poly
            .iter()
            .zip(poly.iter().cycle().skip(1))
            .take(poly.len())
            .any(|(a, b)| {
                cross(*a, *b, p).abs() < 1e-9
                    && p.x >= a.x.min(b.x) - 1e-9
                    && p.x <= a.x.max(b.x) + 1e-9
                    && p.y >= a.y.min(b.y) - 1e-9
                    && p.y <= a.y.max(b.y) + 1e-9
            });
    }
    let sign = area(poly).signum();
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .all(|(a, b)| cross(*a, *b, p) * sign >= -1e-9)
}
/// Intersection against a nondegenerate convex clipping box. Degenerate clips display no descendant content.
pub(crate) fn clip(poly: &[Point], clip: &[Point]) -> Polygon {
    if clip.is_empty() {
        return vec![];
    }
    let sign = area(clip).signum();
    if area(clip) == 0. {
        // Frame clips with no area cannot display content. The frame itself is still selectable.
        return vec![];
    }
    let mut out = poly.to_vec();
    for (a, b) in clip
        .iter()
        .zip(clip.iter().cycle().skip(1))
        .take(clip.len())
    {
        let input = std::mem::take(&mut out);
        let Some(mut prev) = input.last().copied() else {
            break;
        };
        let mut pd = cross(*a, *b, prev) * sign;
        for next in input {
            let nd = cross(*a, *b, next) * sign;
            if (pd >= -1e-9) != (nd >= -1e-9) {
                let t = pd / (pd - nd);
                out.push(Point {
                    x: prev.x + (next.x - prev.x) * t,
                    y: prev.y + (next.y - prev.y) * t,
                });
            }
            if nd >= -1e-9 {
                out.push(next);
            }
            prev = next;
            pd = nd;
        }
    }
    out
}
pub(crate) fn intersects_rect(poly: &[Point], r: Rect) -> bool {
    // Separating axes of both the selection rectangle and each edge of the clipped box.
    if poly.is_empty() {
        return false;
    }
    let other = corners(r);
    let axes = std::iter::once((1., 0.))
        .chain(std::iter::once((0., 1.)))
        .chain(
            poly.iter()
                .zip(poly.iter().cycle().skip(1))
                .take(poly.len())
                .map(|(a, b)| (a.y - b.y, b.x - a.x)),
        );
    for (x, y) in axes {
        if x == 0. && y == 0. {
            continue;
        }
        let range = |ps: &[Point]| {
            ps.iter()
                .map(|p| p.x * x + p.y * y)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v), hi.max(v))
                })
        };
        let (a, b) = range(poly);
        let (c, d) = range(&other);
        if b < c - 1e-9 || d < a - 1e-9 {
            return false;
        }
    }
    true
}
