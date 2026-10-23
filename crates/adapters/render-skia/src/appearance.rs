//! Backend lowering of normalized paints and ordered layer effects. No document mutations.
use crate::{Error, compose, matrix};
use skia_safe::{self as sk, Color, ImageFilter, Paint, Point, Rect};
use vellum_types::{BlendMode, GradientKind, display::*};
pub(crate) fn color([r, g, b, a]: [u8; 4]) -> Color {
    Color::from_argb(a, r, g, b)
}
pub(crate) fn blender(mode: BlendMode) -> Result<sk::Blender, Error> {
    use sk::BlendMode as B;
    Ok(match mode {
        BlendMode::Normal=>B::SrcOver, BlendMode::Darken=>B::Darken, BlendMode::Multiply=>B::Multiply,
        BlendMode::ColorBurn=>B::ColorBurn, BlendMode::Lighten=>B::Lighten, BlendMode::Screen=>B::Screen,
        BlendMode::ColorDodge=>B::ColorDodge, BlendMode::LinearDodge=>B::Plus, BlendMode::Overlay=>B::Overlay,
        BlendMode::SoftLight=>B::SoftLight, BlendMode::HardLight=>B::HardLight, BlendMode::Difference=>B::Difference,
        BlendMode::Exclusion=>B::Exclusion, BlendMode::Hue=>B::Hue, BlendMode::Saturation=>B::Saturation,
        BlendMode::Color=>B::Color, BlendMode::Luminosity=>B::Luminosity,
        BlendMode::LinearBurn=> return sk::RuntimeEffect::make_for_blender(
            "half4 main(half4 s, half4 d) { return half4(max(s.rgb*d.a+d.rgb*s.a-s.a*d.a,half3(0))+s.rgb*(1-d.a)+d.rgb*(1-s.a),s.a+d.a-s.a*d.a); }", None)
            .map_err(|_|Error::Geometry)?.make_blender(sk::Data::new_empty(),None).ok_or(Error::Geometry),
    }.into())
}
pub(crate) fn paint(
    p: &RenderPaint,
    bounds: Rect,
    resources: &crate::Resources,
    errors: &mut Vec<crate::ShaderDiagnostic>,
) -> Result<Paint, Error> {
    let mut out = Paint::default();
    out.set_anti_alias(true);
    let base = [
        bounds.width(),
        0.,
        0.,
        bounds.height(),
        bounds.left,
        bounds.top,
    ];
    match p {
        RenderPaint::Solid(c) => {
            out.set_color(color(*c));
        }
        RenderPaint::Gradient {
            kind,
            transform,
            stops,
        } => {
            if stops.is_empty()
                || stops.iter().any(|s| !(0. ..=1.).contains(&s.position))
                || stops.windows(2).any(|s| s[0].position > s[1].position)
            {
                return Err(Error::Geometry);
            }
            if stops.len() == 1 {
                out.set_color(color(stops[0].color));
                return Ok(out);
            }
            let m = matrix(compose(base, *transform));
            if m.invert().is_none() {
                return Err(Error::Geometry);
            }
            let colors: Vec<sk::Color4f> = stops.iter().map(|s| color(s.color).into()).collect();
            let pos: Vec<_> = stops.iter().map(|s| s.position).collect();
            let gradient = sk::gradient::Gradient::new(
                sk::gradient::Colors::new(&colors, Some(&pos), sk::TileMode::Clamp, None),
                sk::gradient::Interpolation::default(),
            );
            let shader = match kind {
                GradientKind::Linear => sk::gradient::shaders::linear_gradient(
                    ((0., 0.5), (1., 0.5)),
                    &gradient,
                    Some(&m),
                ),
                GradientKind::Radial => {
                    sk::gradient::shaders::radial_gradient(((0.5, 0.5), 0.5), &gradient, Some(&m))
                }
                GradientKind::Angular => sk::gradient::shaders::sweep_gradient(
                    (0.5, 0.5),
                    (0., 360.),
                    &gradient,
                    Some(&m),
                ),
            }
            .ok_or(Error::Geometry)?;
            out.set_shader(shader);
        }
        RenderPaint::Mesh {
            columns,
            rows,
            points,
        } => {
            if *columns < 2
                || *rows < 2
                || u64::from(*columns) * u64::from(*rows) != points.len() as u64
                || points.len() > 65536
            {
                return Err(Error::Geometry);
            }
            if bounds.is_empty() {
                return Ok(out);
            }
            // Coons patches share their boundary cubics. Handles are offsets [left,right,up,down].
            let mut recorder = sk::PictureRecorder::new();
            let canvas =
                recorder.begin_recording(Rect::from_wh(bounds.width(), bounds.height()), false);
            let point = |p: &RenderMeshPoint| {
                Point::new(
                    p.position[0] * bounds.width(),
                    p.position[1] * bounds.height(),
                )
            };
            let handle = |p: &RenderMeshPoint, q: &RenderMeshPoint, i: usize| -> Point {
                let a = point(p);
                if let Some(h) = p.handles {
                    Point::new(
                        a.x + h[i][0] * bounds.width(),
                        a.y + h[i][1] * bounds.height(),
                    )
                } else {
                    let b = point(q);
                    Point::new(a.x + (b.x - a.x) / 3., a.y + (b.y - a.y) / 3.)
                }
            };
            let c = *columns as usize;
            for y in 0..*rows as usize - 1 {
                for x in 0..c - 1 {
                    let a = &points[y * c + x];
                    let b = &points[y * c + x + 1];
                    let d = &points[(y + 1) * c + x];
                    let e = &points[(y + 1) * c + x + 1];
                    let cubics = [
                        point(a),
                        handle(a, b, 1),
                        handle(b, a, 0),
                        point(b),
                        handle(b, e, 3),
                        handle(e, b, 2),
                        point(e),
                        handle(e, d, 0),
                        handle(d, e, 1),
                        point(d),
                        handle(d, a, 2),
                        handle(a, d, 3),
                    ];
                    canvas.draw_patch(
                        &cubics,
                        &[
                            color(a.color),
                            color(b.color),
                            color(e.color),
                            color(d.color),
                        ],
                        None,
                        sk::BlendMode::Dst,
                        &out,
                    );
                }
            }
            let pic = recorder
                .finish_recording_as_picture(None)
                .ok_or(Error::Geometry)?;
            out.set_shader(pic.to_shader(
                (sk::TileMode::Clamp, sk::TileMode::Clamp),
                sk::FilterMode::Linear,
                Some(&matrix([1., 0., 0., 1., bounds.left, bounds.top])),
                None,
            ));
        }
        RenderPaint::Shader { src, uniforms } => {
            match crate::shaders::fill(src, uniforms, bounds, resources) {
                Ok(shader) => {
                    out.set_shader(shader);
                }
                Err(error) => {
                    errors.push(crate::ShaderDiagnostic {
                        src: src.clone(),
                        error,
                    });
                    out.set_color(Color::TRANSPARENT);
                }
            }
        }
        RenderPaint::Image(_) => {
            return Err(Error::Unsupported(
                "image paint must be lowered with resources",
            ));
        }
    }
    Ok(out)
}
fn radius(n: f32) -> Result<f32, Error> {
    if n >= 0. {
        Ok(n / 2.)
    } else {
        Err(Error::Geometry)
    }
}
fn blur(n: f32, input: Option<ImageFilter>) -> Result<ImageFilter, Error> {
    let s = radius(n)?;
    sk::image_filters::blur((s, s), None, input, None).ok_or(Error::Geometry)
}
pub(crate) fn effects(
    effects: &[RenderEffect],
    bounds: Rect,
    resources: &crate::Resources,
    errors: &mut Vec<crate::ShaderDiagnostic>,
) -> Result<(Option<ImageFilter>, Option<ImageFilter>), Error> {
    use sk::image_filters as f;
    let mut input = None;
    let mut backdrop = None;
    for effect in effects {
        input = match effect {
            RenderEffect::LayerBlur(n) => Some(blur(*n, input)?),
            RenderEffect::BackgroundBlur(n) => {
                backdrop = Some(blur(*n, backdrop)?);
                input
            }
            RenderEffect::DropShadow(s) | RenderEffect::InnerShadow(s) => {
                radius(s.blur)?;
                let inner = matches!(effect, RenderEffect::InnerShadow(_));
                let spread = if inner { -s.spread } else { s.spread };
                let mut mask = input.clone();
                if spread > 0. {
                    mask = f::dilate((spread, spread), mask, None);
                } else if spread < 0. {
                    mask = f::erode((-spread, -spread), mask, None);
                }
                if inner {
                    mask = f::color_filter(
                        sk::color_filters::matrix_row_major(
                            &[
                                0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
                                0., -1., 1.,
                            ],
                            None,
                        ),
                        mask,
                        None,
                    );
                }
                mask = f::offset((s.offset[0], s.offset[1]), mask, None);
                mask = Some(blur(s.blur, mask)?);
                mask = f::color_filter(
                    sk::color_filters::blend(color(s.color), sk::BlendMode::SrcIn)
                        .ok_or(Error::Geometry)?,
                    mask,
                    None,
                );
                if inner {
                    mask = f::blend(sk::BlendMode::DstIn, mask, input.clone(), None);
                    f::blend(blender(s.blend)?, input, mask, None)
                } else {
                    // Blend content over its shadow, then retain only the extra shadow with its requested blend.
                    let outside = f::blend(sk::BlendMode::DstOut, mask, input.clone(), None);
                    f::blend(blender(s.blend)?, input, outside, None)
                }
            }
            RenderEffect::Shader { src, uniforms } => {
                match crate::shaders::effect(src, uniforms, bounds, resources, input.clone()) {
                    Ok(filter) => Some(filter),
                    Err(error) => {
                        errors.push(crate::ShaderDiagnostic {
                            src: src.clone(),
                            error,
                        });
                        input
                    }
                }
            }
        };
    }
    Ok((input, backdrop))
}

