//! Icons, pictures and imports: `search_icons`, `insert_icon`, `fill_image`, `import` (ARCHITECTURE §7.3, §3.6).
//!
//! Everything an agent can ask for here ends as an ordinary change to the document: one transaction, one undo
//! step, attributed to the agent. What lives outside the canvas is asked for through [`Outside`]: reading a file
//! (`path`) and reaching the stock library (`query`) need the person's permission; giving the content inline
//! (`data`, `text`) does not, because the agent already holds it.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use vellum_core::ops::Op;
use vellum_core::ports::{
    Attribution, IconGlyph, IconLibrary, IconStyle, ImageHit, ImageQuery, ImportInput, Importer,
    Orientation, asset_ref_index,
};
use vellum_icons::Catalog;
use vellum_types::NodePatch;

use super::batch::{Batch, Label, count, ids_in_use, new_node, node_id, place};
use super::support::{parse_input, schema_of};
use super::{ErrorKind, Tool, ToolCtx, ToolError, Verb};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>, icons: Rc<Catalog>) {
    tools.push(Box::new(SearchIcons(icons.clone())));
    tools.push(Box::new(InsertIcon(icons.clone())));
    tools.push(Box::new(FillImage));
    tools.push(Box::new(Import(icons)));
}

/// The largest picture or file read, in bytes.
const MAX_FILE: u64 = 32 * 1024 * 1024;
/// The largest number of search results one call returns.
const MAX_RESULTS: usize = 100;

fn libraries(catalog: &Catalog) -> String {
    catalog.ids().join(", ")
}

fn library<'a>(
    catalog: &'a Catalog,
    id: &str,
    field: &str,
) -> Result<&'a dyn IconLibrary, ToolError> {
    catalog.library(id).ok_or_else(|| {
        ToolError::invalid(
            field,
            format!(
                "There is no icon library {id:?}. The libraries are: {}.",
                libraries(catalog)
            ),
        )
    })
}

// ---- search_icons ----------------------------------------------------------------------------------------

struct SearchIcons(Rc<Catalog>);

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SearchInput {
    /// Words to find, such as "arrow right" or "settings". Every word has to match a part of an icon's name or one of
    /// its tags. Leave it out to list the libraries.
    query: Option<String>,
    /// Search one library only (see the list the call returns). Searches all of them when left out.
    library: Option<String>,
    /// The most icons to return, 1 to 100. Defaults to 20.
    limit: Option<u32>,
}

fn describe(lib: &dyn IconLibrary) -> Value {
    json!({
        "id": lib.id(), "label": lib.label(),
        "style": if lib.style() == IconStyle::Stroke { "outline" } else { "filled shapes" },
        "weights": lib.weights(), "fill": lib.supports_fill(), "icons": lib.len(),
    })
}

impl Tool for SearchIcons {
    fn name(&self) -> &'static str {
        "search_icons"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Find icons by name or meaning in Lucide, Feather, Phosphor and Material Symbols (and the project's own icons \
         when it has some). Returns library and name for each match, best first, with the libraries and the weights \
         each one draws. Pass the library and name to insert_icon."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<SearchInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, _ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let SearchInput {
            query,
            library: only,
            limit,
        } = parse_input(input)?;
        let limit = limit.map_or(20, |l| l as usize);
        if !(1..=MAX_RESULTS).contains(&limit) {
            return Err(ToolError::invalid(
                "limit",
                format!("Ask for 1 to {MAX_RESULTS} icons."),
            ));
        }
        let ids = match &only {
            Some(id) => vec![library(&self.0, id, "library")?.id().to_owned()],
            None => self.0.ids(),
        };
        let libs: Vec<&dyn IconLibrary> = ids.iter().filter_map(|id| self.0.library(id)).collect();
        let query = query.unwrap_or_default();
        let mut out = json!({"libraries": libs.iter().map(|l| describe(*l)).collect::<Vec<_>>()});
        if query.trim().is_empty() && only.is_none() {
            return Ok(out);
        }
        // Each library's best come first in turn, so one big set does not crowd out the rest.
        let found: Vec<_> = libs.iter().map(|l| l.search(&query, limit)).collect();
        let mut results = Vec::new();
        'rounds: for rank in 0..limit {
            for (lib, hits) in libs.iter().zip(&found) {
                if let Some(hit) = hits.get(rank) {
                    results.push(json!({"library": lib.id(), "name": hit.name, "tags": hit.tags.iter().take(8).collect::<Vec<_>>()}));
                    if results.len() == limit {
                        break 'rounds;
                    }
                }
            }
        }
        out["results"] = Value::Array(results);
        Ok(out)
    }
}

