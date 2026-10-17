//! One prepared command stream for native raster/PDF and the editor's WebGL2 canvas.
//! Preparation validates all input before a canvas is touched. No platform font lookup or I/O.
mod appearance;
mod basic;
mod cache;
pub use cache::CacheStats;
mod fonts;
mod geometry;
mod svg;
#[cfg(target_os = "emscripten")]
pub mod webgl;
use skia_safe::{
    self as sk, Canvas, Color, Data, FontMgr, Image, ImageInfo, Paint, Path, Point, Rect,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};
use vellum_core::ports::{FontData, FontHandle};
use vellum_types::{AssetId, ImageFit, StrokeAlign, StrokeCap, StrokeJoin, display::*};
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Buffer(BufferError),
    Geometry,
    InvalidSize,
    FontBackend,
    InvalidFont(u64),
    MissingFont(u64),
    MissingImage(AssetId),
    InvalidImage(AssetId),
    Unsupported(&'static str),
    Encode,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "renderer: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Default, Clone)]
pub struct Resources {
    pub fonts: BTreeMap<FontHandle, FontData>,
    /// Host-supplied encoded PNG/JPEG bytes, keyed by document asset id. Never a file path or URL.
    pub images: BTreeMap<AssetId, Arc<[u8]>>,
}
pub struct Renderer {
    fonts: FontMgr,
    frames: RefCell<Vec<cache::FrameEntry>>,
}
/// Fully prepared, immutable scene. All backends execute the same commands.
#[derive(Clone)]
pub struct Scene {
    ops: Rc<Vec<Op>>,
    cache: Rc<RefCell<cache::DrawCache>>,
}
pub(crate) enum Op {
    Save,
    Restore,
    Transform([f32; 6]),
    Clip(Path),
    Layer {
        paint: Paint,
        backdrop: Option<sk::ImageFilter>,
    },
    Path(Path, Brush),
    Glyphs(fonts::Glyphs, Brush),
    Image {
        image: Image,
        clip: Rect,
        transform: [f32; 6],
        opacity: f32,
    },
}
#[derive(Clone, Copy)]
pub struct Page<'a> {
    pub width: u32,
    pub height: u32,
    pub scene: &'a Scene,
}
#[derive(Clone)]
pub(crate) struct Brush {
    paint: Paint,
    source: RenderPaint,
    bounds: Rect,
}
impl std::ops::Deref for Brush {
    type Target = Paint;
    fn deref(&self) -> &Paint {
        &self.paint
    }
}
impl Brush {
    fn new(source: &RenderPaint, bounds: Rect) -> Result<Self, Error> {
        // Empty/whitespace glyph runs and degenerate geometry still have valid appearance.
        let bounds = Rect::from_xywh(
            bounds.left,
            bounds.top,
            bounds.width().max(1.),
            bounds.height().max(1.),
        );
        Ok(Self {
            paint: appearance::paint(source, bounds)?,
            source: source.clone(),
            bounds,
        })
    }
}
impl Renderer {
    /// Portable geometry and paint plan for the Canvas2D fallback, without GPU initialization.
    pub fn prepare_basic(&self, buffer: &[u8], resources: &Resources) -> Result<Scene, Error> {
        self.prepare(&basic::simplify(buffer)?.0, resources)
    }
    pub fn basic_plan(&self, buffer: &[u8], resources: &Resources) -> Result<Vec<u8>, Error> {
        basic::plan(self, buffer, resources)
    }

