//! Tools that look at the laid-out design: `snapshot_layout`, `find_space`, `screenshot`.
//!
//! Layout and free space are computed from the document by the daemon's own scene, the same one `doc.layoutProblems`
//! uses, so they answer the same for a live file and a headless one. A screenshot is drawn by whoever holds the
//! file: the editor's renderer when one has it open (Invariant 6).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::doc_api::{Method, from_json, problem_json, to_json};
use vellum_core::layout_check::Problem;
use vellum_core::ports::{Rect, ScriptValue};
use vellum_types::{Document, NodeIdPath};

use crate::daemon::Documents;

use super::support::{Index, parse_input, schema_of};
use super::{ErrorKind, Tool, ToolCtx, ToolError};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(SnapshotLayout));
    tools.push(Box::new(FindSpace));
    tools.push(Box::new(Screenshot));
}

/// The most nodes `snapshot_layout` lists unless asked for more.
const DEFAULT_MAX_NODES: usize = 300;
const MAX_NODES: usize = 2_000;

fn scene_error(error: impl std::fmt::Display) -> ToolError {
    ToolError::new(ErrorKind::Render, error.to_string())
}

/// Whether `path` is just a node's ancestors from the top of its tree, so the node's own id names it. A path
/// through an instance's contents is not: those nodes exist only inside the instance.
fn is_plain(index: &Index<'_>, path: &NodeIdPath) -> bool {
    let Some(at) = index.position(path.last().as_str()) else {
        return false;
    };
    let mut chain: Vec<&str> = index
        .ancestors_or_self(at)
        .into_iter()
        .map(|a| index.entries[a].node.id().as_str())
        .collect();
    chain.reverse();
    chain.len() == path.len()
        && chain
            .iter()
            .zip(path.segments())
            .all(|(a, b)| *a == b.as_str())
}

/// A problem as JSON, naming nodes by id where the id is enough.
fn plain_problem(index: &Index<'_>, problem: &Problem) -> Value {
    let mut out = problem_json(problem);
    if let Value::Object(map) = &mut out {
        for key in ["node", "parent", "first", "second", "child"] {
            let Some(Value::String(text)) = map.get(key) else {
                continue;
            };
            if let Some(path) = problem.nodes().into_iter().find(|p| p.to_string() == *text)
                && is_plain(index, path)
            {
                map.insert(key.to_owned(), json!(path.last().as_str()));
            }
        }
    }
    out
}

struct SnapshotLayout;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LayoutInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// A page or node id. Defaults to the first page.
    node: Option<String>,
    /// Only the nodes that have a layout problem, and the problems: text that overflows, children a frame clips,
    /// siblings that overlap, nodes outside their frame.
    #[serde(default)]
    problems_only: bool,
    /// The most nodes to list (default 300, at most 2000). Past it, `truncated` is true.
    max_nodes: Option<u32>,
}

