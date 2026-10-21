//! Computed DOM snapshots become ordinary editable nodes. No fetching, identifiers or document writes.
use serde_json::{Value, json};
use vellum_core::ports::{
    DomNode, DomSnapshot, ImportError, ImportInput, Imported, ImportedAsset, Importer, asset_ref,
};
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub struct HtmlImporter;
fn err() -> ImportError {
    ImportError::new("Invalid or oversized computed DOM snapshot.")
}
fn px(s: &str) -> Option<f64> {
    s.trim()
        .trim_end_matches("px")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}
fn color(s: &str) -> Option<String> {
    if s == "transparent" {
        return None;
    }
    let values: Vec<f64> = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))?
        .strip_suffix(')')?
        .split(',')
        .map(str::trim)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    if !(3..=4).contains(&values.len()) || values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let alpha = values.get(3).copied().unwrap_or(1.).clamp(0., 1.);
    if alpha == 0. {
        return None;
    }
    Some(format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        values[0].clamp(0., 255.) as u8,
        values[1].clamp(0., 255.) as u8,
        values[2].clamp(0., 255.) as u8,
        (alpha * 255.).round() as u8
    ))
}
fn convert(
    n: &DomNode,
    parent: Option<&DomNode>,
    result: &mut Imported,
) -> Result<Option<Value>, ImportError> {
    let mut node = json!({"type":"frame","name":n.name,"width":n.width,"height":n.height,
        "position":{"type":"absolute","x":n.x-parent.map_or(0.,|p|p.x),"y":n.y-parent.map_or(0.,|p|p.y)}});
    let style = |key: &str| n.style.get(key).map_or("", String::as_str);
    if let Some(opacity) = px(style("opacity")) {
        node["opacity"] = json!(opacity.clamp(0., 1.));
    }
    for (key, what) in [
        ("backgroundImage", "background image"),
        ("boxShadow", "shadow"),
        ("filter", "filter"),
        ("transform", "transform"),
    ] {
        if !matches!(style(key), "" | "none") {
            result
                .notes
                .push(format!("{}: {what} is not converted.", n.name));
        }
    }
    match n.kind.as_str() {
        "frame" => {
            node["children"] = json!([]);
            if let Some(c) = color(style("backgroundColor")) {
                node["fills"] = json!([{"type":"solid","color":c}]);
            }
            if let Some(r) = px(style("borderRadius")) {
                node["cornerRadius"] = json!(r.max(0.));
            }
            let border = px(style("borderTopWidth")).unwrap_or(0.);
            if border > 0.
                && let Some(c) = color(style("borderTopColor"))
            {
                node["stroke"] =
                    json!({"paints":[{"type":"solid","color":c}],"width":border,"align":"inside"});
            }
            if matches!(style("overflow"), "hidden" | "clip" | "scroll" | "auto") {
                node["clip"] = json!(true);
            }
        }
        "text" => {
            if style("fontOpticalSizing") == "auto" {
                result
                    .notes
                    .push(format!("{}: optical sizing uses the font default.", n.name));
            }
            for (key, label) in [
                ("fontVariationSettings", "font variations"),
                ("fontFeatureSettings", "font features"),
                ("textTransform", "text transformation"),
            ] {
                if !matches!(style(key), "" | "normal" | "none") {
                    result
                        .notes
                        .push(format!("{}: {label} are not converted.", n.name));
                }
            }
            node["type"] = json!("text");
            let mut run = json!({"text":n.text.as_deref().unwrap_or_default(),
                "font":style("fontFamily").split(',').next().unwrap_or("sans-serif").trim().trim_matches(['\'', '\"']),
                "size":px(style("fontSize")).unwrap_or(16.).max(0.1),
                "weight":(px(style("fontWeight")).unwrap_or(400.) as u16).clamp(100,900)});
            if let Some(h) = px(style("lineHeight")) {
                run["lineHeight"] = json!(h);
            }
            if let Some(v) = px(style("letterSpacing")) {
                run["letterSpacing"] = json!(v);
            }
            if style("fontStyle") == "italic" {
                run["style"] = json!("italic");
            }
            if style("textDecorationLine").contains("underline") {
                run["underline"] = json!(true);
            }
            if style("textDecorationLine").contains("line-through") {
                run["strikethrough"] = json!(true);
            }
            node["content"] = json!([run]);
            node["resize"] = json!("auto-width");
            if let Some(c) = color(style("color")) {
                node["fills"] = json!([{"type":"solid","color":c}]);
            }
        }
        "image" => {
            let Some(image) = &n.image else {
                result.notes.push(format!(
                    "{}: image pixels are unavailable (cross-origin or undecoded).",
                    n.name
                ));
                return Ok(None);
            };
            let (prefix, data) = image.split_once(',').ok_or_else(err)?;
            if prefix != "data:image/png;base64" {
                return Err(err());
            }
            let bytes = vellum_core::base64::decode(data)
                .filter(|b| b.len() <= 16 * 1024 * 1024)
                .ok_or_else(err)?;
            node["type"] = json!("image");
            node["asset"] = json!(asset_ref(result.assets.len()));
            node["fit"] = json!(if style("objectFit") == "contain" {
                "fit"
            } else {
                "fill"
            });
            result.assets.push(ImportedAsset {
                mime: "image/png".into(),
                bytes,
            });
        }
        _ => return Err(err()),
    }
    Ok(Some(node))
}
impl Importer for HtmlImporter {
    fn id(&self) -> &'static str {
        "html"
    }
    fn recognizes(&self, input: &ImportInput<'_>) -> bool {
        input.bytes.len() <= 32 * 1024 * 1024
            && serde_json::from_slice::<DomSnapshot>(input.bytes).is_ok_and(|s| s.version == 1)
    }
    fn import(&self, input: &ImportInput<'_>) -> Result<Imported, ImportError> {
        if input.bytes.len() > 32 * 1024 * 1024 {
            return Err(err());
        }
        let s: DomSnapshot = serde_json::from_slice(input.bytes).map_err(|_| err())?;
        if s.version != 1
            || s.nodes.is_empty()
            || s.nodes.len() > 5000
            || !s.width.is_finite()
            || !s.height.is_finite()
            || s.width <= 0.
            || s.height <= 0.
            || s.width > 16384.
            || s.height > 16384.
        {
            return Err(err());
        }
        for (i, n) in s.nodes.iter().enumerate() {
            if [n.x, n.y, n.width, n.height]
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
                || n.width < 0.
                || n.height < 0.
                || n.parent
                    .is_some_and(|p| p >= i || s.nodes[p].kind != "frame")
            {
                return Err(err());
            }
        }
        // Build bottom-up, avoiding recursion on untrusted DOM depth.
        let mut result = Imported {
            notes: s.notes,
            ..Imported::default()
        };
        let mut values = Vec::new();
        for n in &s.nodes {
            values.push(convert(n, n.parent.map(|p| &s.nodes[p]), &mut result)?);
        }
        let mut children = vec![Vec::new(); s.nodes.len()];
        for i in (0..s.nodes.len()).rev() {
            if let Some(mut node) = values[i].take() {
                if s.nodes[i].kind == "frame" {
                    children[i].reverse();
                    node["children"] = json!(children[i]);
                }
                if let Some(p) = s.nodes[i].parent {
                    children[p].push(node);
                } else {
                    result.nodes.push(node);
                }
            }
        }
        result.nodes.reverse();
        result.notes.sort();
        result.notes.dedup();
        Ok(result)
    }
}