// ---- insert_icon -----------------------------------------------------------------------------------------

struct InsertIcon(Rc<Catalog>);

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InsertIconInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The page or layer to add the icon to, by id.
    parent: String,
    /// Where among the parent's children, back to front: 0 is the bottom. Defaults to the top.
    index: Option<u32>,
    /// The library, as search_icons lists them: lucide, feather, phosphor, material-symbols-outlined, ...
    library: String,
    /// The icon's name in that library, exactly as search_icons gives it.
    name: String,
    /// The width of the icon in pixels, 4 to 1024. Defaults to 24.
    size: Option<f64>,
    /// 100 to 700, where 400 is regular. Outline sets draw it as line width; Phosphor has 100, 300, 400 and 700;
    /// Material Symbols has 400. The nearest weight the library has is used.
    weight: Option<u16>,
    /// The filled variant, in libraries that have one.
    fill: Option<bool>,
    /// The colour as #RRGGBB or #RRGGBBAA. Defaults to black. Prefer color_variable when the design has colours.
    color: Option<String>,
    /// A colour variable's id to colour the icon by, so it follows themes.
    color_variable: Option<String>,
    /// Left edge inside the parent, for a parent without auto layout.
    x: Option<f64>,
    /// Top edge inside the parent.
    y: Option<f64>,
    /// The layer's name. Defaults to library and icon name.
    layer_name: Option<String>,
}

fn icon_node(glyph: &IconGlyph, input: &InsertIconInput, size: f64) -> Value {
    let paint = match (&input.color_variable, &input.color) {
        (Some(var), _) => json!({"type": "solid", "color": {"$var": var}}),
        (None, Some(color)) => json!({"type": "solid", "color": color}),
        (None, None) => json!({"type": "solid", "color": "#000000"}),
    };
    let [vx, vy, vw, vh] = glyph.view_box;
    let scale = size / vw;
    let mut node = Map::new();
    node.insert("type".into(), json!("vector"));
    node.insert(
        "name".into(),
        json!(
            input
                .layer_name
                .clone()
                .unwrap_or_else(|| format!("{}:{}", glyph.library, glyph.name))
        ),
    );
    node.insert("width".into(), json!(size));
    node.insert("height".into(), json!((vh * scale * 1000.).round() / 1000.));
    if input.x.is_some() || input.y.is_some() {
        node.insert(
            "position".into(),
            json!({"type": "absolute", "x": input.x.unwrap_or(0.), "y": input.y.unwrap_or(0.)}),
        );
    }
    node.insert("path".into(), json!(glyph.path));
    node.insert(
        "viewBox".into(),
        json!({"x": vx, "y": vy, "width": vw, "height": vh}),
    );
    match glyph.stroke {
        Some(stroke) => {
            node.insert(
                "stroke".into(),
                json!({"paints": [paint], "width": (stroke.width * scale * 1000.).round() / 1000.,
                       "cap": "round", "join": "round"}),
            );
        }
        None => {
            node.insert("fills".into(), json!([paint]));
        }
    }
    // Where it came from, so a later call or a person can swap its weight or library.
    node.insert(
        "metadata".into(),
        json!({"icon": {"library": glyph.library, "name": glyph.name, "weight": glyph.weight, "fill": glyph.fill}}),
    );
    Value::Object(node)
}

