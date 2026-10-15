use crate::{Error, Op, Page, encode_image, matrix, raster};
use skia_safe::{Paint, Path, PathFillType, Rect};
use vellum_core::export::svg::{self, Command};
pub(crate) fn write(page: &Page<'_>) -> Result<String, Error> {
    let mut commands = vec![];
    for op in &page.scene.ops {
        match op {
            Op::Save => commands.push(Command::Save),
            Op::Restore => commands.push(Command::Restore),
            Op::Transform(m) => commands.push(Command::Transform(*m)),
            Op::Layer(a) => commands.push(Command::Layer(*a)),
            Op::Clip(p) => commands.push(Command::Clip {
                path: p.to_svg(),
                even_odd: p.fill_type() == PathFillType::EvenOdd,
            }),
            Op::Path(p, paint) => path(&mut commands, p, paint),
            Op::Image {
                image,
                clip,
                transform,
                opacity,
            } => {
                commands.push(Command::Save);
                commands.push(Command::Clip {
                    path: Path::rect(*clip, None).to_svg(),
                    even_odd: false,
                });
                commands.push(Command::Image {
                    png: encode_image(image)?,
                    size: [image.width() as u32, image.height() as u32],
                    transform: *transform,
                    opacity: *opacity,
                });
                commands.push(Command::Restore);
            }
            Op::Glyphs(g, p) => {
                let mut bounds = vec![Rect::default(); g.ids.len()];
                g.font.get_bounds(&g.ids, &mut bounds, Some(p));
                let paths: Vec<_> = g.ids.iter().map(|id| g.font.get_path(*id)).collect();
                if paths
                    .iter()
                    .zip(&bounds)
                    .any(|(p, b)| p.is_none() && !b.is_empty())
                {
                    // Bitmap/colour glyphs retain their colours through a self-contained PNG; never <text>.
                    let mut rect = Rect::default();
                    for (b, pos) in bounds.iter().zip(&g.positions) {
                        rect.join(b.with_offset(*pos));
                    }
                    for r in &g.decorations {
                        rect.join(*r);
                    }
                    if rect.is_empty() {
                        continue;
                    }
                    let rect = Rect::from_ltrb(
                        rect.left.floor() - 1.,
                        rect.top.floor() - 1.,
                        rect.right.ceil() + 1.,
                        rect.bottom.ceil() + 1.,
                    );
                    let mut surface = raster(rect.width() as u32, rect.height() as u32)?;
                    surface.canvas().translate((-rect.left, -rect.top));
                    g.draw(surface.canvas(), p);
                    commands.push(Command::Image {
                        png: encode_image(&surface.image_snapshot())?,
                        size: [rect.width() as u32, rect.height() as u32],
                        transform: [1., 0., 0., 1., rect.left, rect.top],
                        opacity: 1.,
                    });
                } else {
                    for (glyph, pos) in paths.iter().zip(&g.positions) {
                        if let Some(glyph) = glyph {
                            let glyph =
                                glyph.with_transform(&matrix([1., 0., 0., 1., pos.x, pos.y]));
                            if p.style() == skia_safe::PaintStyle::Stroke {
                                let mut b = skia_safe::PathBuilder::new();
                                if !skia_safe::path_utils::fill_path_with_paint(
                                    &glyph, p, &mut b, None, None,
                                ) {
                                    return Err(Error::Geometry);
                                }
                                path(&mut commands, &b.detach(), p);
                            } else {
                                path(&mut commands, &glyph, p);
                            }
                        }
                    }
                    for r in &g.decorations {
                        path(&mut commands, &Path::rect(*r, None), p);
                    }
                }
            }
        }
    }
    svg::write(page.width, page.height, &commands).map_err(|_| Error::Geometry)
}
fn path(out: &mut Vec<Command>, p: &Path, paint: &Paint) {
    let c = paint.color();
    out.push(Command::Path {
        path: p.to_svg(),
        even_odd: p.fill_type() == PathFillType::EvenOdd,
        color: [c.r(), c.g(), c.b(), c.a()],
    });
}
