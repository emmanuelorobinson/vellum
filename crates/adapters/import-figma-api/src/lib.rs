//! Figma REST JSON import. Proprietary `.fig` files are deliberately unsupported.
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use vellum_core::ports::*;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub const KEY_NAME: &str = "figma_token";
const API: &str = "https://api.figma.com/v1/files/";
const MAX: usize = 32 * 1024 * 1024;
pub struct FigmaImporter;
pub fn file_key(value: &str) -> Result<String, ImageWorkError> {
    let key = if value.starts_with("https://") {
        let u = url::Url::parse(value).map_err(|_| ImageWorkError::InvalidInput)?;
        if u.host_str() != Some("www.figma.com") && u.host_str() != Some("figma.com") {
            return Err(ImageWorkError::InvalidInput);
        }
        if !u.username().is_empty() || u.password().is_some() || u.port().is_some() {
            return Err(ImageWorkError::InvalidInput);
        }
        let parts: Vec<_> = u
            .path_segments()
            .ok_or(ImageWorkError::InvalidInput)?
            .collect();
        if !matches!(parts.first(), Some(&"file" | &"design")) {
            return Err(ImageWorkError::InvalidInput);
        }
        parts
            .get(1)
            .ok_or(ImageWorkError::InvalidInput)?
            .to_string()
    } else {
        value.to_string()
    };
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(ImageWorkError::InvalidInput);
    }
    Ok(key)
}
pub struct FigmaApi {
    pub http: Arc<dyn MediaHttp>,
    pub secrets: Arc<dyn SecretStore>,
}
fn status(code: u16) -> Result<(), ImageWorkError> {
    match code {
        200 => Ok(()),
        401 | 403 => Err(ImageWorkError::Authentication),
        429 => Err(ImageWorkError::RateLimited),
        _ => Err(ImageWorkError::Network),
    }
}
impl FigmaSource for FigmaApi {
    fn import_file(
        &self,
        key: &str,
        allow: &dyn Fn(&str) -> Result<(), ImageWorkError>,
    ) -> Result<Imported, ImageWorkError> {
        let key = file_key(key)?;
        allow("https://api.figma.com")?;
        let token = self
            .secrets
            .get(KEY_NAME)
            .map_err(|_| ImageWorkError::Unavailable)?
            .ok_or(ImageWorkError::NotConfigured)?;
        let get =
            |url: String, auth: bool, max_bytes: usize| -> Result<FetchResponse, ImageWorkError> {
                let r = self
                    .http
                    .exchange(&MediaRequest {
                        url,
                        method: MediaMethod::Get,
                        body: vec![],
                        credential: auth.then(|| ("X-Figma-Token".into(), token.clone())),
                        max_bytes,
                    })
                    .map_err(|_| ImageWorkError::Network)?;
                status(r.status)?;
                if r.body.len() > max_bytes
                    || (!token.expose().is_empty()
                        && r.body
                            .windows(token.expose().len())
                            .any(|b| b == token.expose().as_bytes()))
                {
                    return Err(ImageWorkError::InvalidOutput);
                }
                Ok(r)
            };
        let body = get(format!("{API}{key}?geometry=paths"), true, MAX)?.body;
        let raw: Value =
            serde_json::from_slice(&body).map_err(|_| ImageWorkError::InvalidOutput)?;
        let mut references = BTreeSet::new();
        collect_refs(&raw, &mut references);
        if references.len() > 128 {
            return Err(ImageWorkError::InvalidOutput);
        }
        let mut assets = BTreeMap::new();
        let mut notes = vec![];
        if !references.is_empty() {
            match get(format!("{API}{key}/images"), true, 2 * 1024 * 1024).and_then(|r| {
                serde_json::from_slice::<Value>(&r.body).map_err(|_| ImageWorkError::InvalidOutput)
            }) {
                Ok(index) => {
                    let mut total = 0;
                    for id in references {
                        let url = index["meta"]["images"][&id].as_str();
                        let result = url
                            .filter(|u| image_url(u))
                            .ok_or(ImageWorkError::InvalidOutput)
                            .and_then(|u| {
                                let origin = url::Url::parse(u)
                                    .map_err(|_| ImageWorkError::InvalidOutput)?
                                    .origin()
                                    .ascii_serialization();
                                allow(&origin)?;
                                get(u.to_string(), false, 12 * 1024 * 1024)
                            });
                        match result {
                            Ok(r) if matches!(r.content_type.as_str(),"image/png"|"image/jpeg"|"image/webp") && !r.body.is_empty()=>{
                                total+=r.body.len(); if total>64*1024*1024{return Err(ImageWorkError::InvalidOutput)}
                                assets.insert(id,ImportedAsset{mime:r.content_type,bytes:r.body});
                            },
                            _=>notes.push("An image fill could not be downloaded; retry the import to include it.".into()),
                        }
                    }
                }
                Err(_) => notes.push(
                    "Image fills could not be fetched; retry the import to include them.".into(),
                ),
            }
        }
        let mut imported = convert(&raw, &assets).map_err(|_| ImageWorkError::InvalidOutput)?;
        imported.notes.extend(notes);
        Ok(imported)
    }
}
fn image_url(text: &str) -> bool {
    url::Url::parse(text).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && u.fragment().is_none()
            && u.host_str().is_some_and(|h| {
                h.ends_with(".figma.com") || h.ends_with(".amazonaws.com") || h == "figma.com"
            })
    })
}
fn collect_refs(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            if let Some(s) = m.get("imageRef").and_then(Value::as_str) {
                out.insert(s.to_string());
            }
            for v in m.values() {
                collect_refs(v, out)
            }
        }
        Value::Array(a) => {
            for v in a {
                collect_refs(v, out)
            }
        }
        _ => {}
    }
}
impl Importer for FigmaImporter {
    fn id(&self) -> &'static str {
        "figma-api"
    }
    fn recognizes(&self, i: &ImportInput<'_>) -> bool {
        i.bytes.len() <= MAX
            && serde_json::from_slice::<Value>(i.bytes)
                .is_ok_and(|v| v["document"]["type"] == "DOCUMENT")
    }
    fn import(&self, i: &ImportInput<'_>) -> Result<Imported, ImportError> {
        if i.bytes.len() > MAX {
            return Err(ImportError::new("Figma response exceeds 32 MB."));
        }
        let raw = serde_json::from_slice::<Value>(i.bytes).map_err(|_| {
            ImportError::new("Import a Figma REST response, not a local .fig file.")
        })?;
        convert(&raw, &BTreeMap::new())
    }
}
fn num(v: &Value, k: &str, d: f64) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(d)
}
fn color(v: &Value) -> String {
    let c = |k| (num(v, k, if k == "a" { 1. } else { 0. }).clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02X}{:02X}{:02X}{:02X}", c("r"), c("g"), c("b"), c("a"))
}
struct Convert<'a> {
    out: Imported,
    notes: BTreeSet<String>,
    assets: &'a BTreeMap<String, ImportedAsset>,
    refs: BTreeMap<String, usize>,
    count: usize,
}
fn convert(v: &Value, assets: &BTreeMap<String, ImportedAsset>) -> Result<Imported, ImportError> {
    if v["document"]["type"] != "DOCUMENT" {
        return Err(ImportError::new("The response has no Figma document."));
    }
    let mut c = Convert {
        out: Imported::default(),
        notes: BTreeSet::new(),
        assets,
        refs: BTreeMap::new(),
        count: 0,
    };
    c.out.nodes = c.nodes(&v["document"], 0., 0., 0)?;
    c.out.notes = c.notes.into_iter().collect();
    Ok(c.out)
}
impl Convert<'_> {
    fn note(&mut self, s: impl Into<String>) {
        self.notes.insert(s.into());
    }
    fn paints(&mut self, v: &Value) -> Vec<Value> {
        let Some(a) = v.as_array() else { return vec![] };
        a.iter()
            .filter(|p| p["visible"] != false)
            .filter_map(|p| {
                let mut paint = match p["type"].as_str().unwrap_or("") {
                    "SOLID" => json!({"type":"solid","color":color(&p["color"])}),
                    "IMAGE" => {
                        let r = p["imageRef"].as_str().unwrap_or("");
                        let Some(a) = self.assets.get(r) else {
                            self.note("An image fill is missing from the REST import.");
                            return None;
                        };
                        let index = *self.refs.entry(r.into()).or_insert_with(|| {
                            let n = self.out.assets.len();
                            self.out.assets.push(a.clone());
                            n
                        });
                        if p.get("imageTransform").is_some() || p.get("filters").is_some() {
                            self.note("Image transforms and filters were not converted.")
                        }
                        let fit = match p["scaleMode"].as_str() {
                            Some("FIT") => "fit",
                            Some("STRETCH") => "stretch",
                            Some("TILE") => {
                                self.note("Tiled images use fill.");
                                "fill"
                            }
                            _ => "fill",
                        };
                        json!({"type":"image","asset":asset_ref(index),"fit":fit})
                    }
                    _ => {
                        self.note("Unsupported gradient or pattern paints were omitted.");
                        return None;
                    }
                };
                if p.get("boundVariables").is_some() {
                    self.note("Variable bindings were replaced with resolved values.")
                }
                if p["blendMode"].as_str().is_some_and(|s| s != "NORMAL") {
                    self.note("Paint blend modes were not converted.")
                }
                if let Some(o) = p.get("opacity") {
                    paint["opacity"] = o.clone();
                }
                Some(paint)
            })
            .collect()
    }
    fn nodes(
        &mut self,
        v: &Value,
        px: f64,
        py: f64,
        depth: usize,
    ) -> Result<Vec<Value>, ImportError> {
        self.count += 1;
        if self.count > 20000 || depth > 64 {
            return Err(ImportError::new(
                "Figma import exceeds the layer or depth limit.",
            ));
        }
        let kind = v["type"].as_str().unwrap_or("");
        if matches!(kind, "DOCUMENT" | "CANVAS") {
            let mut all = vec![];
            for n in v["children"].as_array().into_iter().flatten() {
                all.extend(self.nodes(n, px, py, depth + 1)?)
            }
            return Ok(all);
        }
        let b = &v["absoluteBoundingBox"];
        let x = num(b, "x", 0.);
        let y = num(b, "y", 0.);
        let w = num(b, "width", 1.);
        let h = num(b, "height", 1.);
        let ty = match kind {
            "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE" => "frame",
            "GROUP" => "group",
            "RECTANGLE" => "rect",
            "ELLIPSE" => "ellipse",
            "LINE" => "line",
            "TEXT" => "text",
            "VECTOR" | "STAR" | "REGULAR_POLYGON" | "BOOLEAN_OPERATION" => "vector",
            _ => {
                self.note(format!("Unsupported {kind} layers were omitted."));
                return Ok(vec![]);
            }
        };
        if ["x", "y", "width", "height"]
            .iter()
            .any(|k| b[*k].as_f64().is_none())
            || w < 0.
            || h < 0.
        {
            self.note("Layers without valid geometry were omitted.");
            return Ok(vec![]);
        }
        let mut n = json!({"type":ty,"name":v["name"].as_str().unwrap_or(kind),"width":w,"height":h,"position":{"type":"absolute","x":x-px,"y":y-py}});
        if let Some(id) = v["id"].as_str() {
            n["metadata"] = json!({"figma":{"nodeId":id}})
        }
        for (k, to) in [
            ("visible", "visible"),
            ("opacity", "opacity"),
            ("cornerRadius", "cornerRadius"),
            ("clipsContent", "clip"),
        ] {
            if let Some(value) = v.get(k) {
                n[to] = value.clone();
            }
        }
        if let Some(rotation) = v["rotation"].as_f64().filter(|r| r.abs() > 0.00001) {
            self.note("Rotated layers use their axis-aligned bounds.");
            let _ = rotation;
        }
        for (k, note) in [
            (
                "constraints",
                "Constraints were flattened to fixed positions.",
            ),
            (
                "rectangleCornerRadii",
                "Independent corner radii were not converted.",
            ),
            (
                "layoutMode",
                "Auto layout was flattened to absolute positions.",
            ),
            ("effects", "Effects were not converted."),
            ("isMask", "Masks were not converted."),
            ("reactions", "Prototype interactions were not converted."),
            (
                "transitionNodeID",
                "Prototype transitions were not converted.",
            ),
            (
                "characterStyleOverrides",
                "Mixed text styles use the base style.",
            ),
        ] {
            if v.get(k).is_some_and(|v| {
                !v.is_null()
                    && v != false
                    && v != "NONE"
                    && v.as_array().is_none_or(|a| !a.is_empty())
            }) {
                self.note(note)
            }
        }
        if matches!(kind, "COMPONENT" | "COMPONENT_SET" | "INSTANCE") {
            self.note("Component links were flattened to editable frames.")
        }
        if ty == "vector" {
            let paths = v["fillGeometry"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| p["path"].as_str())
                .collect::<Vec<_>>()
                .join(" ");
            if paths.is_empty() {
                self.note("Vectors without fill geometry were omitted.");
                return Ok(vec![]);
            }
            n["path"] = json!(paths);
            n["viewBox"] = json!({"x":0,"y":0,"width":w.max(1.),"height":h.max(1.)});
            if v["fillGeometry"][0]["windingRule"] == "EVENODD" {
                n["fillRule"] = json!("evenodd");
            }
        }
        if ty == "text" {
            let st = &v["style"];
            let family = st["fontFamily"].as_str().unwrap_or("Inter");
            let text = v["characters"].as_str().unwrap_or("");
            n["content"] = json!([{"text":text,"font":family,"size":num(st,"fontSize",14.),"weight":num(st,"fontWeight",400.) as u16}]);
            n["resize"] = json!("fixed");
            if family != "Inter" {
                self.note("Fonts are not downloaded; install missing families to match the source.")
            }
        }
        let fills = self.paints(&v["fills"]);
        if !fills.is_empty() {
            n["fills"] = json!(fills)
        }
        let stroke = self.paints(&v["strokes"]);
        if !stroke.is_empty() {
            n["stroke"] = json!({"width":num(v,"strokeWeight",1.),"paints":stroke,"align":match v["strokeAlign"].as_str(){Some("INSIDE")=>"inside",Some("OUTSIDE")=>"outside",_=>"center"}})
        }
        if matches!(ty, "frame" | "group") {
            let mut children = vec![];
            for child in v["children"].as_array().into_iter().flatten() {
                children.extend(self.nodes(child, x, y, depth + 1)?)
            }
            n["children"] = json!(children);
            if ty == "frame" {
                n["layout"] = json!("none")
            }
        }
        Ok(vec![n])
    }
}