impl Tool for InsertIcon {
    fn name(&self) -> &'static str {
        "insert_icon"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Put an icon from an icon library on the canvas as an editable vector layer. Find the library and name with \
         search_icons. Choose size, weight, fill and a colour (or a colour variable). Returns the new layer's id and \
         what was drawn, including when the library could not give the exact weight or fill asked for."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<InsertIconInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::AddedAsset
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let input: InsertIconInput = parse_input(input)?;
        let size = input.size.unwrap_or(24.);
        if !(4.0..=1024.0).contains(&size) {
            return Err(ToolError::invalid("size", "The size is 4 to 1024 pixels."));
        }
        if let Some(weight) = input.weight
            && !(100..=700).contains(&weight)
        {
            return Err(ToolError::invalid("weight", "The weight is 100 to 700."));
        }
        if let Some(color) = &input.color
            && (!color.starts_with('#')
                || ![4, 5, 7, 9].contains(&color.len())
                || !color[1..].bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(ToolError::invalid(
                "color",
                "Give the colour as #RRGGBB or #RRGGBBAA.",
            ));
        }
        let lib = library(&self.0, &input.library, "library")?;
        let wanted = (input.weight.unwrap_or(400), input.fill.unwrap_or(false));
        let Some(glyph) = lib.glyph(&input.name, wanted.0, wanted.1) else {
            let near: Vec<String> = lib
                .search(&input.name, 5)
                .into_iter()
                .map(|i| i.name)
                .collect();
            return Err(ToolError::not_found(if near.is_empty() {
                format!(
                    "{} has no icon called {:?}. Use search_icons to find one.",
                    lib.id(),
                    input.name
                )
            } else {
                format!(
                    "{} has no icon called {:?}. Close names: {}.",
                    lib.id(),
                    input.name,
                    near.join(", ")
                )
            }));
        };
        let mut notes = Vec::new();
        if glyph.weight != wanted.0 {
            notes.push(format!(
                "{} draws weight {} here, the nearest to {}.",
                lib.label(),
                glyph.weight,
                wanted.0
            ));
        }
        if wanted.1 && !glyph.fill {
            notes.push(format!(
                "{} has no filled {}; the outline was used.",
                lib.label(),
                glyph.name
            ));
        }
        let (path, doc) = ctx.document(input.document.as_deref())?;
        let mut taken = ids_in_use(&doc);
        let node = new_node(
            icon_node(&glyph, &input, size),
            &doc,
            &mut ctx.docs().tool_ids(),
            &mut taken,
            "icon",
        )?;
        let id = node.id().as_str().to_owned();
        let to = place(&doc, &input.parent, input.index, None, "parent")?;
        let mut batch = Batch::default();
        batch.push(
            Op::Insert {
                to,
                node: Box::new(node),
            },
            Label::whole("icon")
                .rename("to.parent", "parent")
                .rename("to.index", "index"),
        );
        ctx.commit(&path, batch)?;
        Ok(json!({
            "document": path, "created": [id],
            "icon": {"library": glyph.library, "name": glyph.name, "weight": glyph.weight, "fill": glyph.fill},
            "notes": notes,
        }))
    }
}

// ---- shared: bytes from outside the canvas ---------------------------------------------------------------

fn decode_data(data: &str, field: &str) -> Result<Vec<u8>, ToolError> {
    let body = match data.strip_prefix("data:") {
        Some(rest) => rest.split_once(',').map_or(rest, |(_, b)| b),
        None => data,
    };
    let compact: String = body.split_whitespace().collect();
    let bytes = vellum_core::base64::decode(&compact).ok_or_else(|| {
        ToolError::invalid(
            field,
            "That is not base64. Give the file's bytes encoded as base64.",
        )
    })?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(ToolError::invalid(field, "The data is larger than 32 MB."));
    }
    Ok(bytes)
}

fn refused_without_outside(what: &str, instead: &str) -> ToolError {
    ToolError::new(
        ErrorKind::Refused,
        format!(
            "{what} needs the person's permission, and nothing here can ask them yet. {instead}"
        ),
    )
}

/// Reads the file `path` names for the agent, asking permission. A relative path is relative to the directory the
/// client was started in.
fn read_path(
    ctx: &mut ToolCtx<'_>,
    path: &str,
    field: &str,
) -> Result<(Vec<u8>, String), ToolError> {
    let wanted = Path::new(path);
    let full: PathBuf = if wanted.is_absolute() {
        wanted.to_path_buf()
    } else {
        match &ctx.session().cwd {
            Some(cwd) => cwd.join(wanted),
            None => {
                return Err(ToolError::invalid(
                    field,
                    "Give an absolute path: this connection does not know which directory it was started in.",
                ));
            }
        }
    };
    let Some(outside) = ctx.docs().outside() else {
        return Err(refused_without_outside(
            &format!("Reading {path}"),
            "Pass the file's content as `data` (base64) instead.",
        ));
    };
    let origin = ctx.origin();
    let bytes = outside.read_file(&origin, &full, MAX_FILE)?;
    let name = full
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok((bytes, name))
}

/// A picture's mime type and size, or why it is not one Vellum can use.
fn picture(bytes: &[u8], name: &str) -> Result<(String, Value), ToolError> {
    let imported = vellum_import_image::ImageImporter
        .import(&ImportInput { bytes, name })
        .map_err(|e| ToolError::invalid("", e.to_string()))?;
    let mime = imported.assets[0].mime.clone();
    Ok((mime, imported.nodes[0].clone()))
}

// ---- fill_image ------------------------------------------------------------------------------------------