    pub fn new() -> Result<Self, Error> {
        let fonts = FontMgr::custom_empty().ok_or(Error::FontBackend)?;
        // custom_empty has no system families; a platform manager is never an alternative.
        if fonts.family_names().any(|name| !name.is_empty()) {
            return Err(Error::FontBackend);
        }
        Ok(Self {
            fonts,
            frames: RefCell::default(),
        })
    }
    pub fn prepare_buffer(&self, buffer: &[u8], resources: &Resources) -> Result<Scene, Error> {
        self.prepare_frame("viewport", buffer, resources)
    }
    /// Host frame ids partition picture reuse. Exact content and font/image bytes invalidate atomically.
    pub fn prepare_frame(
        &self,
        id: &str,
        buffer: &[u8],
        resources: &Resources,
    ) -> Result<Scene, Error> {
        cache::validate_buffer(buffer)?;
        if let Some(e) = self
            .frames
            .borrow()
            .iter()
            .find(|e| e.id == id && e.bytes == buffer && cache::equal(&e.resources, resources))
        {
            return Ok(e.scene.clone());
        }
        let scene = self.prepare(
            &DisplayList::from_bytes(buffer).map_err(Error::Buffer)?,
            resources,
        )?;
        let size = buffer.len() + cache::resource_bytes(resources);
        let mut frames = self.frames.borrow_mut();
        frames.retain(|e| e.id != id);
        if size <= 32 * 1024 * 1024 {
            while frames.len() >= 8
                || frames
                    .iter()
                    .map(|e| e.bytes.len() + cache::resource_bytes(&e.resources))
                    .sum::<usize>()
                    + size
                    > 32 * 1024 * 1024
            {
                frames.remove(0);
            }
            frames.push(cache::FrameEntry {
                id: id.into(),
                bytes: buffer.to_vec(),
                resources: resources.clone(),
                scene: scene.clone(),
            });
        }
        Ok(scene)
    }
    pub fn prepare(&self, list: &DisplayList, resources: &Resources) -> Result<Scene, Error> {
        // The ABI validator also covers typed callers (indices, stack, finite values, resource kinds).
        list.to_bytes().map_err(Error::Buffer)?;
        let mut prepared_glyphs = BTreeMap::new();
        for cmd in &list.commands {
            if let DrawCmd::GlyphRun { glyphs, .. } = cmd
                && !prepared_glyphs.contains_key(glyphs)
            {
                let Resource::Glyphs(run) = &list.resources[*glyphs as usize] else {
                    unreachable!()
                };
                prepared_glyphs.insert(*glyphs, fonts::prepare(&self.fonts, run, resources)?);
            }
        }
        // A node's consecutive text runs share one normalized paint, including rich/bidi/font runs.
        let mut glyph_bounds = vec![Rect::default(); list.commands.len()];
        let mut i = 0;
        while i < list.commands.len() {
            if let DrawCmd::GlyphRun { paint, stroke, .. } = &list.commands[i] {
                let start = i;
                let mut bounds = Rect::default();
                while let Some(DrawCmd::GlyphRun {
                    glyphs,
                    paint: p,
                    stroke: s,
                }) = list.commands.get(i)
                {
                    if p != paint || s != stroke {
                        break;
                    }
                    bounds.join(prepared_glyphs[glyphs].bounds());
                    i += 1;
                }
                glyph_bounds[start..i].fill(bounds);
            } else {
                i += 1;
            }
        }
        let mut ops = vec![];
        for (at, cmd) in list.commands.iter().enumerate() {
            match cmd {
                DrawCmd::Save => ops.push(Op::Save),
                DrawCmd::Restore => ops.push(Op::Restore),
                DrawCmd::Transform(m) => ops.push(Op::Transform(*m)),
                DrawCmd::Layer { opacity, blend } => {
                    unit(*opacity)?;
                    let mut paint = Paint::default();
                    paint
                        .set_alpha_f(*opacity)
                        .set_blender(appearance::blender(*blend)?);
                    let effects = list.commands[at + 1..]
                        .iter()
                        .take_while(|cmd| matches!(cmd, DrawCmd::Effect { .. }))
                        .map(|cmd| {
                            let DrawCmd::Effect { effect } = cmd else {
                                unreachable!()
                            };
                            let Resource::Effect(effect) = &list.resources[*effect as usize] else {
                                unreachable!()
                            };
                            effect.clone()
                        })
                        .collect::<Vec<_>>();
                    let (filter, backdrop) = appearance::effects(&effects)?;
                    paint.set_image_filter(filter);
                    ops.push(Op::Layer { paint, backdrop });
                }
                DrawCmd::Effect { .. } => {
                    if at == 0
                        || !matches!(
                            list.commands[at - 1],
                            DrawCmd::Layer { .. } | DrawCmd::Effect { .. }
                        )
                    {
                        return Err(Error::Unsupported("effects must precede layer content"));
                    }
                }
                DrawCmd::Clip { shape } => {
                    ops.push(Op::Clip(geometry::shape(get_shape(list, *shape))?))
                }
                DrawCmd::Path {
                    shape,
                    paint,
                    stroke,
                } => {
                    let descriptor = get_shape(list, *shape);
                    let path = geometry::shape(descriptor)?;
                    let paint = get_paint(list, *paint);
                    let path = if let Some(i) = stroke {
                        stroke_path(&path, descriptor, get_stroke(list, *i))?
                    } else {
                        path
                    };
                    match paint {
                        RenderPaint::Image(image) => {
                            ops.push(Op::Save);
                            ops.push(Op::Clip(path.clone()));
                            ops.push(image_op(image, path.compute_tight_bounds(), resources)?);
                            ops.push(Op::Restore);
                        }
                        _ => {
                            let mut bounds = geometry::shape(descriptor)?.compute_tight_bounds();
                            if bounds.width() == 0. || bounds.height() == 0. {
                                bounds = path.compute_tight_bounds();
                            }
                            bounds = Rect::from_xywh(
                                bounds.left,
                                bounds.top,
                                bounds.width().max(1.),
                                bounds.height().max(1.),
                            );
                            ops.push(Op::Path(path, Brush::new(paint, bounds)?));
                        }
                    }
                }
                DrawCmd::Image { image, dst } => {
                    let Resource::Image(image) = &list.resources[*image as usize] else {
                        unreachable!()
                    };
                    ops.push(image_op(
                        image,
                        Rect::from_xywh(dst[0], dst[1], dst[2], dst[3]),
                        resources,
                    )?);
                }
                DrawCmd::GlyphRun {
                    glyphs,
                    paint,
                    stroke,
                } => {
                    let glyphs = prepared_glyphs[glyphs].clone();
                    let bounds = glyph_bounds[at];
                    let paint = Brush::new(get_paint(list, *paint), bounds)?;
                    if let Some(i) = stroke {
                        let s = get_stroke(list, *i);
                        validate_stroke(s)?;
                        if s.widths.iter().any(|w| *w != s.widths[0]) {
                            return Err(Error::Unsupported("per-side text stroke"));
                        }
                        for (id, position) in glyphs.ids.iter().zip(&glyphs.positions) {
                            let Some(path) = glyphs.font.get_path(*id) else {
                                let mut bounds = [Rect::default()];
                                glyphs.font.get_bounds(&[*id], &mut bounds, None);
                                if bounds[0].is_empty() {
                                    continue;
                                }
                                return Err(Error::Unsupported("bitmap glyph stroke"));
                            };
                            let path = path
                                .with_transform(&matrix([1., 0., 0., 1., position.x, position.y]));
                            let descriptor = Shape::Vector {
                                data: String::new(),
                                view_box: [0., 0., 1., 1.],
                                size: [1., 1.],
                                fill_rule: vellum_types::FillRule::Nonzero,
                            };
                            ops.push(Op::Path(stroke_path(&path, &descriptor, s)?, paint.clone()));
                        }
                        for rect in &glyphs.decorations {
                            ops.push(Op::Path(Path::rect(*rect, None), paint.clone()));
                        }
                    } else {
                        ops.push(Op::Glyphs(glyphs, paint));
                    }
                }
            }
        }
        appearance::mask_backdrops(&mut ops)?;
        Ok(Scene {
            ops: Rc::new(ops),
            cache: Rc::default(),
        })
    }
    pub fn rgba(&self, page: &Page<'_>) -> Result<Vec<u8>, Error> {
        let mut surface = raster(page.width, page.height)?;
        page.scene.draw(surface.canvas());
        let mut rgba = vec![0; page.width as usize * page.height as usize * 4];
        let info = ImageInfo::new(
            (page.width as i32, page.height as i32),
            sk::ColorType::RGBA8888,
            sk::AlphaType::Unpremul,
            None,
        );
        if !surface.read_pixels(&info, &mut rgba, page.width as usize * 4, (0, 0)) {
            return Err(Error::Encode);
        }
        Ok(rgba)
    }
    pub fn png(&self, page: &Page<'_>) -> Result<Vec<u8>, Error> {
        let mut surface = raster(page.width, page.height)?;
        page.scene.draw(surface.canvas());
        encode_image(&surface.image_snapshot())
    }
    pub fn pdf(&self, pages: &[Page<'_>]) -> Result<Vec<u8>, Error> {
        if pages.is_empty() {
            return Err(Error::InvalidSize);
        }
        for p in pages {
            dimensions(p.width, p.height)?;
        }
        let mut bytes = vec![];
        let mut doc = sk::pdf::new_document(&mut bytes, None);
        for page in pages {
            let mut open = doc.begin_page((page.width as f32, page.height as f32), None);
            page.scene.draw(open.canvas());
            doc = open.end_page();
        }
        doc.close();
        Ok(bytes)
    }
    pub fn svg(&self, page: &Page<'_>) -> Result<String, Error> {
        dimensions(page.width, page.height)?;
        svg::write(page)
    }
}
impl Scene {
    /// Draw into a clean device-space viewport. The caller owns clear/presentation/context lifetime.
    pub fn draw(&self, canvas: &Canvas) {
        self.draw_cached(canvas, 1.);
    }
    /// Zoom metadata selects viewport tiles below 50%; coordinates already include zoom and DPR.
    pub fn draw_cached(&self, canvas: &Canvas, zoom: f32) {
        cache::draw(self, canvas, zoom, &self.cache);
    }
    pub fn cache_stats(&self) -> CacheStats {
        self.cache.borrow().stats
    }
    pub(crate) fn draw_ops(&self, canvas: &Canvas) {
        let count = canvas.save();
        for op in self.ops.iter() {
            match op {
                Op::Save => {
                    canvas.save();
                }
                Op::Restore => {
                    canvas.restore();
                }
                Op::Transform(m) => {
                    canvas.set_matrix(&sk::M44::from(matrix(*m)));
                }
                Op::Clip(p) => {
                    canvas.clip_path(p, None, true);
                }
                Op::Layer { paint, backdrop } => {
                    let rec = sk::canvas::SaveLayerRec::default().paint(paint);
                    canvas.save_layer(&if let Some(filter) = backdrop {
                        rec.backdrop(filter)
                    } else {
                        rec
                    });
                }
                Op::Path(p, paint) => {
                    canvas.draw_path(p, paint);
                }
                Op::Glyphs(g, p) => g.draw(canvas, p),
                Op::Image {
                    image,
                    clip,
                    transform,
                    opacity,
                } => {
                    canvas.save();
                    canvas.clip_rect(*clip, None, true);
                    canvas.concat(&matrix(*transform));
                    let mut p = Paint::default();
                    p.set_alpha_f(*opacity);
                    canvas.draw_image_with_sampling_options(
                        image,
                        (0., 0.),
                        sk::SamplingOptions::new(sk::FilterMode::Linear, sk::MipmapMode::None),
                        Some(&p),
                    );
                    canvas.restore();
                }
            }
        }
        canvas.restore_to_count(count);
    }
}
pub(crate) fn dimensions(w: u32, h: u32) -> Result<(), Error> {
    if w == 0 || h == 0 || w > 16384 || h > 16384 || u64::from(w) * u64::from(h) > 64 * 1024 * 1024
    {
        Err(Error::InvalidSize)
    } else {
        Ok(())
    }
}
pub(crate) fn raster(w: u32, h: u32) -> Result<sk::Surface, Error> {
    dimensions(w, h)?;
    let mut s = sk::surfaces::raster_n32_premul((w as i32, h as i32)).ok_or(Error::InvalidSize)?;
    s.canvas().clear(Color::TRANSPARENT);
    Ok(s)
}
pub(crate) fn encode_image(image: &Image) -> Result<Vec<u8>, Error> {
    image
        .encode(None, sk::EncodedImageFormat::PNG, 100)
        .map(|d| d.as_bytes().to_vec())
        .ok_or(Error::Encode)
}
pub(crate) fn matrix(m: [f32; 6]) -> sk::Matrix {
    sk::Matrix::new_all(m[0], m[2], m[4], m[1], m[3], m[5], 0., 0., 1.)
}
fn get_shape(list: &DisplayList, i: u32) -> &Shape {
    let Resource::Shape(s) = &list.resources[i as usize] else {
        unreachable!()
    };
    s
}
fn get_paint(list: &DisplayList, i: u32) -> &RenderPaint {
    let Resource::Paint(p) = &list.resources[i as usize] else {
        unreachable!()
    };
    p
}
fn get_stroke(list: &DisplayList, i: u32) -> &RenderStroke {
    let Resource::Stroke(s) = &list.resources[i as usize] else {
        unreachable!()
    };
    s
}
fn unit(n: f32) -> Result<(), Error> {
    if (0. ..=1.).contains(&n) {
        Ok(())
    } else {
        Err(Error::Geometry)
    }
}
fn image_op(r: &RenderImage, dst: Rect, resources: &Resources) -> Result<Op, Error> {
    unit(r.opacity)?;
    let bytes = resources
        .images
        .get(&r.asset)
        .ok_or_else(|| Error::MissingImage(r.asset.clone()))?;
    let image = Image::from_encoded(Data::new_copy(bytes.as_ref()))
        .ok_or_else(|| Error::InvalidImage(r.asset.clone()))?;
    if dst.width() < 0. || dst.height() < 0. {
        return Err(Error::Geometry);
    }
    dimensions(image.width() as u32, image.height() as u32)
        .map_err(|_| Error::InvalidImage(r.asset.clone()))?;
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let (sx, sy) = (dst.width() / iw, dst.height() / ih);
    let (sx, sy) = match r.fit {
        ImageFit::Stretch => (sx, sy),
        ImageFit::Fill => (sx.max(sy), sx.max(sy)),
        ImageFit::Fit => (sx.min(sy), sx.min(sy)),
    };
    let mut transform = [
        sx,
        0.,
        0.,
        sy,
        dst.left + (dst.width() - iw * sx) / 2.,
        dst.top + (dst.height() - ih * sy) / 2.,
    ];
    if let Some(crop) = r.crop {
        // Crop maps normalized source coordinates into normalized destination coordinates, after fit.
        let crop = [
            crop[0],
            crop[1] * dst.height() / dst.width().max(f32::MIN_POSITIVE),
            crop[2] * dst.width() / dst.height().max(f32::MIN_POSITIVE),
            crop[3],
            crop[4] * dst.width(),
            crop[5] * dst.height(),
        ];
        transform = compose(
            [1., 0., 0., 1., dst.left, dst.top],
            compose(
                crop,
                compose([1., 0., 0., 1., -dst.left, -dst.top], transform),
            ),
        );
    }
    if !transform.iter().all(|n| n.is_finite()) {
        return Err(Error::Geometry);
    }
    Ok(Op::Image {
        image,
        clip: dst,
        transform,
        opacity: r.opacity,
    })
}
pub(crate) fn compose(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}
fn validate_stroke(s: &RenderStroke) -> Result<(), Error> {
    if s.widths.iter().any(|w| *w < 0.)
        || s.dash.iter().any(|n| *n < 0.)
        || (!s.dash.is_empty() && s.dash.iter().sum::<f32>() <= 0.)
    {
        return Err(Error::Geometry);
    }
    Ok(())
}
fn configure_stroke(p: &mut Paint, s: &RenderStroke, width: f32) -> Result<(), Error> {
    p.set_style(sk::PaintStyle::Stroke);
    p.set_stroke_width(width);
    p.set_stroke_cap(match s.cap {
        StrokeCap::Butt => sk::paint::Cap::Butt,
        StrokeCap::Round => sk::paint::Cap::Round,
        StrokeCap::Square => sk::paint::Cap::Square,
    });
    p.set_stroke_join(match s.join {
        StrokeJoin::Miter => sk::paint::Join::Miter,
        StrokeJoin::Round => sk::paint::Join::Round,
        StrokeJoin::Bevel => sk::paint::Join::Bevel,
    });
    if !s.dash.is_empty() {
        let mut dash = s.dash.clone();
        if dash.len() % 2 == 1 {
            dash.extend_from_slice(&s.dash);
        }
        p.set_path_effect(sk::PathEffect::dash(&dash, 0.).ok_or(Error::Geometry)?);
    }
    Ok(())
}
fn stroke_path(path: &Path, shape: &Shape, s: &RenderStroke) -> Result<Path, Error> {
    validate_stroke(s)?;
    let uniform = s.widths.iter().all(|n| *n == s.widths[0]);
    let single = |width: f32| -> Result<Path, Error> {
        if width == 0. {
            return Ok(Path::default());
        }
        let mut p = Paint::default();
        configure_stroke(
            &mut p,
            s,
            if s.align == StrokeAlign::Center {
                width
            } else {
                width * 2.
            },
        )?;
        let out = outline(path, &p)?;
        if matches!(shape, Shape::Line { .. }) || s.align == StrokeAlign::Center {
            return Ok(out);
        }
        out.op(
            path,
            if s.align == StrokeAlign::Inside {
                sk::PathOp::Intersect
            } else {
                sk::PathOp::Difference
            },
        )
        .ok_or(Error::Geometry)
    };
    if uniform {
        return single(s.widths[0]);
    }
    let Shape::Rect { size: [w, h], .. } = shape else {
        return Err(Error::Unsupported(
            "per-side stroke on non-rectangular geometry",
        ));
    };
    if *w == 0. || *h == 0. {
        return Ok(Path::default());
    }
    let center = Point::new(*w / 2., *h / 2.);
    let extent = 1. + s.widths.iter().copied().fold(0., f32::max) * 4. / w.min(*h);
    let corners = [
        Point::new(center.x - extent * w, center.y - extent * h),
        Point::new(center.x + extent * w, center.y - extent * h),
        Point::new(center.x + extent * w, center.y + extent * h),
        Point::new(center.x - extent * w, center.y + extent * h),
    ];
    let mut out = Path::default();
    for i in 0..4 {
        let region = Path::polygon(
            &[center, corners[i], corners[(i + 1) % 4]],
            true,
            None,
            None,
        );
        let side = single(s.widths[i])?
            .op(&region, sk::PathOp::Intersect)
            .ok_or(Error::Geometry)?;
        out = out.op(&side, sk::PathOp::Union).ok_or(Error::Geometry)?;
    }
    Ok(out)
}

pub(crate) fn outline(path: &Path, paint: &Paint) -> Result<Path, Error> {
    let mut b = sk::PathBuilder::new();
    if !sk::path_utils::fill_path_with_paint(path, paint, &mut b, None, None) {
        return Err(Error::Geometry);
    }
    Ok(b.detach())
}

impl Renderer {
    /// JPEG has no alpha. Transparent canvas areas are composited onto white before encoding.
    pub fn jpeg(&self, page: &Page<'_>) -> Result<Vec<u8>, Error> {
        let mut surface = raster(page.width, page.height)?;
        surface.canvas().clear(Color::WHITE);
        page.scene.draw(surface.canvas());
        surface
            .image_snapshot()
            .encode(None, sk::EncodedImageFormat::JPEG, 95)
            .map(|d| d.as_bytes().to_vec())
            .ok_or(Error::Encode)
    }
    pub fn webp(&self, page: &Page<'_>) -> Result<Vec<u8>, Error> {
        let mut surface = raster(page.width, page.height)?;
        page.scene.draw(surface.canvas());
        surface
            .image_snapshot()
            .encode(None, sk::EncodedImageFormat::WEBP, 100)
            .map(|d| d.as_bytes().to_vec())
            .ok_or(Error::Encode)
    }
}
impl vellum_core::ports::ExportRenderer for Renderer {
    fn render(
        &self,
        frames: &[vellum_core::export::ExportFrame],
        format: &str,
    ) -> Result<Vec<u8>, vellum_core::ports::ExportError> {
        use vellum_core::{export::ExportScene, ports::ExportError};
        ExportScene {
            frames: frames.to_vec(),
        }
        .check(1.)?;
        if format != "pdf" && frames.len() != 1 {
            return Err(ExportError::new("This encoding takes one frame"));
        }
        let scenes = frames
            .iter()
            .map(|f| {
                self.prepare(
                    &f.output.list,
                    &Resources {
                        fonts: f.output.fonts.clone(),
                        images: f.images.clone(),
                    },
                )
                .map_err(ExportError::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pages: Vec<_> = frames
            .iter()
            .zip(&scenes)
            .map(|(f, scene)| {
                let (width, height) = f.size();
                Page {
                    width,
                    height,
                    scene,
                }
            })
            .collect();
        let bytes = match format {
            "png" => self.png(&pages[0]),
            "jpeg" => self.jpeg(&pages[0]),
            "webp" => self.webp(&pages[0]),
            "svg" => self.svg(&pages[0]).map(String::into_bytes),
            "pdf" => self.pdf(&pages),
            _ => return Err(ExportError::new("Unsupported export encoding")),
        }
        .map_err(ExportError::new)?;
        Ok(bytes)
    }
}
