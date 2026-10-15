use crate::{Error, Resources};
use skia_safe::{self as sk, Canvas, Data, Font, FontMgr, Paint, Point, Rect};
use vellum_core::ports::FontHandle;
use vellum_types::display::RenderGlyphRun;

pub(crate) struct Glyphs {
    pub font: Font,
    pub ids: Vec<u16>,
    pub positions: Vec<Point>,
    pub decorations: Vec<Rect>,
}
impl Glyphs {
    pub fn draw(&self, c: &Canvas, p: &Paint) {
        c.draw_glyphs_at(
            &self.ids,
            self.positions.as_slice(),
            Point::default(),
            &self.font,
            p,
        );
        for r in &self.decorations {
            c.draw_rect(*r, p);
        }
    }
}
pub(crate) fn prepare(
    mgr: &FontMgr,
    r: &RenderGlyphRun,
    resources: &Resources,
) -> Result<Glyphs, Error> {
    if r.size <= 0. {
        return Err(Error::Geometry);
    }
    let data = resources
        .fonts
        .get(&FontHandle(r.font))
        .ok_or(Error::MissingFont(r.font))?;
    let face = mgr
        .new_from_data(Data::new_copy(data.bytes.as_ref()), Some(data.index))
        .ok_or(Error::InvalidFont(r.font))?;
    let face = if r.normalized_coords.is_empty() {
        face
    } else {
        let axes = face
            .variation_design_parameters()
            .ok_or(Error::InvalidFont(r.font))?;
        if axes.len() != r.normalized_coords.len() {
            return Err(Error::InvalidFont(r.font));
        }
        let avar = face.copy_table_data(u32::from_be_bytes(*b"avar"));
        let coords = design_coords(
            &axes,
            &r.normalized_coords,
            avar.as_ref().map(|d| d.as_bytes()),
        )
        .ok_or(Error::InvalidFont(r.font))?;
        face.clone_with_arguments(&sk::FontArguments::new().set_variation_design_position(
            sk::font_arguments::VariationPosition {
                coordinates: &coords,
            },
        ))
        .ok_or(Error::InvalidFont(r.font))?
    };
    let count = face.count_glyphs();
    let ids: Vec<_> = r
        .glyphs
        .iter()
        .map(|g| {
            if (g.id as usize) < count {
                u16::try_from(g.id).map_err(|_| Error::InvalidFont(r.font))
            } else {
                Err(Error::InvalidFont(r.font))
            }
        })
        .collect::<Result<_, _>>()?;
    let mut font = Font::from_typeface(face, r.size);
    font.set_edging(sk::font::Edging::AntiAlias);
    font.set_subpixel(true);
    font.set_hinting(sk::FontHinting::None);
    font.set_linear_metrics(true);
    font.set_embolden(r.embolden);
    font.set_skew_x(r.skew.map_or(0., |v| -v.to_radians().tan()));
    let positions: Vec<_> = r
        .glyphs
        .iter()
        .map(|g| Point::new(g.position[0], g.position[1]))
        .collect();
    let mut decorations = vec![];
    if let Some(first) = r.glyphs.first() {
        let left = r
            .glyphs
            .iter()
            .map(|g| g.position[0])
            .fold(f32::INFINITY, f32::min);
        let right = r
            .glyphs
            .iter()
            .map(|g| g.position[0] + g.advance)
            .fold(f32::NEG_INFINITY, f32::max);
        for [offset, thickness] in [r.underline, r.strikethrough].into_iter().flatten() {
            if thickness < 0. {
                return Err(Error::Geometry);
            }
            decorations.push(Rect::from_xywh(
                left,
                first.position[1] + offset,
                right - left,
                thickness,
            ));
        }
    }
    Ok(Glyphs {
        font,
        ids,
        positions,
        decorations,
    })
}
/// Parley provides post-avar normalized positions. Invert avar v1 before mapping through fvar axes.
/// Unknown avar versions fail explicitly rather than drawing a different variable-font instance.
fn design_coords(
    axes: &[sk::font_parameters::VariationAxis],
    normalized: &[i16],
    avar: Option<&[u8]>,
) -> Option<Vec<sk::font_arguments::variation_position::Coordinate>> {
    let mut at = 8;
    let word = |b: &[u8], i: usize| Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?));
    if let Some(b) = avar
        && (word(b, 0)? != 1 || word(b, 6)? as usize != axes.len())
    {
        return None;
    }
    axes.iter()
        .zip(normalized)
        .map(|(a, n)| {
            let mut n = f32::from(*n) / 16384.;
            if !(-1. ..=1.).contains(&n) {
                return None;
            }
            if let Some(b) = avar {
                let count = word(b, at)? as usize;
                at += 2;
                let mut pairs = Vec::with_capacity(count);
                for _ in 0..count {
                    let from = word(b, at)? as i16 as f32 / 16384.;
                    let to = word(b, at + 2)? as i16 as f32 / 16384.;
                    at += 4;
                    pairs.push((from, to));
                }
                if count < 3
                    || pairs
                        .windows(2)
                        .any(|p| p[1].0 <= p[0].0 || p[1].1 < p[0].1)
                {
                    return None;
                }
                let pair = pairs.windows(2).find(|p| n >= p[0].1 && n <= p[1].1)?;
                n = if pair[0].1 == pair[1].1 {
                    pair[0].0
                } else {
                    pair[0].0 + (n - pair[0].1) / (pair[1].1 - pair[0].1) * (pair[1].0 - pair[0].0)
                };
            }
            Some(sk::font_arguments::variation_position::Coordinate {
                axis: a.tag,
                value: if n < 0. {
                    a.def + n * (a.def - a.min)
                } else {
                    a.def + n * (a.max - a.def)
                },
            })
        })
        .collect()
}