/// Backdrop filters sample the destination but write only inside the layer's geometric silhouette.
/// Work in the layer's local coordinates; absolute transforms inside the group are rebased here.
pub(crate) fn mask_backdrops(
    ops: &mut [crate::Op],
    pending: &std::collections::BTreeMap<usize, Vec<RenderEffect>>,
    resources: &crate::Resources,
    errors: &mut Vec<crate::ShaderDiagnostic>,
) -> Result<(), Error> {
    use crate::Op;
    let mut current = sk::Matrix::new_identity();
    let mut stack = Vec::new();
    for at in 0..ops.len() {
        match &ops[at] {
            Op::Transform(m) => current = matrix(*m),
            Op::Save => stack.push(current),
            Op::Restore => current = stack.pop().ok_or(Error::Geometry)?,
            Op::Layer { .. } => {
                stack.push(current);
                let bounds = if pending[&at]
                    .iter()
                    .any(|e| matches!(e, RenderEffect::Shader { .. }))
                {
                    let inverse = current.invert().ok_or(Error::Geometry)?;
                    coverage(&ops[at + 1..], &inverse)?.compute_tight_bounds()
                } else {
                    Rect::default()
                };
                let failed = errors.len();
                let (filter, backdrop) = effects(&pending[&at], bounds, resources, errors)?;
                // A filtered layer is bounded by its content, so its filters do not run over the whole
                // viewport. A shader effect (one that compiled; a failed one leaves its input as it was) and a
                // backdrop may draw where the content is transparent; they keep the whole layer.
                let shaders = pending[&at]
                    .iter()
                    .filter(|e| matches!(e, RenderEffect::Shader { .. }))
                    .count();
                let content =
                    if filter.is_some() && backdrop.is_none() && errors.len() - failed == shaders {
                        content_bounds(ops, at, pending, &current)
                    } else {
                        None
                    };
                if let Op::Layer {
                    paint,
                    backdrop: target,
                    bounds,
                } = &mut ops[at]
                {
                    paint.set_image_filter(filter);
                    *target = backdrop;
                    *bounds = content;
                }
                let Op::Layer { backdrop, .. } = &ops[at] else {
                    unreachable!()
                };
                if let Some(filter) = backdrop {
                    let inverse = current.invert().ok_or(Error::Geometry)?;
                    let mask = coverage(&ops[at + 1..], &inverse)?;
                    let bounds = mask.compute_tight_bounds();
                    let mut rec = sk::PictureRecorder::new();
                    let canvas = rec.begin_recording(bounds, false);
                    let mut paint = Paint::default();
                    paint.set_color(Color::WHITE).set_anti_alias(true);
                    canvas.draw_path(&mask, &paint);
                    let pic = rec
                        .finish_recording_as_picture(None)
                        .ok_or(Error::Geometry)?;
                    let clipped = sk::image_filters::blend(
                        sk::BlendMode::DstIn,
                        Some(filter.clone()),
                        sk::image_filters::picture(pic, None),
                        None,
                    )
                    .ok_or(Error::Geometry)?;
                    if let Op::Layer { backdrop, .. } = &mut ops[at] {
                        *backdrop = Some(clipped);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
/// Conservative bounds of what the layer at `at` draws, in the layer's own space (`current` is its matrix), from
/// each drawing's fast bounds (strokes included) in device space. None when a nested layer has effects of its
/// own (they reach past their content) or a drawing cannot be bounded.
fn content_bounds(
    ops: &[crate::Op],
    at: usize,
    pending: &std::collections::BTreeMap<usize, Vec<RenderEffect>>,
    current: &sk::Matrix,
) -> Option<Rect> {
    use crate::Op;
    let mut m = *current;
    let mut clip: Option<Rect> = None;
    let mut stack = Vec::new();
    let mut out: Option<Rect> = None;
    let mut add = |r: Rect, m: &sk::Matrix, clip: &Option<Rect>| {
        let mut r = m.map_rect(r).0;
        if let Some(c) = clip
            && !r.intersect(c)
        {
            return;
        }
        out = Some(out.map_or(r, |o| Rect::join2(o, r)));
    };
    // What a paint can cover past its geometry: half a stroke, longer at miters and square caps.
    let fast = |paint: &Paint, r: Rect| {
        if paint.path_effect().is_some()
            || paint.mask_filter().is_some()
            || paint.image_filter().is_some()
        {
            return None;
        }
        if paint.style() == sk::PaintStyle::Fill {
            return Some(r);
        }
        let reach = paint.stroke_miter().max(std::f32::consts::SQRT_2);
        let half = (paint.stroke_width() / 2.).max(0.5) * reach;
        Some(r.with_outset((half, half)))
    };
    for (index, op) in ops.iter().enumerate().skip(at + 1) {
        match op {
            Op::Layer { .. } if pending.get(&index).is_some_and(|e| !e.is_empty()) => return None,
            Op::Save | Op::Layer { .. } => stack.push((m, clip)),
            Op::Restore => {
                let Some((saved, c)) = stack.pop() else { break };
                m = saved;
                clip = c;
            }
            Op::Transform(v) => m = matrix(*v),
            Op::Clip(p) => {
                let r = m.map_rect(p.bounds()).0;
                clip = Some(match clip {
                    Some(mut c) => {
                        if !c.intersect(r) {
                            c = Rect::default();
                        }
                        c
                    }
                    None => r,
                });
            }
            Op::Path(p, brush) => add(fast(brush, p.bounds().with_outset((1., 1.)))?, &m, &clip),
            Op::Image { clip: r, .. } => add(*r, &m, &clip),
            Op::Glyphs(g, brush) => {
                let mut boxes = vec![Rect::default(); g.ids.len()];
                g.font.get_bounds(&g.ids, &mut boxes, None);
                for (b, pos) in boxes.iter().zip(&g.positions) {
                    add(
                        fast(brush, b.with_offset(*pos).with_outset((1., 1.)))?,
                        &m,
                        &clip,
                    );
                }
                for r in &g.decorations {
                    add(fast(brush, *r)?, &m, &clip);
                }
            }
        }
    }
    // Anti-aliased edges reach a pixel past the geometry. A layer that draws nothing has nothing to filter.
    let Some(device) = out else {
        return Some(Rect::default());
    };
    let device = device.with_outset((2., 2.));
    Some(current.invert()?.map_rect(device).0)
}
fn coverage(ops: &[crate::Op], inverse: &sk::Matrix) -> Result<sk::Path, Error> {
    use crate::Op;
    let mut m = sk::Matrix::new_identity();
    let mut clip: Option<sk::Path> = None;
    let mut stack = Vec::new();
    let mut out = sk::Path::default();
    let add = |out: &mut sk::Path, p: sk::Path, clip: &Option<sk::Path>| -> Result<(), Error> {
        let p = if let Some(c) = clip {
            p.op(c, sk::PathOp::Intersect).ok_or(Error::Geometry)?
        } else {
            p
        };
        *out = out.op(&p, sk::PathOp::Union).ok_or(Error::Geometry)?;
        Ok(())
    };
    for op in ops {
        match op {
            Op::Save | Op::Layer { .. } => stack.push((m, clip.clone())),
            Op::Restore => {
                let Some((saved, c)) = stack.pop() else { break };
                m = saved;
                clip = c;
            }
            Op::Transform(v) => m = sk::Matrix::concat(inverse, &matrix(*v)),
            Op::Clip(p) => {
                let p = p.with_transform(&m);
                clip = Some(if let Some(c) = clip {
                    p.op(&c, sk::PathOp::Intersect).ok_or(Error::Geometry)?
                } else {
                    p
                });
            }
            Op::Path(p, _) => add(&mut out, p.with_transform(&m), &clip)?,
            Op::Image { clip: r, .. } => {
                add(&mut out, sk::Path::rect(*r, None).with_transform(&m), &clip)?
            }
            Op::Glyphs(g, _) => {
                for (id, pos) in g.ids.iter().zip(&g.positions) {
                    let p = if let Some(p) = g.font.get_path(*id) {
                        p
                    } else {
                        let mut b = [Rect::default()];
                        g.font.get_bounds(&[*id], &mut b, None);
                        sk::Path::rect(b[0], None)
                    };
                    let p = p.with_transform(&matrix([1., 0., 0., 1., pos.x, pos.y]));
                    add(&mut out, p.with_transform(&m), &clip)?;
                }
                for r in &g.decorations {
                    add(&mut out, sk::Path::rect(*r, None).with_transform(&m), &clip)?;
                }
            }
        }
    }
    Ok(out)
}
