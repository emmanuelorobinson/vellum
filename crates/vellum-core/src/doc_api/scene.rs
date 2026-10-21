//! Shared resolver/layout/scene/display-list reads over the current staged document.
use super::*;
use crate::{
    display_list::{self, Viewport},
    layout, layout_check,
    ports::*,
    resolver::{
        Resolver,
        libraries::{Libraries, LibrarySource},
    },
    scene::{self, Scene},
};
use std::sync::Arc;
pub struct Rendered {
    pub mime: String,
    pub bytes: Vec<u8>,
}
/// The composition root binds this to the editor's renderer. No file writes or resource fetching.
pub trait RenderRead {
    fn render_export(
        &self,
        _frames: &[crate::export::ExportFrame],
        _format: &str,
    ) -> Result<Vec<u8>, ExportError> {
        Err(ExportError::new("Export requires the full renderer"))
    }
    fn render(
        &self,
        output: &display_list::Output,
        bounds: Rect,
        scale: f64,
        format: &str,
    ) -> Result<Rendered, ScriptError>;
}
pub struct SceneConfig {
    /// Fresh successful derived outputs, scoped by document path. Refresh after source/input changes.
    pub script_outputs: BTreeMap<(String, NodeId), crate::script_nodes::Output>,
    pub path: String,
    pub libraries: Rc<dyn LibrarySource>,
    pub layout: RefCell<Box<dyn LayoutEngine>>,
    pub text: Rc<dyn TextEngine>,
    pub fonts: Rc<dyn FontSource>,
    pub renderer: Rc<dyn RenderRead>,
}
pub struct SceneServices<S> {
    config: SceneConfig,
    make_index: fn() -> S,
}
impl<S: SpatialIndex> SceneServices<S> {
    pub fn new(config: SceneConfig, make_index: fn() -> S) -> Self {
        Self { config, make_index }
    }
}
impl<S: SpatialIndex> SceneServices<S> {
    /// The resolved roots of the page or node `target` names, laid out.
    fn inspect(
        &self,
        doc: &Document,
        target: &str,
        method: Option<Method>,
    ) -> Result<(Vec<Arc<crate::resolver::ResolvedNode>>, LayoutResult), ScriptError> {
        let roots: Vec<Arc<crate::resolver::ResolvedNode>> = {
            let mut resolver = Resolver::new();
            resolver.set_script_outputs(self.config.script_outputs.clone());
            let mut libraries = Libraries::new(&*self.config.libraries);
            if let Some(page) = doc.pages.iter().find(|p| p.id.as_str() == target) {
                resolver.resolve_page(doc, &self.config.path, page, &mut libraries)
            } else {
                if method == Some(Method::FindSpace) {
                    return Err(error("findSpace requires a page"));
                }
                let id = NodeId::new(target).map_err(error)?;
                let location =
                    ops::locate(doc, &id).ok_or_else(|| error("Unknown scene target"))?;
                match location.tree {
                    ops::Tree::Page(page) => resolver.resolve_page(
                        doc,
                        &self.config.path,
                        doc.pages
                            .iter()
                            .find(|p| p.id == page)
                            .expect("located page"),
                        &mut libraries,
                    ),
                    ops::Tree::Component(component) => vec![resolver.resolve_node(
                        doc,
                        &self.config.path,
                        &doc.components[&component].root,
                        &mut libraries,
                    )],
                }
            }
        };
        let mut boxes = LayoutResult::default();
        for root in &roots {
            let tree = layout::from_resolved(root).map_err(error)?;
            let result =
                self.config
                    .layout
                    .borrow_mut()
                    .layout(&tree, &DirtySet::all(), &*self.config.text);
            boxes.boxes.extend(result.boxes);
            boxes.problems.extend(result.problems);
        }
        Ok((roots, boxes))
    }
}
impl<S: SpatialIndex> Services for SceneServices<S> {
    fn render_export(
        &self,
        frames: &[crate::export::ExportFrame],
        format: &str,
    ) -> Result<Vec<u8>, ExportError> {
        self.config.renderer.render_export(frames, format)
    }
    fn prepare_export(
        &self,
        doc: &Document,
        targets: &[String],
    ) -> Result<crate::export::ExportScene, ScriptError> {
        use crate::export::{ExportFrame, ExportScene};
        fn find(
            roots: &[Arc<crate::resolver::ResolvedNode>],
            target: &vellum_types::NodeIdPath,
        ) -> Option<Arc<crate::resolver::ResolvedNode>> {
            for root in roots {
                if &root.address == target {
                    return Some(root.clone());
                }
                if let Some(found) = find(&root.children, target) {
                    return Some(found);
                }
            }
            None
        }
        if targets.is_empty() || targets.len() > 100 {
            return Err(error("Select between 1 and 100 layers"));
        }
        let mut frames = vec![];
        for target in targets {
            let (roots, mut boxes) = self.inspect(doc, target, None)?;
            let (roots, name) = if let Some(page) =
                doc.pages.iter().find(|p| p.id.as_str() == target)
            {
                (roots, page.name.clone())
            } else {
                // Resolve the document's own ancestor chain: library-local ids can equal document ids.
                let mut cursor = NodeId::new(target).map_err(error)?;
                let mut chain = vec![cursor.clone()];
                while let Some(parent) = ops::locate(doc, &cursor).and_then(|p| p.parent) {
                    cursor = parent;
                    chain.push(cursor.clone());
                }
                let mut chain = chain.into_iter().rev();
                let mut address = vellum_types::NodeIdPath::from(chain.next().expect("the target"));
                for segment in chain {
                    address = address.join(segment);
                }
                let root =
                    find(&roots, &address).ok_or_else(|| error("No visible export target"))?;
                let b = boxes
                    .boxes
                    .get_mut(&root.address)
                    .ok_or_else(|| error("No visible export target"))?;
                b.x = 0.;
                b.y = 0.;
                let name = root.value.node.name().to_owned();
                (vec![root], name)
            };
            let mut scene = Scene::new((self.make_index)());
            let report =
                layout_check::check(&roots, &boxes, &*self.config.text, (self.make_index)())
                    .map_err(|e| error(format!("{e:?}")))?;
            let bounds = report
                .nodes
                .iter()
                .filter(|n| n.parent.is_none())
                .map(|n| n.bounds)
                .reduce(|a, b| a.union(&b))
                .ok_or_else(|| error("No visible export bounds"))?;
            let output = display_list::build(
                &roots,
                &boxes,
                &*self.config.text,
                &*self.config.fonts,
                &mut scene,
                Viewport { bounds, scale: 1. },
            )
            .map_err(|e| error(format!("{e:?}")))?;
            let mut frame = ExportFrame::empty(target, &name, bounds.width, bounds.height);
            frame.bounds = bounds;
            frame.roots = roots;
            frame.layout = boxes;
            frame.output = output;
            fn fonts(
                r: &crate::resolver::ResolvedNode,
                source: &dyn FontSource,
                faces: &mut Vec<crate::export::ExportFont>,
                doc: &Document,
                path: &str,
            ) {
                if !r.value.visible {
                    return;
                }
                if let Node::Text(t) = &r.value.node {
                    for run in &t.content {
                        if faces.iter().any(|f| {
                            f.family == run.font && f.weight == run.weight && f.style == run.style
                        }) {
                            continue;
                        }
                        if let Some(handle) = source.resolve(&run.font, run.weight, run.style)
                            && let Some(data) = source.data(handle)
                        {
                            faces.push(crate::export::ExportFont {
                                family: run.font.clone(),
                                weight: run.weight,
                                style: run.style,
                                data,
                                embed: r.source != path
                                    || doc.fonts.values().any(|f| f.family == run.font),
                            });
                        }
                    }
                }
                for child in &r.children {
                    fonts(child, source, faces, doc, path);
                }
            }
            for root in &frame.roots {
                fonts(
                    root,
                    &*self.config.fonts,
                    &mut frame.faces,
                    doc,
                    &self.config.path,
                );
            }
            fn render_text<S: SpatialIndex>(
                r: &Arc<crate::resolver::ResolvedNode>,
                frame: &mut ExportFrame,
                services: &SceneServices<S>,
            ) -> Result<(), ScriptError> {
                if !r.value.visible {
                    return Ok(());
                }
                if matches!(r.value.node, Node::Text(_))
                    && r.value.node.stroke().is_none()
                    && r.value.node.effects().is_empty()
                    && r.value
                        .node
                        .fills()
                        .iter()
                        .all(|p| matches!(p, vellum_types::Paint::Solid(_)))
                {
                    let b = frame.layout.boxes[&r.address];
                    if b.width > 0. && b.height > 0. {
                        let mut own = (**r).clone();
                        let mut value = (*own.value).clone();
                        if let Node::Text(t) = &mut value.node {
                            t.rotation = vellum_types::Value::Literal(0.);
                            t.flip_x = false;
                            t.flip_y = false;
                            t.opacity = vellum_types::Value::Literal(1.);
                        }
                        own.value = Arc::new(value);
                        let mut boxes = frame.layout.clone();
                        let mut extent = Extent::new(b.width, b.height);
                        let tree = layout::from_resolved(r).map_err(error)?;
                        let mut pad = 0.;
                        if let LayoutContent::Text(rich) = &tree.root.content {
                            let measured = services.config.text.layout_box(rich, extent);
                            extent.width = extent.width.max(measured.layout.width);
                            extent.height = extent.height.max(measured.layout.height);
                            pad = rich
                                .runs
                                .iter()
                                .map(|r| r.size * 2.)
                                .fold(0., f64::max)
                                .ceil();
                        }
                        let bounds = Rect::new(0., 0., extent.width, extent.height).expand(pad);
                        boxes
                            .boxes
                            .insert(r.address.clone(), Rect::new(0., 0., b.width, b.height));
                        let mut scene = Scene::new((services.make_index)());
                        let out = display_list::build(
                            &[Arc::new(own)],
                            &boxes,
                            &*services.config.text,
                            &*services.config.fonts,
                            &mut scene,
                            Viewport { bounds, scale: 1. },
                        )
                        .map_err(|e| error(format!("{e:?}")))?;
                        let image = services.config.renderer.render(&out, bounds, 1., "png")?;
                        frame.text_visuals.insert(
                            r.address.clone(),
                            crate::export::TextVisual {
                                bytes: image.bytes,
                                bounds,
                            },
                        );
                    }
                }
                for child in &r.children {
                    render_text(child, frame, services)?;
                }
                Ok(())
            }
            for root in frame.roots.clone() {
                render_text(&root, &mut frame, self)?;
            }
            frames.push(frame);
        }
        let scene = ExportScene { frames };
        scene.check(1.).map_err(error)?;
        Ok(scene)
    }

