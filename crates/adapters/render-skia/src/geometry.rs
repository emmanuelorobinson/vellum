use crate::{Error, matrix};
use skia_safe::{Path, PathBuilder, PathFillType, PathOp, Point, RRect, Rect};
use vellum_types::{BoolOp, FillRule, display::Shape};

pub(crate) fn shape(s: &Shape) -> Result<Path, Error> {
    let path = match s {
        Shape::Rect {
            size: [w, h],
            radii,
        } => {
            size(*w, *h)?;
            if radii.iter().any(|r| *r < 0.) {
                return Err(Error::Geometry);
            }
            Path::rrect(
                RRect::new_rect_radii(Rect::from_wh(*w, *h), &radii.map(|r| Point::new(r, r))),
                None,
            )
        }
        Shape::Ellipse {
            size: [w, h],
            inner,
            start,
            sweep,
        } => {
            size(*w, *h)?;
            if !(0. ..=1.).contains(inner) || sweep.abs() > 360. {
                return Err(Error::Geometry);
            }
            let r = Rect::from_wh(*w, *h);
            if sweep.abs() == 360. {
                let outer = Path::oval(r, None);
                if *inner == 0. {
                    outer
                } else {
                    let ri = Rect::from_xywh(
                        w * (1. - inner) / 2.,
                        h * (1. - inner) / 2.,
                        w * inner,
                        h * inner,
                    );
                    outer
                        .op(&Path::oval(ri, None), PathOp::Difference)
                        .ok_or(Error::Geometry)?
                }
            } else {
                let mut b = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
                b.arc_to(r, *start, *sweep, true);
                if *inner > 0. {
                    let ri = Rect::from_xywh(
                        w * (1. - inner) / 2.,
                        h * (1. - inner) / 2.,
                        w * inner,
                        h * inner,
                    );
                    b.arc_to(ri, start + sweep, -sweep, false);
                } else if sweep.abs() != 360. {
                    b.line_to((*w / 2., *h / 2.));
                }
                b.close();
                b.detach()
            }
        }
        Shape::Polygon {
            size: [w, h],
            sides,
            radius,
        } => {
            size(*w, *h)?;
            if !(3..=4096).contains(sides) || *radius < 0. {
                return Err(Error::Geometry);
            }
            let p = polygon(*w, *h, *sides, None);
            if *radius == 0. {
                p
            } else {
                let effect = skia_safe::PathEffect::corner_path(*radius).ok_or(Error::Geometry)?;
                let mut paint = skia_safe::Paint::default();
                paint.set_path_effect(effect);
                crate::outline(&p, &paint)?
            }
        }
        Shape::Star {
            size: [w, h],
            points,
            inner,
        } => {
            size(*w, *h)?;
            if !(2..=4096).contains(points) || !(0. ..=1.).contains(inner) {
                return Err(Error::Geometry);
            }
            polygon(*w, *h, *points, Some(*inner))
        }
        Shape::Line { width } => {
            size(*width, 0.)?;
            Path::line((0., 0.), (*width, 0.))
        }
        Shape::Vector {
            data,
            view_box: [x, y, w, h],
            size: [dw, dh],
            fill_rule,
        } => {
            size(*dw, *dh)?;
            if *w <= 0. || *h <= 0. {
                return Err(Error::Geometry);
            }
            let mut p = Path::from_svg(data).ok_or(Error::Geometry)?;
            p.set_fill_type(match fill_rule {
                FillRule::Nonzero => PathFillType::Winding,
                FillRule::Evenodd => PathFillType::EvenOdd,
            });
            p.with_transform(&matrix([dw / w, 0., 0., dh / h, -x * dw / w, -y * dh / h]))
        }
        Shape::Boolean { op, operands } => {
            let mut paths = operands
                .iter()
                .map(|o| Ok(shape(&o.shape)?.with_transform(&matrix(o.transform))));
            let mut out = paths.next().transpose()?.unwrap_or_default();
            for p in paths {
                out = out
                    .op(
                        &p?,
                        match op {
                            BoolOp::Union => PathOp::Union,
                            BoolOp::Subtract => PathOp::Difference,
                            BoolOp::Intersect => PathOp::Intersect,
                            BoolOp::Exclude => PathOp::XOR,
                        },
                    )
                    .ok_or(Error::Geometry)?;
            }
            out
        }
    };
    if !path.is_finite() {
        return Err(Error::Geometry);
    }
    Ok(path)
}
fn size(w: f32, h: f32) -> Result<(), Error> {
    if w < 0. || h < 0. || !w.is_finite() || !h.is_finite() {
        Err(Error::Geometry)
    } else {
        Ok(())
    }
}
fn polygon(w: f32, h: f32, n: u32, inner: Option<f32>) -> Path {
    let count = if inner.is_some() { n * 2 } else { n };
    let points: Vec<_> = (0..count)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / count as f32 - std::f32::consts::FRAC_PI_2;
            let r = if i % 2 == 1 { inner.unwrap_or(1.) } else { 1. };
            Point::new(w / 2. + a.cos() * w / 2. * r, h / 2. + a.sin() * h / 2. * r)
        })
        .collect();
    Path::polygon(&points, true, None, None)
}
