//! Local image processing changes the selected layer in one attributed transaction.
use super::batch::{Batch, Label, ids_in_use, new_node, node_id};
use super::{ErrorKind, Tool, ToolCtx, ToolError, Verb, parse_input, schema_of};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::{
    ops::Op,
    ports::{AssetHash, ImportInput, Importer},
};
use vellum_types::NodePatch;
pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(ImageWork));
}
struct ImageWork;
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    document: Option<String>,
    node: String,
    action: Action,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Action {
    RemoveBackground,
    Trace,
}
impl Tool for ImageWork {
    fn name(&self) -> &'static str {
        "image_ops"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Remove an image background using the configured local ONNX model, or trace flat artwork into editable vectors with vtracer. Uses the selected image layer or a shape with one image fill, preserves its id and placement, and commits one undoable attributed transaction. No network or automatic model downloads."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<Input>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::AddedAsset
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, value: Value) -> Result<Value, ToolError> {
        let i: Input = parse_input(value)?;
        let (path, doc) = ctx.document(i.document.as_deref())?;
        let id = node_id(&i.node, "node")?;
        let target = vellum_core::ops::find(&doc, &id)
            .ok_or_else(|| ToolError::not_found("The selected layer is unavailable."))?;
        let raw = serde_json::to_value(target)
            .map_err(|_| ToolError::new(ErrorKind::Internal, "The layer could not be read."))?;
        if raw["children"].as_array().is_some_and(|a| !a.is_empty()) {
            return Err(ToolError::invalid("node", "Select a leaf image or shape."));
        }
        let asset = if raw["type"] == "image" {
            raw["asset"].as_str()
        } else {
            let fills = raw["fills"].as_array().ok_or_else(|| {
                ToolError::invalid(
                    "node",
                    "Select an image layer or a shape with one image fill.",
                )
            })?;
            if fills.len() != 1 || fills[0]["type"] != "image" {
                return Err(ToolError::invalid(
                    "node",
                    "Select a shape with exactly one image fill.",
                ));
            }
            fills[0]["asset"].as_str()
        }
        .ok_or_else(|| ToolError::invalid("node", "The layer has no stored image."))?;
        let hash = AssetHash::new(asset)
            .map_err(|_| ToolError::invalid("node", "The image asset is invalid."))?;
        let bytes = ctx.docs().get_asset(&path, &hash)?;
        let ops = ctx.docs().image_ops().ok_or_else(|| {
            ToolError::new(ErrorKind::Refused, "Local image processing is unavailable.")
        })?;
        let mut batch = Batch::default();
        let mut notes = vec![];
        match i.action {
            Action::RemoveBackground => {
                let r = ops
                    .remove_background(&bytes)
                    .map_err(|e| ToolError::new(ErrorKind::Document, e.to_string()))?;
                // The import path validates the result's image header before the asset is referenced.
                vellum_import_image::ImageImporter
                    .import(&ImportInput {
                        bytes: &r.bytes,
                        name: "image.png",
                    })
                    .map_err(|_| {
                        ToolError::new(
                            ErrorKind::Document,
                            "The local model returned an invalid image.",
                        )
                    })?;
                let hash = ctx.docs().put_asset(&path, &r.bytes, &r.mime)?;
                let set = if raw["type"] == "image" {
                    json!({"asset":hash.as_str()})
                } else {
                    let mut fills = raw["fills"].clone();
                    fills[0]["asset"] = json!(hash.as_str());
                    json!({"fills":fills})
                };
                let patch = NodePatch::new(
                    set.as_object()
                        .expect("object")
                        .clone()
                        .into_iter()
                        .collect(),
                )
                .map_err(|_| ToolError::invalid("node", "The image cannot be updated."))?;
                batch.push(Op::Update { id, patch }, Label::whole("node"));
            }
            Action::Trace => {
                let svg = ops
                    .trace(&bytes)
                    .map_err(|e| ToolError::new(ErrorKind::Document, e.to_string()))?;
                let mut imported = vellum_import_svg::SvgImporter
                    .import(&ImportInput {
                        bytes: svg.as_bytes(),
                        name: "Traced artwork",
                    })
                    .map_err(|_| {
                        ToolError::new(
                            ErrorKind::Document,
                            "The traced paths could not be imported.",
                        )
                    })?;
                if imported.nodes.is_empty() {
                    return Err(ToolError::new(
                        ErrorKind::Document,
                        "The image produced no visible vectors.",
                    ));
                }
                let (w, h) = (
                    raw["width"].as_f64().ok_or_else(|| {
                        ToolError::invalid("node", "Trace needs a fixed image width.")
                    })?,
                    raw["height"].as_f64().ok_or_else(|| {
                        ToolError::invalid("node", "Trace needs a fixed image height.")
                    })?,
                );
                let root = imported.nodes.first().expect("not empty");
                let sx = w / root["width"].as_f64().unwrap_or(1.).max(1.);
                let sy = h / root["height"].as_f64().unwrap_or(1.).max(1.);
                for node in &mut imported.nodes {
                    scale_trace(node, sx, sy);
                }
                notes = imported.notes;
                let mut replacement = json!({"id":i.node,"type":"frame","name":target.name(),"width":raw["width"],"height":raw["height"],"children":imported.nodes});
                for key in [
                    "position", "rotation", "flip", "opacity", "visible", "locked", "metadata",
                ] {
                    if let Some(v) = raw.get(key) {
                        replacement[key] = v.clone()
                    }
                }
                // Trace only the pixels. Crop, rounded clipping and decoration are not silently claimed as preserved.
                notes.push("Tracing converts source pixels; image fit, crop, corner clipping, strokes and effects are not retained.".into());
                let node = new_node(
                    replacement,
                    &doc,
                    &mut ctx.docs().tool_ids(),
                    &mut ids_in_use(&doc),
                    "node",
                )?;
                batch.push(
                    Op::Replace {
                        id,
                        node: Box::new(node),
                    },
                    Label::whole("node"),
                );
            }
        }
        ctx.commit(&path, batch)?;
        Ok(json!({"document":path,"node":i.node,"notes":notes}))
    }
}

fn scale_trace(node: &mut Value, sx: f64, sy: f64) {
    for (key, scale) in [("width", sx), ("height", sy)] {
        if let Some(n) = node[key].as_f64() {
            node[key] = json!(n * scale)
        }
    }
    if let Some(p) = node.get_mut("position") {
        for (key, scale) in [("x", sx), ("y", sy)] {
            if let Some(n) = p[key].as_f64() {
                p[key] = json!(n * scale)
            }
        }
    }
    if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
        for child in children {
            scale_trace(child, sx, sy)
        }
    }
}