    fn font_families(&self) -> Vec<String> {
        self.config.fonts.families()
    }
    fn layout(&self, doc: &Document, target: &str) -> Result<layout_check::Report, ScriptError> {
        let (roots, boxes) = self.inspect(doc, target, None)?;
        layout_check::check(&roots, &boxes, &*self.config.text, (self.make_index)())
            .map_err(|e| error(format!("{e:?}")))
    }
    fn read(
        &self,
        method: Method,
        doc: &Document,
        args: &[ScriptValue],
    ) -> Result<ScriptValue, ScriptError> {
        let target = text(args, 0)?;
        let (roots, boxes) = self.inspect(doc, target, Some(method))?;
        let mut nodes =
            scene::from_resolved(&roots, &boxes).map_err(|e| error(format!("{e:?}")))?;
        if method == Method::FindSpace {
            nodes.extend(reservations(&to_json(arg(args, 1)?)?)?);
        }
        let mut scene = Scene::new((self.make_index)());
        scene.sync(&nodes).map_err(|e| error(format!("{e:?}")))?;
        match method {
            Method::FindSpace => {
                let size = to_json(arg(args, 1)?)?;
                let w = size
                    .get("width")
                    .and_then(Json::as_f64)
                    .ok_or_else(|| error("Expected width"))?;
                let h = size
                    .get("height")
                    .and_then(Json::as_f64)
                    .ok_or_else(|| error("Expected height"))?;
                let near = size
                    .get("near")
                    .map(|n| serde_json::from_value(n.clone()).map_err(error))
                    .transpose()?;
                let b = scene
                    .find_space(w, h, near)
                    .map_err(|e| error(format!("{e:?}")))?;
                from_json(json!({"x":b.x,"y":b.y,"width":b.width,"height":b.height}))
            }
            Method::LayoutProblems => {
                let report =
                    layout_check::check(&roots, &boxes, &*self.config.text, (self.make_index)())
                        .map_err(|e| error(format!("{e:?}")))?;
                let scope = report
                    .nodes
                    .iter()
                    .find(|n| n.node.last().as_str() == target)
                    .map(|n| n.node.to_string());
                from_json(Json::Array(
                    report
                        .problems
                        .iter()
                        .filter(|p| {
                            scope.as_ref().is_none_or(|scope| {
                                p.nodes().iter().any(|id| {
                                    let id = id.to_string();
                                    id == *scope || id.starts_with(&format!("{scope}/"))
                                })
                            })
                        })
                        .map(problem)
                        .collect(),
                ))
            }
            Method::Screenshot | Method::Export => {
                let options = args.get(1).map(to_json).transpose()?.unwrap_or(json!({}));
                if !options.is_object() {
                    return Err(error("Expected render options"));
                }
                let scale = match options.get("scale") {
                    None => 1.,
                    Some(v) => v.as_f64().ok_or_else(|| error("Invalid render scale"))?,
                };
                let format = if method == Method::Screenshot {
                    "png"
                } else {
                    options
                        .get("format")
                        .and_then(Json::as_str)
                        .ok_or_else(|| error("Expected export format"))?
                };
                if !matches!(format, "png" | "jpeg" | "webp" | "svg" | "pdf") {
                    return Err(error("Unsupported export format"));
                }
                // Recompute world bounds for nested transforms using the shared inspection path.
                let report =
                    layout_check::check(&roots, &boxes, &*self.config.text, (self.make_index)())
                        .map_err(|e| error(format!("{e:?}")))?;
                let is_page = doc.pages.iter().any(|p| p.id.as_str() == target);
                let bounds = report
                    .nodes
                    .iter()
                    .filter(|n| {
                        if is_page {
                            n.parent.is_none()
                        } else {
                            n.node.last().as_str() == target
                        }
                    })
                    .map(|n| n.bounds)
                    .reduce(|a, b| a.union(&b))
                    .ok_or_else(|| error("Target has no visible bounds"))?;
                let (w, h) = (
                    (bounds.width * scale).ceil(),
                    (bounds.height * scale).ceil(),
                );
                if !scale.is_finite()
                    || scale <= 0.
                    || w < 1.
                    || h < 1.
                    || w > 8192.
                    || h > 8192.
                    || w * h > 16_777_216.
                {
                    return Err(error("Render exceeds image bounds"));
                }
                let output = display_list::build(
                    &roots,
                    &boxes,
                    &*self.config.text,
                    &*self.config.fonts,
                    &mut scene,
                    Viewport { bounds, scale },
                )
                .map_err(|e| error(format!("{e:?}")))?;
                if !output.problems.is_empty() {
                    return Err(error(format!(
                        "Incomplete render: {}",
                        output
                            .problems
                            .iter()
                            .map(render_problem)
                            .collect::<Vec<_>>()
                            .join("; ")
                    )));
                }
                let image = self
                    .config
                    .renderer
                    .render(&output, bounds, scale, format)?;
                if image.bytes.len() > 700 * 1024 {
                    return Err(error("Rendered data exceeds the synchronous result budget"));
                }
                from_json(
                    json!({"mime":image.mime,"base64":base64(&image.bytes),"width":w,"height":h}),
                )
            }
            _ => Err(error("Unknown scene read")),
        }
    }
}
fn render_problem(problem: &display_list::Problem) -> String {
    use display_list::Problem::*;
    match problem {
        UnsupportedNode { node, kind } => {
            format!("the {} node {node} cannot be drawn yet", kind.as_str())
        }
        MissingEmbed { node } => format!("the embed {node} has no authorized browser preview"),
        MissingFontData { node, .. } => format!("the font of {node} has no data to draw with"),
        Text { node, problem } => format!("the text of {node} could not be laid out ({problem:?})"),
    }
}
/// A layout problem as the JSON `doc.layoutProblems` and `snapshot_layout` show.
pub fn problem(p: &layout_check::Problem) -> Json {
    use layout_check::Problem::*;
    match p {
        TextOverflow {
            node,
            horizontal,
            vertical,
        } => {
            json!({"kind":"textOverflow","node":node.to_string(),"horizontal":horizontal,"vertical":vertical})
        }
        ClippedChild { node, parent } => {
            json!({"kind":"clippedChild","node":node.to_string(),"parent":parent.to_string()})
        }
        OverlappingSiblings {
            parent,
            first,
            second,
        } => {
            json!({"kind":"overlappingSiblings","parent":parent.to_string(),"first":first.to_string(),"second":second.to_string()})
        }
        OutsideFrame { node } => json!({"kind":"outsideFrame","node":node.to_string()}),
        HugCycle {
            parent,
            child,
            axis,
        } => {
            json!({"kind":"hugCycle","parent":parent.to_string(),"child":child.to_string(),"axis":format!("{axis:?}")})
        }
        InvalidLayout(e) => json!({"kind":"invalidLayout","reason":e.to_string()}),
    }
}
fn base64(bytes: &[u8]) -> String {
    crate::base64::encode(bytes)
}

/// Areas `findSpace` must treat as taken besides the page's frames: planned-frame placeholders (session state,
/// never in the document), given as `reserved: [{x, y, width, height}]`. They block space but are never selectable.
fn reservations(size: &Json) -> Result<Vec<scene::SceneNode>, ScriptError> {
    let Some(list) = size.get("reserved") else {
        return Ok(vec![]);
    };
    let list = list
        .as_array()
        .ok_or_else(|| error("Expected reserved to be a list of rectangles"))?;
    list.iter()
        .enumerate()
        .map(|(n, rect)| {
            let side = |key: &str| {
                rect.get(key)
                    .and_then(Json::as_f64)
                    .ok_or_else(|| error(format!("Expected reserved[{n}].{key}")))
            };
            let address = NodeId::new(format!("--reserved-{n}")).map_err(error)?;
            let mut node = scene::SceneNode::default_for(address.into());
            node.bounds = Rect::new(side("x")?, side("y")?, side("width")?, side("height")?);
            node.selectable = false;
            Ok(node)
        })
        .collect()
}