struct FillImage;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FillInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The layer to fill: an image layer, or a frame, rectangle, ellipse or other shape.
    node: String,
    /// A picture file (PNG, JPEG or WEBP) in the repository. Needs the person's permission.
    path: Option<String>,
    /// Generate an image using the provider configured in Settings, Assets.
    prompt: Option<String>,
    /// The image model id. Defaults to the model configured in native image settings.
    model: Option<String>,
    /// A picture as base64 (a `data:` URL also works). Needs no permission.
    data: Option<String>,
    /// Words to find a stock picture with. The picture is downloaded into the repository, so the design never
    /// depends on the stock site. Needs the person's permission to reach it, and a key in Settings, Assets.
    query: Option<String>,
    /// With `query`: which result to use, counting from 0. Defaults to 0, the best match.
    result: Option<u32>,
    /// With `query`: "landscape" or "portrait" pictures only.
    orientation: Option<String>,
    /// How the picture fills the layer: "fill" (cover, the default), "fit" (contain) or "stretch".
    fit: Option<String>,
}

fn attribution_json(a: &Attribution) -> Value {
    json!({
        "provider": a.provider, "author": a.author, "authorUrl": a.author_url, "sourceUrl": a.source_url,
        "licence": a.licence, "licenceUrl": a.licence_url,
    })
}