impl Tool for SnapshotLayout {
    fn name(&self) -> &'static str {
        "snapshot_layout"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Show the computed layout of a page or node: each node's parent and bounds in page coordinates, and the \
         layout problems found (text overflow, clipped children, overlapping siblings, nodes outside their \
         frame). With `problems_only`, list just the problems and the nodes they involve. Use it after a change \
         to check the result without drawing it."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<LayoutInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let LayoutInput {
            document,
            node,
            problems_only,
            max_nodes,
        } = parse_input(input)?;
        let max_nodes = max_nodes.map_or(DEFAULT_MAX_NODES, |n| n as usize);
        if !(1..=MAX_NODES).contains(&max_nodes) {
            return Err(ToolError::invalid(
                "max_nodes",
                format!("max_nodes is between 1 and {MAX_NODES}."),
            ));
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let target = match node {
            Some(target) => target,
            None => doc
                .pages
                .first()
                .map(|page| page.id.as_str().to_owned())
                .ok_or_else(|| ToolError::not_found("The document has no pages."))?,
        };
        let index = Index::build(&doc);
        let scope: BTreeSet<&str> = index
            .scope(&target)
            .ok_or_else(|| {
                ToolError::not_found(format!("There is no page, component or node {target:?}."))
            })?
            .map(|at| index.entries[at].node.id().as_str())
            .collect();
        let report = ctx
            .docs()
            .services()
            .layout(&doc, &target)
            .map_err(scene_error)?;
        // The report covers the whole page the target is on; keep what is inside the target.
        let mut report = report;
        let inside = |path: &NodeIdPath| path.segments().any(|id| scope.contains(id.as_str()));
        report.nodes.retain(|n| inside(&n.node));
        report
            .problems
            .retain(|p| p.nodes().into_iter().any(inside));
        if problems_only {
            report = report.problems_only();
        }
        let truncated = report.nodes.len() > max_nodes;
        let nodes: Vec<Value> = report
            .nodes
            .iter()
            .take(max_nodes)
            .map(|n| {
                let id = n.node.last().as_str();
                let mut out = json!({"id": id});
                if let Some(at) = index.position(id) {
                    out["name"] = json!(index.entries[at].node.name());
                    out["type"] = json!(index.entries[at].node.kind().as_str());
                }
                if !is_plain(&index, &n.node) {
                    out["path"] = json!(n.node.to_string());
                }
                if let Some(parent) = &n.parent {
                    out["parent"] = json!(parent.last().as_str());
                }
                out["bounds"] = json!({"x": n.bounds.x, "y": n.bounds.y, "width": n.bounds.width, "height": n.bounds.height});
                out
            })
            .collect();
        Ok(json!({
            "document": path,
            "target": target,
            "nodes": nodes,
            "problems": report.problems.iter().map(|p| plain_problem(&index, p)).collect::<Vec<_>>(),
            "truncated": truncated,
        }))
    }
}

struct FindSpace;

#[derive(Deserialize, JsonSchema)]
struct Point {
    /// Canvas x.
    x: f64,
    /// Canvas y.
    y: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SpaceInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The page to look on. Defaults to the first page.
    page: Option<String>,
    /// The width of the area needed.
    width: f64,
    /// The height of the area needed.
    height: f64,
    /// Prefer the free area nearest this point.
    near: Option<Point>,
}

