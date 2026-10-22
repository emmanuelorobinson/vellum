//! Geometry compilation shared with the fallback. Canvas2D executes these commands independently.
use crate::{Error, Op, Renderer, Resources, raster};
use serde::Serialize;
use skia_safe::{self as sk, Path, Rect};
use std::collections::BTreeMap;
use vellum_types::{BlendMode, display::*};
#[derive(Serialize)]
struct Plan {
    paths: Vec<String>,
    commands: Vec<Command>,
    simplifications: Vec<&'static str>,
}
#[derive(Serialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
enum Command {
    Save,
    Restore,
    Transform {
        matrix: [f32; 6],
    },
    Clip {
        data: usize,
        even_odd: bool,
    },
    Layer {
        opacity: f32,
        blend: String,
    },
    Path {
        data: usize,
        even_odd: bool,
        paint: RenderPaint,
        bounds: [f32; 4],
    },
    Image {
        pixels: Vec<u8>,
        size: [u32; 2],
        clip: [f32; 4],
        transform: [f32; 6],
        opacity: f32,
    },
}
fn rect(r: Rect) -> [f32; 4] {
    [r.left, r.top, r.width(), r.height()]
}
#[derive(Default)]
struct Geometry {
    indices: BTreeMap<u32, usize>,
    paths: Vec<String>,
}
impl Geometry {
    fn insert(&mut self, path: &Path) -> usize {
        let id = path.generation_id();
        if let Some(index) = self.indices.get(&id) {
            return *index;
        }
        let index = self.paths.len();
        self.paths.push(path.to_svg());
        self.indices.insert(id, index);
        index
    }
}
fn path(p: &Path, brush: &crate::Brush, geometry: &mut Geometry) -> Command {
    Command::Path {
        data: geometry.insert(p),
        even_odd: p.fill_type() == sk::PathFillType::EvenOdd,
        paint: brush.source.clone(),
        bounds: rect(brush.bounds),
    }
}
fn pixels(image: &sk::Image) -> Result<Vec<u8>, Error> {
    let info = sk::ImageInfo::new(
        (image.width(), image.height()),
        sk::ColorType::RGBA8888,
        sk::AlphaType::Unpremul,
        None,
    );
    let mut bytes = vec![0; image.width() as usize * image.height() as usize * 4];
    if !image.read_pixels(
        &info,
        &mut bytes,
        image.width() as usize * 4,
        (0, 0),
        sk::image::CachingHint::Disallow,
    ) {
        return Err(Error::Encode);
    }
    Ok(bytes)
}
pub(crate) fn simplify(buffer: &[u8]) -> Result<(DisplayList, Vec<&'static str>), Error> {
    let mut list = DisplayList::from_bytes(buffer).map_err(Error::Buffer)?;
    let mut simplifications = vec![];
    if list
        .commands
        .iter()
        .any(|c| matches!(c, DrawCmd::Effect { .. }))
    {
        simplifications.push("effects-omitted");
    }
    list.commands
        .retain(|c| !matches!(c, DrawCmd::Effect { .. }));
    for cmd in &mut list.commands {
        if let DrawCmd::Layer {
            blend: BlendMode::LinearBurn,
            ..
        } = cmd
        {
            simplifications.push("linear-burn-as-normal");
            if let DrawCmd::Layer { blend, .. } = cmd {
                *blend = BlendMode::Normal;
            }
        }
    }
    for resource in &mut list.resources {
        if let Resource::Paint(p) = resource {
            match p {
                RenderPaint::Shader { .. } => {
                    simplifications.push("shader-as-transparent");
                    *p = RenderPaint::Solid([0; 4]);
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
                    let mut sum = [0u64; 4];
                    for p in points.iter() {
                        for (s, c) in sum.iter_mut().zip(p.color) {
                            *s += u64::from(c);
                        }
                    }
                    *p = RenderPaint::Solid(sum.map(|c| (c / points.len() as u64) as u8));
                    simplifications.push("mesh-as-average-color");
                }
                _ => {}
            }
        }
    }
    simplifications.sort();
    simplifications.dedup();
    Ok((list, simplifications))
}
pub(crate) fn plan(
    renderer: &Renderer,
    buffer: &[u8],
    resources: &Resources,
) -> Result<Vec<u8>, Error> {
    let (list, simplifications) = simplify(buffer)?;
    let scene = renderer.prepare(&list, resources)?;
    let mut commands = vec![];
    let mut geometry = Geometry::default();
    let mut scale = 1f32;
    let mut scales = vec![];
    for op in scene.ops.iter() {
        match op {
            Op::Save => {
                scales.push(scale);
                commands.push(Command::Save)
            }
            Op::Restore => {
                scale = scales.pop().ok_or(Error::Geometry)?;
                commands.push(Command::Restore)
            }
            Op::Transform(m) => {
                scale = m[0].hypot(m[1]).max(m[2].hypot(m[3])).clamp(0.125, 8.);
                commands.push(Command::Transform { matrix: *m })
            }
            Op::Clip(p) => commands.push(Command::Clip {
                data: geometry.insert(p),
                even_odd: p.fill_type() == sk::PathFillType::EvenOdd,
            }),
            Op::Layer { paint, .. } => {
                scales.push(scale);
                commands.push(Command::Layer {
                    opacity: paint.alpha_f(),
                    blend: format!("{:?}", paint.blend_mode_or(sk::BlendMode::SrcOver)),
                })
            }
            Op::Path(p, b) => commands.push(path(p, b, &mut geometry)),
            Op::Image {
                image,
                clip,
                transform,
                opacity,
            } => commands.push(Command::Image {
                pixels: pixels(image)?,
                size: [image.width() as u32, image.height() as u32],
                clip: rect(*clip),
                transform: *transform,
                opacity: *opacity,
            }),
            Op::Glyphs(g, b) => {
                // Preserve hinting, variable font synthesis and color glyphs without consulting system fonts.
                let bounds = g.bounds();
                if bounds.is_empty() {
                    continue;
                }
                let r = Rect::from_ltrb(
                    (bounds.left * scale).floor() / scale - 2. / scale,
                    (bounds.top * scale).floor() / scale - 2. / scale,
                    (bounds.right * scale).ceil() / scale + 2. / scale,
                    (bounds.bottom * scale).ceil() / scale + 2. / scale,
                );
                let width = (r.width() * scale).round() as u32;
                let height = (r.height() * scale).round() as u32;
                let mut surface = raster(width, height)?;
                surface.canvas().scale((scale, scale));
                surface.canvas().translate((-r.left, -r.top));
                g.draw(surface.canvas(), b);
                commands.push(Command::Image {
                    pixels: pixels(&surface.image_snapshot())?,
                    size: [width, height],
                    clip: rect(r),
                    transform: [1. / scale, 0., 0., 1. / scale, r.left, r.top],
                    opacity: 1.,
                });
            }
        }
    }
    let bytes = serde_json::to_vec(&Plan {
        paths: geometry.paths,
        commands,
        simplifications,
    })
    .map_err(|_| Error::Encode)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(Error::Encode);
    }
    Ok(bytes)
}