impl Tool for FillImage {
    fn name(&self) -> &'static str {
        "fill_image"
    }
    fn version(&self) -> u32 {
        2
    }
    fn description(&self) -> &'static str {
        "Fill a layer with a real picture: a file from the repository (`path`), a picture you hold (`data`, base64) or \
         a stock photo found by words (`query`), or a generated image (`prompt`, configured `model`). The picture is stored in the repository next to the document. An \
         image layer gets the new picture; any other shape gets an image fill. A stock photo's author, page and licence \
         are written to the layer's metadata. Reading a file or reaching the stock library asks the person first."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<FillInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::AddedAsset
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let FillInput {
            document,
            node,
            path,
            data,
            query,
            prompt,
            model,
            result,
            orientation,
            fit,
        } = parse_input(input)?;
        let given = [
            path.is_some(),
            data.is_some(),
            query.is_some(),
            prompt.is_some(),
        ]
        .iter()
        .filter(|g| **g)
        .count();
        if given != 1 {
            return Err(ToolError::invalid(
                "path",
                "Give exactly one of `path`, `data`, `query` or `prompt`.",
            ));
        }
        let fit = match fit.as_deref() {
            None | Some("fill") => "fill",
            Some("fit") => "fit",
            Some("stretch") => "stretch",
            Some(other) => {
                return Err(ToolError::invalid(
                    "fit",
                    format!("{other:?} is not a fit. Use fill, fit or stretch."),
                ));
            }
        };
        let orientation = match orientation.as_deref() {
            None => None,
            Some("landscape") => Some(Orientation::Landscape),
            Some("portrait") => Some(Orientation::Portrait),
            Some(other) => {
                return Err(ToolError::invalid(
                    "orientation",
                    format!("{other:?} is not landscape or portrait."),
                ));
            }
        };
        let (doc_path, doc) = ctx.document(document.as_deref())?;
        let id = node_id(&node, "node")?;
        let Some(target) = vellum_core::ops::find(&doc, &id) else {
            return Err(ToolError::not_found(format!("There is no layer {node:?}.")));
        };
        let is_image = matches!(target, vellum_types::Node::Image(_));
        use vellum_types::Node as N;
        if !matches!(
            target,
            N::Image(_)
                | N::Frame(_)
                | N::Rect(_)
                | N::Ellipse(_)
                | N::Polygon(_)
                | N::Star(_)
                | N::Vector(_)
                | N::Boolean(_)
        ) {
            return Err(ToolError::invalid(
                "node",
                "That layer cannot hold a picture. Use an image layer, a frame, or a shape such as a rectangle.",
            ));
        }

        // Where the picture comes from.
        let mut generated = None;
        let mut stock_note = None;
        let (bytes, mime, attribution) = if let Some(prompt) = prompt {
            let generator = ctx.docs().image_generator().ok_or_else(|| {
                ToolError::new(
                    ErrorKind::Refused,
                    "Configure an image provider in Settings, Assets.",
                )
            })?;
            let model = model
                .or_else(|| ctx.docs().image_model())
                .filter(|m| !m.trim().is_empty())
                .ok_or_else(|| ToolError::invalid("model", "Choose an image model."))?;
            let outside = ctx.docs().outside().ok_or_else(|| {
                refused_without_outside("Image generation", "Connect a permission host.")
            })?;
            outside.network(&ctx.origin(), generator.origin())?;
            let r = generator
                .generate(&vellum_core::ports::GenerateImage {
                    prompt,
                    model: model.clone(),
                })
                .map_err(|e| ToolError::new(ErrorKind::Document, e.to_string()))?;
            picture(&r.bytes, "generated.png")?;
            generated = Some(json!({"provider":generator.provider(),"model":model}));
            (r.bytes, r.mime, None)
        } else if let Some(query) = query {
            let Some(stock) = ctx.docs().stock() else {
                return Err(ToolError::new(
                    ErrorKind::Refused,
                    "No stock image library is set up. Add a key in Settings, Assets, or use `path` or `data`.",
                ));
            };
            let Some(outside) = ctx.docs().outside() else {
                return Err(refused_without_outside(
                    &format!("Reaching {}", stock.origin()),
                    "Use `path` or `data` instead.",
                ));
            };
            let origin = ctx.origin();
            outside.network(&origin, stock.origin())?;
            let pick = result.unwrap_or(0) as usize;
            let page = stock
                .search(&ImageQuery {
                    query: query.clone(),
                    page: 1,
                    per_page: (pick as u32 + 1).clamp(3, 50),
                    orientation,
                })
                .map_err(|e| ToolError::new(ErrorKind::Document, e.to_string()))?;
            let Some(hit) = page.hits.get(pick) else {
                return Err(ToolError::not_found(if page.hits.is_empty() {
                    format!("The stock library has no pictures for {query:?}.")
                } else {
                    format!(
                        "There are only {} results for {query:?}; result {pick} does not exist.",
                        page.hits.len()
                    )
                }));
            };
            let hit: ImageHit = hit.clone();
            let fetched = stock
                .fetch(&hit)
                .map_err(|e| ToolError::new(ErrorKind::Document, e.to_string()))?;
            stock_note = Some(
                json!({"query": query, "result": pick, "found": page.total, "description": hit.description}),
            );
            (fetched.bytes, fetched.mime, Some(fetched.attribution))
        } else {
            let (bytes, name) = match (path, data) {
                (Some(path), _) => read_path(ctx, &path, "path")?,
                (_, Some(data)) => (decode_data(&data, "data")?, String::new()),
                _ => unreachable!("exactly one was given"),
            };
            let (mime, _) = picture(&bytes, &name)?;
            (bytes, mime, None)
        };

        let hash = ctx.docs().put_asset(&doc_path, &bytes, &mime)?;
        let asset = hash.as_str().to_owned();

        // The change: the layer's picture, and where the picture came from.
        let mut metadata: Map<String, Value> = target.metadata().clone().into_iter().collect();
        metadata.remove("attribution");
        metadata.remove("generated");
        if let Some(generated) = generated {
            metadata.insert("generated".into(), generated);
        }
        if let Some(a) = &attribution {
            metadata.insert("attribution".into(), attribution_json(a));
        }
        let mut set: Map<String, Value> = Map::new();
        if is_image {
            set.insert("asset".into(), json!(asset));
            set.insert("fit".into(), json!(fit));
        } else {
            let mut fills = serde_json::to_value(target.fills())
                .map_err(|e| ToolError::new(ErrorKind::Internal, e.to_string()))?;
            let fills = fills.as_array_mut().expect("fills are a list");
            let paint = json!({"type": "image", "asset": asset, "fit": fit});
            match fills.iter().position(|p| p["type"] == "image") {
                Some(at) => fills[at] = paint,
                None => fills.push(paint),
            }
            set.insert("fills".into(), Value::Array(fills.clone()));
        }
        set.insert("metadata".into(), Value::Object(metadata));
        let patch = NodePatch::new(set.into_iter().collect())
            .map_err(|e| ToolError::invalid("node", super::batch::sentence(&e.to_string())))?;
        let mut batch = Batch::default();
        batch.push(Op::Update { id, patch }, Label::whole("node"));
        ctx.commit(&doc_path, batch)?;
        let mut out = json!({
            "document": doc_path, "node": node, "asset": asset, "mime": mime, "bytes": bytes.len(), "fit": fit,
        });
        if let Some(a) = attribution {
            out["attribution"] = attribution_json(&a);
            out["stock"] = stock_note.unwrap_or(Value::Null);
        }
        Ok(out)
    }
}

// ---- import ----------------------------------------------------------------------------------------------

