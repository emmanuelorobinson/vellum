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
impl<S: SpatialIndex> Services for SceneServices<S> {
    fn read(
        &self,
        method: Method,
        doc: &Document,
        args: &[ScriptValue],
    ) -> Result<ScriptValue, ScriptError> {
        let target = text(args, 0)?;
        let roots: Vec<Arc<crate::resolver::ResolvedNode>> = {
            let mut resolver = Resolver::new();
            resolver.set_script_outputs(self.config.script_outputs.clone());
            let mut libraries = Libraries::new(&*self.config.libraries);
            if let Some(page) = doc.pages.iter().find(|p| p.id.as_str() == target) {
                resolver.resolve_page(doc, &self.config.path, page, &mut libraries)
            } else {
                if method == Method::FindSpace {
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
        let mut scene = Scene::new((self.make_index)());
        scene
            .sync(&scene::from_resolved(&roots, &boxes).map_err(|e| error(format!("{e:?}")))?)
            .map_err(|e| error(format!("{e:?}")))?;
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
                if !matches!(format, "png" | "svg" | "pdf") {
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
                    return Err(error(format!("Incomplete render: {:?}", output.problems)));
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
fn problem(p: &layout_check::Problem) -> Json {
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
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let a = c[0] as usize;
        let b = c.get(1).copied().unwrap_or(0) as usize;
        let d = c.get(2).copied().unwrap_or(0) as usize;
        out.push(CHARS[a >> 2] as char);
        out.push(CHARS[((a & 3) << 4) | (b >> 4)] as char);
        out.push(if c.len() > 1 {
            CHARS[((b & 15) << 2) | (d >> 6)] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            CHARS[d & 63] as char
        } else {
            '='
        });
    }
    out
}
