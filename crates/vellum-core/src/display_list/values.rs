//! Resolved document appearance → renderer-only scalars; unresolved/nonfinite values are never guessed.
use super::Error;
use std::collections::BTreeMap;
use vellum_types::{display::*, *};
pub(super) fn f(n: f64) -> Result<f32, Error> {
    let v = n as f32;
    if v.is_finite() {
        Ok(if v == 0. { 0. } else { v })
    } else {
        Err(Error::InvalidNumber)
    }
}
pub(super) fn number(v: &Value<f64>) -> Result<f32, Error> {
    match v {
        Value::Literal(n) => f(*n),
        _ => Err(Error::UnresolvedValue),
    }
}
pub(super) fn color(v: &Value<Color>) -> Result<[u8; 4], Error> {
    match v {
        Value::Literal(c) => Ok([c.r, c.g, c.b, c.a]),
        _ => Err(Error::UnresolvedValue),
    }
}
pub(super) fn matrix(m: [f64; 6]) -> Result<[f32; 6], Error> {
    Ok([f(m[0])?, f(m[1])?, f(m[2])?, f(m[3])?, f(m[4])?, f(m[5])?])
}
pub(super) fn radii(r: &CornerRadius) -> Result<[f32; 4], Error> {
    Ok(match r {
        CornerRadius::All(v) => [number(v)?; 4],
        CornerRadius::Corners(v) => [
            number(&v[0])?,
            number(&v[1])?,
            number(&v[2])?,
            number(&v[3])?,
        ],
    })
}
pub(super) fn uniforms(
    v: &BTreeMap<String, Value<Uniform>>,
) -> Result<BTreeMap<String, RenderUniform>, Error> {
    v.iter()
        .map(|(k, v)| {
            let Value::Literal(v) = v else {
                return Err(Error::UnresolvedValue);
            };
            let value = match v {
                Uniform::Bool(b) => RenderUniform::Bool(*b),
                Uniform::Number(n) => RenderUniform::Number(f(*n)?),
                Uniform::Color(c) => RenderUniform::Color([c.r, c.g, c.b, c.a]),
                Uniform::Vec2(v) => {
                    RenderUniform::Vector(v.iter().map(|v| f(*v)).collect::<Result<_, _>>()?)
                }
                Uniform::Vec3(v) => {
                    RenderUniform::Vector(v.iter().map(|v| f(*v)).collect::<Result<_, _>>()?)
                }
                Uniform::Vec4(v) => {
                    RenderUniform::Vector(v.iter().map(|v| f(*v)).collect::<Result<_, _>>()?)
                }
            };
            Ok((k.clone(), value))
        })
        .collect()
}
pub(super) fn paint(p: &Paint) -> Result<RenderPaint, Error> {
    Ok(match p {
        Paint::Solid(p) => RenderPaint::Solid(color(&p.color)?),
        Paint::Gradient(p) => RenderPaint::Gradient {
            kind: p.kind,
            transform: matrix(p.transform.0)?,
            stops: p
                .stops
                .iter()
                .map(|s| {
                    Ok(RenderStop {
                        position: f(s.position)?,
                        color: color(&s.color)?,
                    })
                })
                .collect::<Result<_, Error>>()?,
        },
        Paint::Mesh(p) => RenderPaint::Mesh {
            columns: p.columns,
            rows: p.rows,
            points: p
                .points
                .iter()
                .map(|p| {
                    Ok(RenderMeshPoint {
                        position: [f(p.x)?, f(p.y)?],
                        color: color(&p.color)?,
                        handles: p
                            .handles
                            .map(|h| {
                                Ok::<_, Error>([
                                    [f(h.left.x)?, f(h.left.y)?],
                                    [f(h.right.x)?, f(h.right.y)?],
                                    [f(h.up.x)?, f(h.up.y)?],
                                    [f(h.down.x)?, f(h.down.y)?],
                                ])
                            })
                            .transpose()?,
                    })
                })
                .collect::<Result<_, Error>>()?,
        },
        Paint::Image(p) => RenderPaint::Image(RenderImage {
            asset: p.asset.clone(),
            fit: p.fit,
            crop: p.crop.map(|m| matrix(m.0)).transpose()?,
            opacity: number(&p.opacity)?,
        }),
        Paint::Shader(p) => RenderPaint::Shader {
            src: p.src.clone(),
            uniforms: uniforms(&p.uniforms)?,
        },
    })
}
pub(super) fn stroke(s: &Stroke) -> Result<RenderStroke, Error> {
    Ok(RenderStroke {
        widths: match &s.width {
            StrokeWidth::Uniform(v) => [number(v)?; 4],
            StrokeWidth::PerSide(v) => [
                number(&v[0])?,
                number(&v[1])?,
                number(&v[2])?,
                number(&v[3])?,
            ],
        },
        align: s.align,
        cap: s.cap,
        join: s.join,
        dash: s.dash.iter().map(|v| f(*v)).collect::<Result<_, _>>()?,
    })
}
pub(super) fn effect(e: &Effect) -> Result<Option<RenderEffect>, Error> {
    let shadow = |s: &Shadow| {
        Ok::<_, Error>(RenderShadow {
            offset: [number(&s.x)?, number(&s.y)?],
            blur: number(&s.blur)?,
            spread: number(&s.spread)?,
            color: color(&s.color)?,
            blend: s.blend_mode,
        })
    };
    Ok(match e {
        Effect::DropShadow(s) if s.visible => Some(RenderEffect::DropShadow(shadow(s)?)),
        Effect::InnerShadow(s) if s.visible => Some(RenderEffect::InnerShadow(shadow(s)?)),
        Effect::LayerBlur(b) if b.visible => Some(RenderEffect::LayerBlur(number(&b.radius)?)),
        Effect::BackgroundBlur(b) if b.visible => {
            Some(RenderEffect::BackgroundBlur(number(&b.radius)?))
        }
        Effect::Shader(s) if s.visible => Some(RenderEffect::Shader {
            src: s.src.clone(),
            uniforms: uniforms(&s.uniforms)?,
        }),
        _ => None,
    })
}