impl Tool for FindSpace {
    fn name(&self) -> &'static str {
        "find_space"
    }
    fn version(&self) -> u32 {
        2
    }
    fn description(&self) -> &'static str {
        "Find an empty area of the canvas of a given size, clear of every frame already on the page and of every \
         planned frame an agent has reserved. Use it to place a new top-level frame instead of guessing \
         coordinates. Returns the rectangle."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<SpaceInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let SpaceInput {
            document,
            page,
            width,
            height,
            near,
        } = parse_input(input)?;
        for (field, value) in [("width", width), ("height", height)] {
            if !value.is_finite() || value <= 0.0 {
                return Err(ToolError::invalid(
                    field,
                    format!("{field} is a number above 0."),
                ));
            }
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let page = match page {
            Some(page) => page,
            None => doc
                .pages
                .first()
                .map(|page| page.id.as_str().to_owned())
                .ok_or_else(|| ToolError::not_found("The document has no pages."))?,
        };
        if !doc.pages.iter().any(|p| p.id.as_str() == page) {
            return Err(ToolError::not_found(format!(
                "There is no page {page:?}. The pages are: {}.",
                doc.pages
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let (_, rect) = free_space(
            ctx.docs(),
            &path,
            &doc,
            &page,
            (width, height),
            near.map(|p| (p.x, p.y)),
            &[],
        )?;
        Ok(json!({"document": path, "page": page, "rect": rect}))
    }
}

struct Screenshot;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ShotInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The page or node to draw.
    node: String,
    /// Pixels per canvas unit, above 0 and at most 4 (default 1).
    scale: Option<f64>,
}

impl Tool for Screenshot {
    fn name(&self) -> &'static str {
        "screenshot"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Draw a page or node to a PNG you can look at. It is drawn the way the editor draws it. Use it to check \
         your work; large areas at high scale are refused, so draw one frame at a time."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ShotInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ShotInput {
            document,
            node,
            scale,
        } = parse_input(input)?;
        let scale = scale.unwrap_or(1.0);
        if !scale.is_finite() || scale <= 0.0 || scale > 4.0 {
            return Err(ToolError::invalid(
                "scale",
                "scale is above 0 and at most 4.",
            ));
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let known = Index::build(&doc).scope(&node).is_some();
        if !known {
            return Err(ToolError::not_found(format!(
                "There is no page or node {node:?} to draw."
            )));
        }
        fn has_shader(nodes: &[vellum_types::Node]) -> bool {
            nodes
                .iter()
                .any(|n| !vellum_core::shaders::sources(n).is_empty() || has_shader(n.children()))
        }
        if doc.pages.iter().any(|p| has_shader(&p.children)) {
            let services = ctx.docs().services();
            let scene = ctx
                .docs()
                .export_scene(&path, &doc, std::slice::from_ref(&node))
                .map_err(scene_error)?;
            let frames: Vec<_> = scene.frames.iter().map(|f| f.at_scale(scale)).collect();
            let (width, height) = frames[0].size();
            if width > 8192 || height > 8192 || u64::from(width) * u64::from(height) > 16_777_216 {
                return Err(scene_error("Render exceeds image bounds"));
            }
            let bytes = services
                .render_export(&frames, "png")
                .map_err(scene_error)?;
            if bytes.len() > 700 * 1024 {
                return Err(scene_error(
                    "Rendered data exceeds the synchronous result budget",
                ));
            }
            return Ok(
                json!({"document":path,"node":node,"scale":scale,"image":{"mime":"image/png","base64":vellum_core::base64::encode(&bytes),"width":width,"height":height}}),
            );
        }
        let image = ctx.target(&path)?.render(&node, scale)?;
        let image = to_json(&image).map_err(scene_error)?;
        Ok(json!({"document": path, "node": node, "scale": scale, "image": image}))
    }
}

/// A free area of `size` on `page` of the document at `path`, clear of its frames, of every placeholder standing
/// there, and of `also` (placeholders about to be placed). What `find_space` answers and what Build places with;
/// also returned as the JSON `find_space` has always answered with.
pub(crate) fn free_space(
    docs: &mut Documents,
    path: &str,
    doc: &Document,
    page: &str,
    (width, height): (f64, f64),
    near: Option<(f64, f64)>,
    also: &[Rect],
) -> Result<(Rect, Value), ToolError> {
    let reserved: Vec<Value> = docs
        .placeholders(path)
        .iter()
        .map(|planned| &planned.placeholder)
        .filter(|p| p.page.as_str() == page)
        .map(|p| p.bounds)
        .chain(also.iter().copied())
        .map(|b| json!({"x": b.x, "y": b.y, "width": b.width, "height": b.height}))
        .collect();
    let mut size = json!({"width": width, "height": height});
    if let Some((x, y)) = near {
        size["near"] = json!({"x": x, "y": y});
    }
    if !reserved.is_empty() {
        size["reserved"] = json!(reserved);
    }
    let rect = docs
        .services()
        .read(
            Method::FindSpace,
            doc,
            &[
                ScriptValue::Text(page.to_owned()),
                from_json(size).map_err(scene_error)?,
            ],
        )
        .map_err(scene_error)?;
    let rect = to_json(&rect).map_err(scene_error)?;
    let side = |key: &str| {
        rect[key]
            .as_f64()
            .ok_or_else(|| ToolError::new(ErrorKind::Internal, "find_space returned no rectangle"))
    };
    Ok((
        Rect::new(side("x")?, side("y")?, side("width")?, side("height")?),
        rect,
    ))
}