struct Import(Rc<Catalog>);

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImportInputs {
    /// A Figma design URL or file key. Token stays in SecretStore.
    figma: Option<String>,
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The page or layer to import into, by id.
    parent: String,
    /// Where among the parent's children, back to front. Defaults to the top.
    index: Option<u32>,
    /// A file in the repository: .pen 2.21, SVG, or PNG/JPEG/WEBP. Needs permission; .pen companion images are authorized separately.
    path: Option<String>,
    /// The file as base64 (a `data:` URL also works). Needs no permission.
    data: Option<String>,
    /// Inline .pen or Figma REST JSON, computed DOM JSON, SVG markup, or Figma clipboard HTML. Needs no filesystem permission.
    text: Option<String>,
    /// The file's name, which names the layer. Taken from `path` when there is one.
    name: Option<String>,
    /// "pen", "svg", "image", "figma", "figma-api", or "html". Detected when absent.
    format: Option<String>,
    /// Left edge of the imported content inside the parent, for a parent without auto layout.
    x: Option<f64>,
    /// Top edge.
    y: Option<f64>,
}

fn importers() -> [(&'static str, &'static str, Box<dyn Importer>); 6] {
    [
        (
            "pen",
            "pen",
            Box::new(
                vellum_import_pen::PenImporter::default()
                    .with_path_bounds(vellum_import_svg::path_bounds),
            ),
        ),
        (
            "figma-api",
            "figma-api",
            Box::new(vellum_import_figma_api::FigmaImporter),
        ),
        ("html", "html", Box::new(vellum_import_html::HtmlImporter)),
        (
            "figma",
            "figma-clipboard",
            Box::new(vellum_import_figma_clipboard::FigmaClipboardImporter),
        ),
        ("svg", "svg", Box::new(vellum_import_svg::SvgImporter)),
        (
            "image",
            "image",
            Box::new(vellum_import_image::ImageImporter),
        ),
    ]
}

/// Replaces each `import:N` asset reference in `value` with the hash of the N-th stored asset.
fn resolve_assets(value: &mut Value, hashes: &[String]) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map.iter_mut() {
                match (key.as_str(), &*inner) {
                    ("asset", Value::String(text)) if asset_ref_index(text).is_some() => {
                        let index = asset_ref_index(text).expect("checked");
                        *inner = json!(hashes.get(index).cloned().unwrap_or_default());
                    }
                    _ => resolve_assets(inner, hashes),
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| resolve_assets(item, hashes)),
        _ => {}
    }
}

