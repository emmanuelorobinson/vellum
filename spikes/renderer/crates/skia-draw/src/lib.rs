//! Draws a spike display list onto any skia-safe canvas: a CPU raster surface (native PNG path) or
//! a Ganesh GL surface over WebGL2 (Renderer B). One function, so the two paths share every call.

use skia_safe::{
    AlphaType, BlendMode, Canvas, ClipOp, Color4f, ColorType, Data, FilterMode, Font, FontMgr,
    Image, ImageInfo, MipmapMode, Paint, PaintStyle, Path, Point, RRect, Rect, SamplingOptions,
    Shader, TileMode, Typeface,
    gradient::{Colors, Gradient, Interpolation, shaders},
    images,
};
use spike_scene::{DisplayList, ImageData, blend, op, paint_kind};

pub use skia_safe;

pub fn make_image(data: &ImageData) -> Image {
    let info = ImageInfo::new(
        (data.width as i32, data.height as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    images::raster_from_data(&info, Data::new_copy(&data.rgba), data.width as usize * 4)
        .expect("raster image from RGBA8")
}

pub fn make_typeface(font: &[u8]) -> Option<Typeface> {
    FontMgr::new().new_from_data(Data::new_copy(font), None)
}

/// A display list plus the resources it references, ready to draw many times.
pub struct SkiaScene {
    pub list: DisplayList,
    pub images: Vec<Image>,
    pub fonts: Vec<Font>,
    typeface: Option<Typeface>,
}

impl SkiaScene {
    pub fn new(list: DisplayList, images: Vec<Image>, typeface: Option<Typeface>) -> Self {
        Self {
            list,
            images,
            fonts: Vec::new(),
            typeface,
        }
    }

    pub fn draw(&mut self, canvas: &Canvas) {
        let ops = &self.list.ops;
        let mut i = 0;
        let mut fill = Paint::default();
        fill.set_anti_alias(true);
        let mut stroke = Paint::default();
        stroke.set_anti_alias(true);
        stroke.set_style(PaintStyle::Stroke);
        while i < ops.len() {
            let code = ops[i];
            i += 1;
            if code == op::SAVE {
                canvas.save();
            } else if code == op::RESTORE {
                canvas.restore();
            } else if code == op::TRANSLATE {
                canvas.translate((ops[i], ops[i + 1]));
                i += 2;
            } else if code == op::ROTATE {
                canvas.rotate(ops[i], Some(Point::new(ops[i + 1], ops[i + 2])));
                i += 3;
            } else if code == op::CLIP_RRECT {
                canvas.clip_rrect(rrect(&ops[i..i + 5]), ClipOp::Intersect, true);
                i += 5;
            } else if code == op::SAVE_LAYER_ALPHA {
                canvas.save_layer_alpha_f(None, ops[i]);
                i += 1;
            } else if code == op::FILL_RRECT {
                let shape = rrect(&ops[i..i + 5]);
                i = read_paint(ops, i + 5, &mut fill);
                canvas.draw_rrect(shape, &fill);
            } else if code == op::STROKE_RRECT {
                let shape = rrect(&ops[i..i + 5]);
                i = read_stroke(ops, i + 5, &mut stroke);
                canvas.draw_rrect(shape, &stroke);
            } else if code == op::FILL_OVAL {
                let shape = rect(&ops[i..i + 4]);
                i = read_paint(ops, i + 4, &mut fill);
                canvas.draw_oval(shape, &fill);
            } else if code == op::STROKE_OVAL {
                let shape = rect(&ops[i..i + 4]);
                i = read_stroke(ops, i + 4, &mut stroke);
                canvas.draw_oval(shape, &stroke);
            } else if code == op::FILL_POLY {
                let n = ops[i] as usize;
                let points: Vec<Point> = (0..n)
                    .map(|k| Point::new(ops[i + 1 + k * 2], ops[i + 2 + k * 2]))
                    .collect();
                i = read_paint(ops, i + 1 + n * 2, &mut fill);
                canvas.draw_path(&Path::polygon(&points, true, None, None), &fill);
            } else if code == op::IMAGE {
                let image = &self.images[ops[i] as usize];
                let dst = rect(&ops[i + 1..i + 5]);
                let filter = if ops[i + 5] > 0.5 {
                    FilterMode::Linear
                } else {
                    FilterMode::Nearest
                };
                let paint = Paint::default();
                canvas.draw_image_rect_with_sampling_options(
                    image,
                    None,
                    dst,
                    SamplingOptions::new(filter, MipmapMode::None),
                    &paint,
                );
                i += 6;
            } else if code == op::TEXT {
                let text = &self.list.strings[ops[i] as usize];
                let (x, y, size) = (ops[i + 1], ops[i + 2], ops[i + 3]);
                let mut paint = Paint::new(color(&ops[i + 4..i + 8]), None);
                paint.set_anti_alias(true);
                i += 8;
                if let Some(typeface) = &self.typeface {
                    let font = match self.fonts.iter().find(|f| f.size() == size) {
                        Some(font) => font,
                        None => {
                            self.fonts.push(Font::from_typeface(typeface.clone(), size));
                            self.fonts.last().expect("just pushed")
                        }
                    };
                    canvas.draw_str(text, (x, y), font, &paint);
                }
            } else {
                panic!("unknown display-list opcode {code} at {}", i - 1);
            }
        }
    }
}

fn rect(v: &[f32]) -> Rect {
    Rect::from_xywh(v[0], v[1], v[2], v[3])
}

fn rrect(v: &[f32]) -> RRect {
    RRect::new_rect_xy(rect(v), v[4], v[4])
}

fn color(v: &[f32]) -> Color4f {
    Color4f::new(v[0], v[1], v[2], v[3])
}

fn read_stroke(ops: &[f32], i: usize, paint: &mut Paint) -> usize {
    paint.set_stroke_width(ops[i]);
    paint.set_color4f(color(&ops[i + 1..i + 5]), None);
    i + 5
}

/// Reads `[blend, kind, args...]` into `paint` and returns the index after it.
fn read_paint(ops: &[f32], mut i: usize, paint: &mut Paint) -> usize {
    paint.set_blend_mode(match ops[i] as u8 {
        blend::MULTIPLY => BlendMode::Multiply,
        blend::SCREEN => BlendMode::Screen,
        _ => BlendMode::SrcOver,
    });
    let kind = ops[i + 1];
    i += 2;
    if kind == paint_kind::SOLID {
        paint.set_shader(None);
        paint.set_color4f(color(&ops[i..i + 4]), None);
        return i + 4;
    }
    let (shader, next) = if kind == paint_kind::LINEAR {
        let (from, to) = (
            Point::new(ops[i], ops[i + 1]),
            Point::new(ops[i + 2], ops[i + 3]),
        );
        let (colors, pos, next) = read_stops(ops, i + 4);
        let shader = shaders::linear_gradient((from, to), &gradient(&colors, &pos), None);
        (shader, next)
    } else {
        let (center, radius) = (Point::new(ops[i], ops[i + 1]), ops[i + 2]);
        let (colors, pos, next) = read_stops(ops, i + 3);
        let shader = shaders::radial_gradient((center, radius), &gradient(&colors, &pos), None);
        (shader, next)
    };
    paint.set_color4f(Color4f::new(0.0, 0.0, 0.0, 1.0), None);
    paint.set_shader(shader.map(|s: Shader| s));
    next
}

fn gradient<'a>(colors: &'a [Color4f], pos: &'a [f32]) -> Gradient<'a> {
    Gradient::new(
        Colors::new(colors, Some(pos), TileMode::Clamp, None),
        Interpolation::default(),
    )
}

fn read_stops(ops: &[f32], i: usize) -> (Vec<Color4f>, Vec<f32>, usize) {
    let n = ops[i] as usize;
    let mut colors = Vec::with_capacity(n);
    let mut pos = Vec::with_capacity(n);
    for k in 0..n {
        let s = &ops[i + 1 + k * 5..i + 6 + k * 5];
        pos.push(s[0]);
        colors.push(color(&s[1..5]));
    }
    (colors, pos, i + 1 + n * 5)
}
