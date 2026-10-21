//! In-memory export API shared by MCP, CLI and editor commands. Callers own destination/clipboard handling.
use super::{ErrorKind, Index, Tool, ToolCtx, ToolError, parse_input, schema_of};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::{doc_api::Services, export::*, ports::*};
struct Export;
pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(Export));
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Assets {
    Inline,
    Files,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    document: Option<String>,
    /// Layer or page ids. PDF slides follow deck order; a slides page exports each frame. Canvas targets retain selection order; multiple rasters return a ZIP.
    nodes: Vec<String>,
    /// png, jpeg, webp, svg, pdf, html-css or html-tailwind.
    format: String,
    /// Raster scale: 1, 2 or 3. Vector and code exports retain canvas units.
    scale: Option<u8>,
    /// Include a complete HTML page (default true).
    scaffold: Option<bool>,
    /// Inline HTML assets (default) or return them as additional files in the bundle.
    assets: Option<Assets>,
    /// A filename suffix. Unsafe characters become hyphens; never interpreted as a path.
    suffix: Option<String>,
}
struct Host<'a>(&'a dyn Services);
impl ExportRenderer for Host<'_> {
    fn render(&self, frames: &[ExportFrame], format: &str) -> Result<Vec<u8>, ExportError> {
        self.0.render_export(frames, format)
    }
}
impl Tool for Export {
    fn name(&self) -> &'static str {
        "export"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Export selected layers to PNG, JPEG, WEBP, SVG, PDF, HTML + CSS or HTML + Tailwind. Returns named files as MIME/base64 data, warnings and, for one PNG or inline HTML output, clipboard MIME data. PDF slides follow deck order (a slides page exports its frames); canvas PDF pages follow selection order; multiple raster images become a ZIP. HTML includes its styles and works offline. No file is written. The caller saves the bundle; agent file writes require permission."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<Input>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let mut input: Input = parse_input(input)?;
        if input.nodes.is_empty() || input.nodes.len() > 100 {
            return Err(ToolError::invalid(
                "nodes",
                "Select between 1 and 100 layers",
            ));
        }
        let scale = input.scale.unwrap_or(1);
        if !(1..=3).contains(&scale) {
            return Err(ToolError::invalid("scale", "Scale is 1, 2 or 3"));
        }
        let image = vellum_export_raster::Images::new(&input.format);
        let code = vellum_export_html::Html::new(&input.format);
        if image.is_none() && code.is_none() {
            return Err(ToolError::invalid(
                "format",
                "Choose png, jpeg, webp, svg, pdf, html-css or html-tailwind",
            ));
        }
        let (path, doc) = ctx.document(input.document.as_deref())?;
        let index = Index::build(&doc);
        for target in &input.nodes {
            if index.scope(target).is_none() {
                return Err(ToolError::not_found(format!(
                    "There is no layer or page {target:?}"
                )));
            }
        }
        if input.format == "pdf" {
            input.nodes = pdf_targets(&doc, &input.nodes)
                .map_err(|e| ToolError::invalid("nodes", e.to_string()))?;
        }
        let services = ctx.docs().services();
        let scene = ctx
            .docs()
            .export_scene(&path, &doc, &input.nodes)
            .map_err(|e| ToolError::new(ErrorKind::Render, e.to_string()))?;
        let host = Host(&*services);
        let result = if let Some(adapter) = image {
            adapter.export(
                &scene,
                &ImageOptions {
                    scale,
                    suffix: input.suffix.unwrap_or_default(),
                },
                &host,
            )
        } else {
            code.expect("checked format").export(
                &scene,
                &CodeOptions {
                    scaffold: input.scaffold.unwrap_or(true),
                    assets: match input.assets.unwrap_or(Assets::Inline) {
                        Assets::Inline => AssetMode::Inline,
                        Assets::Files => AssetMode::Files,
                    },
                },
                &host,
            )
        }
        .map_err(|e| ToolError::new(ErrorKind::Render, e.to_string()))?;
        let size: usize = result.files.iter().map(|f| f.bytes.len()).sum();
        if size > 700 * 1024 {
            return Err(ToolError::new(
                ErrorKind::Render,
                "Export exceeds the synchronous result budget; export fewer layers or use a lower scale",
            ));
        }
        // Clipboard bytes are a host capability, not a side effect of this tool. Clients may choose to use them.
        let clipboard = result.clipboard().ok().map(|entries| {
            entries
                .into_iter()
                .map(|(mime, bytes)| (mime, json!(vellum_core::base64::encode(&bytes))))
                .collect::<serde_json::Map<_, _>>()
        });
        let files:Vec<_>=result.files.iter().map(|f|json!({"name":f.name,"mime":f.mime,"base64":vellum_core::base64::encode(&f.bytes)})).collect();
        let output = json!({"document":path,"format":input.format,"nodes":input.nodes,"files":files,"warnings":result.warnings,"clipboard":clipboard});
        if serde_json::to_vec(&output)
            .map_err(|e| ToolError::new(ErrorKind::Internal, e.to_string()))?
            .len()
            > 950 * 1024
        {
            return Err(ToolError::new(
                ErrorKind::Render,
                "Encoded export exceeds the synchronous result budget; export fewer layers",
            ));
        }
        Ok(output)
    }
}