impl Tool for Import {
    fn name(&self) -> &'static str {
        "import"
    }
    fn version(&self) -> u32 {
        4
    }
    fn description(&self) -> &'static str {
        "Bring existing content onto the canvas as editable layers: an SVG (shapes become rectangles, ellipses and \
         vectors, text becomes text), a PNG, JPEG or WEBP picture (an image layer, the file stored in the repository), \
         or layers copied from Figma (pass the clipboard HTML as `text`; pictures do not come across). Give a file \
         `path` (needs the person's permission) or the content inline as `data` or `text`. For a full Figma file, give figma (a design URL or key); its token stays in the OS keychain. Returns the new layer ids \
         and notes on what could not be imported. .pen 2.21 files become editable layers; local component instances and themed variables are resolved, companion images need separate file permission, and executable content is never loaded. BrowserPort computed DOM snapshots use format html."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ImportInputs>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::AddedAsset
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ImportInputs {
            figma,
            document,
            parent,
            index,
            path,
            data,
            text,
            name,
            format,
            x,
            y,
        } = parse_input(input)?;
        if let Some(figma) = figma {
            if path.is_some() || data.is_some() || text.is_some() {
                return Err(ToolError::invalid(
                    "figma",
                    "Give only the Figma URL or key.",
                ));
            }
            let key = vellum_import_figma_api::file_key(&figma)
                .map_err(|e| ToolError::invalid("figma", e.to_string()))?;
            let (_, doc) = ctx.document(document.as_deref())?;
            place(&doc, &parent, index, None, "parent")?;
            let source = ctx.docs().figma().ok_or_else(|| {
                ToolError::new(ErrorKind::Refused, "Figma import is not configured.")
            })?;
            let outside = ctx.docs().outside().ok_or_else(|| {
                refused_without_outside("Figma import", "Connect a permission host.")
            })?;
            // Figma supplies its image URLs; the adapter restricts them to Figma's CDN/S3 and never sends the token there.
            let origin = ctx.origin();
            let imported = source
                .import_file(&key, &|url| {
                    outside
                        .network(&origin, url)
                        .map_err(|_| vellum_core::ports::ImageWorkError::PermissionDenied)
                })
                .map_err(|e| {
                    ToolError::new(
                        if e == vellum_core::ports::ImageWorkError::PermissionDenied {
                            ErrorKind::Refused
                        } else {
                            ErrorKind::Document
                        },
                        e.to_string(),
                    )
                })?;
            return apply_imported(
                ctx,
                document.as_deref(),
                &parent,
                index,
                x,
                y,
                "figma-api",
                imported,
            );
        }
        let given = [path.is_some(), data.is_some(), text.is_some()]
            .iter()
            .filter(|g| **g)
            .count();
        if given != 1 {
            return Err(ToolError::invalid(
                "path",
                "Give exactly one of `path`, `data` or `text`.",
            ));
        }
        let source_path = path.clone();
        let (bytes, file) = match (path, data, text) {
            (Some(path), _, _) => read_path(ctx, &path, "path")?,
            (_, Some(data), _) => (
                decode_data(&data, "data")?,
                name.clone().unwrap_or_default(),
            ),
            (_, _, Some(text)) => (text.into_bytes(), name.clone().unwrap_or_default()),
            _ => unreachable!("exactly one was given"),
        };
        let file = name.unwrap_or(file);
        let all = importers();
        let chosen = match format.as_deref() {
            Some(wanted) => all
                .iter()
                .find(|(key, id, _)| *key == wanted || *id == wanted)
                .ok_or_else(|| ToolError::invalid("format", format!("{wanted:?} is not a format. Use pen, svg, image, figma, figma-api or html (a computed DOM snapshot).")))?,
            None => all
                .iter()
                .find(|(_, _, importer)| importer.recognizes(&ImportInput { bytes: &bytes, name: &file }))
                .ok_or_else(|| {
                    ToolError::invalid("", "This is not an SVG, a PNG, JPEG or WEBP picture, a supported .pen file, a Figma response or a computed DOM snapshot.")
                })?,
        };
        let imported = if chosen.1 == "pen" {
            import_pen(ctx, &bytes, &file, source_path.as_deref(), &self.0)?
        } else {
            chosen
                .2
                .import(&ImportInput {
                    bytes: &bytes,
                    name: &file,
                })
                .map_err(|e| ToolError::invalid("", e.to_string()))?
        };

        apply_imported(
            ctx,
            document.as_deref(),
            &parent,
            index,
            x,
            y,
            chosen.1,
            imported,
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_imported(
    ctx: &mut ToolCtx<'_>,
    document: Option<&str>,
    parent: &str,
    index: Option<u32>,
    x: Option<f64>,
    y: Option<f64>,
    importer: &str,
    imported: vellum_core::ports::Imported,
) -> Result<Value, ToolError> {
    let (doc_path, doc) = ctx.document(document)?;
    let mut hashes = Vec::new();
    for asset in &imported.assets {
        hashes.push(
            ctx.docs()
                .put_asset(&doc_path, &asset.bytes, &asset.mime)?
                .as_str()
                .to_owned(),
        );
    }
    let mut taken = ids_in_use(&doc);
    let mut batch = Batch::default();
    fn frame_count(node: &vellum_types::Node) -> usize {
        usize::from(matches!(node, vellum_types::Node::Frame(_)))
            + node.children().iter().map(frame_count).sum::<usize>()
    }
    let (mut created, mut total, mut frames) = (Vec::new(), 0, 0);
    for (i, mut value) in imported.nodes.into_iter().enumerate() {
        resolve_assets(&mut value, &hashes);
        // The content sits at its own coordinates, shifted to where it was asked to go.
        if x.is_some() || y.is_some() {
            let (dx, dy) = (x.unwrap_or(0.), y.unwrap_or(0.));
            let (ox, oy) = value.get("position").map_or((0., 0.), |p| {
                (p["x"].as_f64().unwrap_or(0.), p["y"].as_f64().unwrap_or(0.))
            });
            value["position"] = json!({"type": "absolute", "x": ox + dx, "y": oy + dy});
        }
        let at = format!("import[{i}]");
        let node = new_node(value, &doc, &mut ctx.docs().tool_ids(), &mut taken, &at)?;
        let mut to = place(&doc, parent, index, None, "parent")?;
        to.index += i;
        created.push(node.id().as_str().to_owned());
        total += count(&node);
        frames += frame_count(&node);
        batch.push(
            Op::Insert {
                to,
                node: Box::new(node),
            },
            Label::new(i, at)
                .rename("to.parent", "parent")
                .rename("to.index", "index"),
        );
    }
    ctx.commit(&doc_path, batch)?;
    Ok(json!({
        "document": doc_path, "importer": importer, "created": created, "nodes": total, "frames":frames,
        "assets": hashes, "notes": imported.notes,
    }))
}

fn import_pen(
    ctx: &mut ToolCtx<'_>,
    bytes: &[u8],
    name: &str,
    source: Option<&str>,
    icons: &Catalog,
) -> Result<vellum_core::ports::Imported, ToolError> {
    let input = ImportInput { bytes, name };
    let mut assets = std::collections::BTreeMap::new();
    let mut notes = vec![];
    let mut total = 0usize;
    let paths = vellum_import_pen::PenImporter::image_paths(&input)
        .map_err(|e| ToolError::invalid("", e.to_string()))?;
    if let Some(source) = source {
        let source = PathBuf::from(source);
        let full = if source.is_absolute() {
            source
        } else {
            ctx.session()
                .cwd
                .as_ref()
                .expect("read_path validated cwd")
                .join(source)
        };
        let base = full.parent().expect("a file has a parent");
        for relative in paths {
            let path = base.join(&relative);
            match read_path(ctx, &path.to_string_lossy(), "path") {
                Ok((bytes, name)) => {
                    total += bytes.len();
                    if total > 64 * 1024 * 1024 {
                        return Err(ToolError::invalid("path", "Companion images exceed 64 MB."));
                    }
                    match picture(&bytes, &name) {
                        Ok((mime, _)) => {
                            assets.insert(
                                relative,
                                vellum_core::ports::ImportedAsset { mime, bytes },
                            );
                        }
                        Err(_) => notes.push(
                            "A companion image was unreadable and its fill was omitted."
                                .to_string(),
                        ),
                    }
                }
                Err(_) => notes
                    .push("A companion image was unavailable or its read was denied.".to_string()),
            }
        }
    }
    let mut imported = vellum_import_pen::PenImporter::with_assets(assets)
        .with_path_bounds(vellum_import_svg::path_bounds)
        .import(&input)
        .map_err(|e| ToolError::invalid("", e.to_string()))?;
    // Inline images use the same header validation as other imported pictures before any assets/ops are written.
    for asset in &mut imported.assets {
        let (mime, _) = picture(&asset.bytes, "image")?;
        asset.mime = mime;
    }
    lower_pen_icons(&mut imported.nodes, icons, &mut imported.notes);
    imported.notes.extend(notes);
    if imported.nodes.is_empty() {
        return Err(ToolError::invalid(
            "",
            "No supported layers could be imported from this .pen file.",
        ));
    }
    Ok(imported)
}

/// Icon nodes use the existing catalog/insert-icon conversion, so imports draw without a foreign icon SDK.
fn lower_pen_icons(nodes: &mut Vec<Value>, catalog: &Catalog, notes: &mut Vec<String>) {
    nodes.retain_mut(|node| {
        if let Some(children) = node.get_mut("children").and_then(Value::as_array_mut) {
            lower_pen_icons(children, catalog, notes);
        }
        if node["type"] != "icon" {
            return true;
        }
        let library = node["library"].as_str().unwrap_or("");
        let name = node["icon"].as_str().unwrap_or("");
        let weight = node["weight"].as_u64().unwrap_or(400) as u16;
        let Some(glyph) = catalog
            .library(library)
            .and_then(|l| l.glyph(name, weight, false))
        else {
            notes.push("An unavailable icon was omitted.".into());
            return false;
        };
        let size = node["width"]
            .as_f64()
            .or_else(|| node["height"].as_f64())
            .unwrap_or(24.);
        if !node["width"].is_number() || !node["height"].is_number() {
            notes.push("Layout-sized icons retain the catalog's base stroke weight.".into());
        }
        let input = InsertIconInput {
            document: None,
            parent: String::new(),
            index: None,
            library: library.into(),
            name: name.into(),
            size: Some(size),
            weight: Some(weight),
            fill: Some(false),
            color: None,
            color_variable: None,
            x: None,
            y: None,
            layer_name: None,
        };
        let vector = icon_node(&glyph, &input, size);
        let paints = node.get("fills").cloned().unwrap_or_else(|| json!([]));
        let object = node.as_object_mut().expect("node object");
        for key in ["icon", "library", "weight", "fill"] {
            object.remove(key);
        }
        object.insert("type".into(), json!("vector"));
        for key in ["path", "viewBox"] {
            object.insert(key.into(), vector[key].clone());
        }
        if let Some(stroke) = vector.get("stroke") {
            let mut stroke = stroke.clone();
            stroke["paints"] = paints;
            object.insert("stroke".into(), stroke);
            object.insert("fills".into(), json!([]));
        } else {
            object.insert("fills".into(), paints);
        }
        true
    });
}
