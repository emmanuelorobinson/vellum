//! Resolved page + layout → renderer-neutral versioned commands, culled through the scene R-tree.
//! Core owns no graphics context. Font bytes are immutable side resources shared by every backend.
mod shapes;
mod values;
use crate::{
    layout,
    ports::*,
    resolver::ResolvedNode,
    scene::{self, Scene, SceneError, geometry::Affine},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use values::*;
use vellum_types::{BlendMode, Node, NodeIdPath, NodeKind, Value, display::*};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Visible model-coordinate box. Its top-left maps to (0,0) device pixels.
    pub bounds: Rect,
    /// Zoom × device pixel ratio. All subsequent commands are under this device transform.
    pub scale: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Scene(SceneError),
    InvalidViewport,
    InvalidNumber,
    UnresolvedValue,
    UnsupportedGeometry(NodeIdPath),
    Buffer(BufferError),
}
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    UnsupportedNode {
        node: NodeIdPath,
        kind: NodeKind,
    },
    MissingEmbed {
        node: NodeIdPath,
    },
    MissingFontData {
        node: NodeIdPath,
        font: FontHandle,
    },
    Text {
        node: NodeIdPath,
        problem: TextProblem,
    },
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Output {
    pub list: DisplayList,
    /// Occurrences that issued own drawing commands, in paint order. Ancestor state isn't counted.
    pub painted: Vec<NodeIdPath>,
    pub fonts: BTreeMap<FontHandle, FontData>,
    pub problems: Vec<Problem>,
}
/// Synchronizes the scene first so a stale index can never fabricate a current viewport. Culling keeps
/// ancestors for state and conservatively includes paint outsets (stroke, effects and overflowing text).
/// It may overdraw near an outset. Runtime shaders disable culling until their bounds can be known.
pub fn build<S: SpatialIndex>(
    roots: &[Arc<ResolvedNode>],
    l: &LayoutResult,
    text: &dyn TextEngine,
    fonts: &dyn FontSource,
    scene: &mut Scene<S>,
    viewport: Viewport,
) -> Result<Output, Error> {
    build_with_embeds(roots, l, text, fonts, scene, viewport, &BTreeMap::new())
}
/// Host-supplied, permission-authorized preview assets. These are derived session data, never file content.
/// The caller supplies the encoded pixels to the renderer alongside the returned display list.
pub fn build_with_embeds<S: SpatialIndex>(
    roots: &[Arc<ResolvedNode>],
    l: &LayoutResult,
    text: &dyn TextEngine,
    fonts: &dyn FontSource,
    scene: &mut Scene<S>,
    viewport: Viewport,
    embeds: &BTreeMap<NodeIdPath, vellum_types::AssetId>,
) -> Result<Output, Error> {
    if !viewport.bounds.is_valid() || !viewport.scale.is_finite() || viewport.scale <= 0. {
        return Err(Error::InvalidViewport);
    }
    f(viewport.bounds.width * viewport.scale)?;
    f(viewport.bounds.height * viewport.scale)?;
    let nodes = scene::from_resolved(roots, l).map_err(Error::Scene)?;
    scene.sync(&nodes).map_err(Error::Scene)?;
    let mut texts = BTreeMap::new();
    let mut outset = 0.0_f64;
    let mut unbounded = false;
    fn inspect(
        r: &ResolvedNode,
        l: &LayoutResult,
        text: &dyn TextEngine,
        texts: &mut BTreeMap<NodeIdPath, TextBoxLayout>,
        outset: &mut f64,
        unbounded: &mut bool,
        inherited: f64,
    ) -> Result<(), Error> {
        if !r.value.visible {
            return Ok(());
        }
        if r.problems.iter().any(|p| {
            matches!(
                p,
                crate::resolver::components::Problem::Component(_)
                    | crate::resolver::components::Problem::Script { .. }
            )
        }) {
            return Err(Error::UnresolvedValue);
        }
        let n = &r.value.node;
        // The viewBox is a mapping, not a path bound; SVG can draw outside it.
        if matches!(n, Node::Vector(_)) {
            *unbounded = true;
        }
        let mut pad = 0.0_f64;
        if let Some(s) = n.stroke() {
            let s = stroke(s)?;
            pad = s.widths.into_iter().map(f64::from).fold(0., f64::max) * 4.;
        }
        for e in n.effects() {
            if let Some(e) = effect(e)? {
                pad += match e {
                    RenderEffect::DropShadow(s) => f64::from(
                        s.offset[0].abs().max(s.offset[1].abs()) + s.blur * 3. + s.spread.abs(),
                    ),
                    RenderEffect::LayerBlur(v) | RenderEffect::BackgroundBlur(v) => {
                        f64::from(v) * 3.
                    }
                    RenderEffect::Shader { .. } => {
                        *unbounded = true;
                        0.
                    }
                    _ => 0.,
                };
            }
        }
        if n.fills()
            .iter()
            .any(|p| matches!(p,vellum_types::Paint::Shader(p) if p.visible))
        {
            *unbounded = true;
        }
        if matches!(n, Node::Text(_)) {
            let tree = layout::from_resolved(r).map_err(|_| Error::UnresolvedValue)?;
            if let LayoutContent::Text(rich) = &tree.root.content {
                let b = l.boxes[&r.address];
                let t = text.layout_box(rich, Extent::new(b.width, b.height));
                pad += (t.layout.width - b.width).max(0.) + (t.layout.height - b.height).max(0.);
                texts.insert(r.address.clone(), t);
            }
        }
        *outset = (*outset).max(pad + inherited);
        for c in &r.children {
            inspect(c, l, text, texts, outset, unbounded, inherited + pad)?;
        }
        Ok(())
    }
    for r in roots {
        inspect(r, l, text, &mut texts, &mut outset, &mut unbounded, 0.)?;
    }
    // Rotation can put both horizontal and vertical outsets on either world axis.
    let expanded = viewport.bounds.expand(outset * std::f64::consts::SQRT_2);
    if !expanded.is_valid() {
        return Err(Error::InvalidNumber);
    }
    let mut needed: BTreeSet<_> = if unbounded {
        nodes.iter().map(|n| n.address.clone()).collect()
    } else if outset > 0. {
        // A fully clipped layout box may still paint a shadow/stroke inside its ancestor's clip.
        // Query unclipped bounds conservatively, then restore the normal scene before returning.
        let mut raw = nodes.clone();
        for n in &mut raw {
            n.clip = false;
        }
        scene.sync(&raw).map_err(Error::Scene)?;
        let candidates = scene.cull(expanded).into_iter().collect();
        scene.sync(&nodes).map_err(Error::Scene)?;
        candidates
    } else {
        scene.cull(expanded).into_iter().collect()
    };
    let parents: BTreeMap<_, _> = nodes
        .iter()
        .map(|n| (&n.address, n.parent.as_ref()))
        .collect();
    for id in needed.clone() {
        let mut p = parents[&id];
        while let Some(id) = p {
            needed.insert(id.clone());
            p = parents[id];
        }
    }
    let mut b = Builder {
        out: Output::default(),
        layout: l,
        texts: &texts,
        fonts,
        needed: &needed,
        viewport,
        embeds,
    };
    b.out
        .list
        .commands
        .extend([DrawCmd::Save, DrawCmd::Transform([1., 0., 0., 1., 0., 0.])]);
    // The device-space viewport clip also bounds the final composite of all effects.
    let clip = b.resource(Resource::Shape(Shape::Rect {
        size: [
            f(viewport.bounds.width * viewport.scale)?,
            f(viewport.bounds.height * viewport.scale)?,
        ],
        radii: [0.; 4],
    }))?;
    b.out.list.commands.push(DrawCmd::Clip { shape: clip });
    for root in roots {
        b.node(root, Affine::IDENTITY)?;
    }
    b.out.list.commands.push(DrawCmd::Restore);
    b.out.list.validate().map_err(Error::Buffer)?;
    Ok(b.out)
}
struct Builder<'a> {
    out: Output,
    layout: &'a LayoutResult,
    texts: &'a BTreeMap<NodeIdPath, TextBoxLayout>,
    fonts: &'a dyn FontSource,
    needed: &'a BTreeSet<NodeIdPath>,
    viewport: Viewport,
    embeds: &'a BTreeMap<NodeIdPath, vellum_types::AssetId>,
}
impl Builder<'_> {
    fn resource(&mut self, r: Resource) -> Result<u32, Error> {
        // Resources are deterministic in first-use order; no hash iteration enters the ABI.
        let id = u32::try_from(self.out.list.resources.len())
            .map_err(|_| Error::Buffer(BufferError::TooLarge))?;
        self.out.list.resources.push(r);
        Ok(id)
    }
    fn node(&mut self, r: &ResolvedNode, parent: Affine) -> Result<(), Error> {
        if !r.value.visible || !self.needed.contains(&r.address) {
            return Ok(());
        }
        let n = &r.value.node;
        let bounds = self.layout.boxes[&r.address];
        let rotation = match n.rotation() {
            Value::Literal(v) => *v,
            _ => return Err(Error::UnresolvedValue),
        };
        let world = parent.then(Affine::local(bounds, rotation, n.flip_x(), n.flip_y()));
        let mut device = world.0;
        // Rebase in f64 before conversion: a 1px offset at 10^12px must survive pan and zoom.
        device[4] -= self.viewport.bounds.x;
        device[5] -= self.viewport.bounds.y;
        for scalar in &mut device {
            *scalar *= self.viewport.scale;
        }
        self.out
            .list
            .commands
            .extend([DrawCmd::Save, DrawCmd::Transform(matrix(device)?)]);
        let opacity = number(n.opacity())?;
        if !(0. ..=1.).contains(&opacity) {
            return Err(Error::InvalidNumber);
        }
        if opacity == 0. {
            self.out.list.commands.push(DrawCmd::Restore);
            return Ok(());
        }
        let effects = n
            .effects()
            .iter()
            .map(effect)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let layer = opacity != 1. || n.blend_mode() != BlendMode::Normal || !effects.is_empty();
        if layer {
            self.out.list.commands.push(DrawCmd::Layer {
                opacity,
                blend: n.blend_mode(),
            });
        }
        for effect in effects {
            let effect = self.resource(Resource::Effect(effect))?;
            self.out.list.commands.push(DrawCmd::Effect { effect });
        }
        let before = self.out.list.commands.len();
        // A group's bounds are structural. Boolean group operands are handled inside the shape resource.
        let shape = if matches!(n, Node::Group(_)) {
            None
        } else {
            shapes::shape(r, self.layout)?
        };
        let shape = shape
            .map(|s| self.resource(Resource::Shape(s)))
            .transpose()?;
        let glyphs = if let Some(t) = self.texts.get(&r.address) {
            for problem in &t.layout.problems {
                self.out.problems.push(Problem::Text {
                    node: r.address.clone(),
                    problem: problem.clone(),
                });
            }
            let mut glyphs = vec![];
            for line in &t.layout.lines {
                for run in &line.glyph_runs {
                    if let Some(data) = self.fonts.data(run.font) {
                        self.out.fonts.insert(run.font, data);
                    } else {
                        let p = Problem::MissingFontData {
                            node: r.address.clone(),
                            font: run.font,
                        };
                        if !self.out.problems.contains(&p) {
                            self.out.problems.push(p)
                        }
                    }
                    let decoration =
                        |d: TextDecoration| Ok::<_, Error>([f(d.offset)?, f(d.thickness)?]);
                    let g = RenderGlyphRun {
                        font: run.font.0,
                        size: f(run.size)?,
                        glyphs: run
                            .glyphs
                            .iter()
                            .map(|g| {
                                Ok(RenderGlyph {
                                    id: g.id,
                                    cluster: u32::try_from(g.cluster)
                                        .map_err(|_| Error::Buffer(BufferError::TooLarge))?,
                                    position: [f(line.x + g.x)?, f(line.y + line.baseline + g.y)?],
                                    advance: f(g.advance)?,
                                })
                            })
                            .collect::<Result<_, Error>>()?,
                        normalized_coords: run.normalized_coords.clone(),
                        embolden: run.embolden,
                        skew: run.skew.map(f).transpose()?,
                        underline: run.underline.map(decoration).transpose()?,
                        strikethrough: run.strikethrough.map(decoration).transpose()?,
                        link: run.link.clone(),
                    };
                    glyphs.push(self.resource(Resource::Glyphs(g))?);
                }
            }
            glyphs
        } else {
            vec![]
        };
        for p in n.fills().iter().filter(|p| p.visible()) {
            let paint = self.resource(Resource::Paint(paint(p)?))?;
            self.draw(shape, &glyphs, paint, None);
        }
        if let Node::Image(n) = n {
            let image = self.resource(Resource::Image(RenderImage {
                asset: n.asset.clone(),
                fit: n.fit,
                crop: n.crop.map(|m| matrix(m.0)).transpose()?,
                opacity: 1.,
            }))?;
            self.out.list.commands.push(DrawCmd::Image {
                image,
                dst: [0., 0., f(bounds.width)?, f(bounds.height)?],
            });
        }
        if let Node::Embed(_) = n {
            if let Some(asset) = self.embeds.get(&r.address) {
                self.out.list.commands.push(DrawCmd::Save);
                self.out.list.commands.push(DrawCmd::Clip {
                    shape: shape.ok_or(Error::UnresolvedValue)?,
                });
                let image = self.resource(Resource::Image(RenderImage {
                    asset: asset.clone(),
                    fit: vellum_types::ImageFit::Stretch,
                    crop: None,
                    opacity: 1.,
                }))?;
                self.out.list.commands.push(DrawCmd::Image {
                    image,
                    dst: [0., 0., f(bounds.width)?, f(bounds.height)?],
                });
                self.out.list.commands.push(DrawCmd::Restore);
            } else {
                self.out.problems.push(Problem::MissingEmbed {
                    node: r.address.clone(),
                });
            }
        }
        if let Some(s) = n.stroke() {
            let stroke = self.resource(Resource::Stroke(stroke(s)?))?;
            for p in s.paints.iter().filter(|p| p.visible()) {
                let paint = self.resource(Resource::Paint(paint(p)?))?;
                self.draw(shape, &glyphs, paint, Some(stroke));
            }
        }
        if self.out.list.commands[before..].iter().any(|c| {
            matches!(
                c,
                DrawCmd::Path { .. } | DrawCmd::GlyphRun { .. } | DrawCmd::Image { .. }
            )
        }) {
            self.out.painted.push(r.address.clone());
        }
        // Notes, context and prompts are annotations: the editor draws them as overlays, and exports and
        // screenshots leave them out (reports.md, Prompt 29, decided 2026-10-22).
        if shape.is_none()
            && !matches!(
                n,
                Node::Text(_) | Node::Group(_) | Node::Note(_) | Node::Context(_) | Node::Prompt(_)
            )
        {
            self.out.problems.push(Problem::UnsupportedNode {
                node: r.address.clone(),
                kind: n.kind(),
            });
        }
        if matches!(n,Node::Frame(n) if n.clip) | matches!(n,Node::Script(n) if n.clip) {
            self.out.list.commands.push(DrawCmd::Clip {
                shape: shape.ok_or(Error::UnresolvedValue)?,
            });
        }
        if !matches!(n, Node::Boolean(_)) {
            for c in &r.children {
                self.node(c, world)?;
            }
        }
        if layer {
            self.out.list.commands.push(DrawCmd::Restore);
        }
        self.out.list.commands.push(DrawCmd::Restore);
        Ok(())
    }
    fn draw(&mut self, shape: Option<u32>, glyphs: &[u32], paint: u32, stroke: Option<u32>) {
        if let Some(shape) = shape {
            self.out.list.commands.push(DrawCmd::Path {
                shape,
                paint,
                stroke,
            });
        }
        for glyphs in glyphs {
            self.out.list.commands.push(DrawCmd::GlyphRun {
                glyphs: *glyphs,
                paint,
                stroke,
            });
        }
    }
}
