//! Rectangles and extents shared by the ports.

/// An axis-aligned rectangle in model coordinates. The width and height are never negative.
///
/// Edges count as inside: two rectangles that only touch intersect, and a point on an edge is contained. A
/// rectangle with no width or height (a horizontal line, say) is valid and can be found.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Rect {
    /// A rectangle from its top-left corner and size.
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Finite coordinates, nonnegative sizes and finite far edges.
    pub fn is_valid(&self) -> bool {
        [
            self.x,
            self.y,
            self.width,
            self.height,
            self.right(),
            self.bottom(),
        ]
        .iter()
        .all(|v| v.is_finite())
            && self.width >= 0.0
            && self.height >= 0.0
    }

    /// The right edge.
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// The bottom edge.
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Whether the rectangles share any point, edges included.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x <= other.right()
            && other.x <= self.right()
            && self.y <= other.bottom()
            && other.y <= self.bottom()
    }

    /// Whether `(x, y)` is inside or on an edge.
    pub fn contains_point(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// Whether `other` lies inside this rectangle, edges included, give or take `tolerance`.
    pub fn contains_rect(&self, other: &Rect, tolerance: f64) -> bool {
        other.x >= self.x - tolerance
            && other.y >= self.y - tolerance
            && other.right() <= self.right() + tolerance
            && other.bottom() <= self.bottom() + tolerance
    }

    /// The smallest rectangle holding both.
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }

    /// This rectangle grown by `by` on every side (shrunk when negative, never below nothing).
    pub fn expand(&self, by: f64) -> Rect {
        let width = (self.width + 2.0 * by).max(0.0);
        let height = (self.height + 2.0 * by).max(0.0);
        Rect::new(
            self.x + (self.width - width) / 2.0,
            self.y + (self.height - height) / 2.0,
            width,
            height,
        )
    }
}

/// A width and a height. `f64::INFINITY` means unbounded on that axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Extent {
    /// No limit in either direction.
    pub const UNBOUNDED: Extent = Extent {
        width: f64::INFINITY,
        height: f64::INFINITY,
    };

    /// An extent from a width and a height.
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// Limited in width only.
    pub const fn with_width(width: f64) -> Self {
        Self {
            width,
            height: f64::INFINITY,
        }
    }
}
