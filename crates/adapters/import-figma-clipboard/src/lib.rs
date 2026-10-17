//! Importer: Figma layer paste (ARCHITECTURE §3.6).
//!
//! A copy in Figma puts HTML on the clipboard whose `data-buffer` holds a `fig-kiwi` file: a Kiwi schema and a
//! message of node changes, each deflated. This reads that file and returns the layers as frames, groups, rects,
//! ellipses, lines, vectors and text with their positions, sizes, fills, strokes and effects. A paste never
//! carries images, so image fills are left out and the notes always say so. Not imported: gradients (their first
//! colour stands in), auto layout (children keep their positions), component links (they become frames), mixed
//! text styles (the first style is used), blend modes and constraints.
mod kiwi;

use std::collections::{BTreeMap, HashMap};
use std::io::Read;

use flate2::read::DeflateDecoder;
use serde_json::{Map, Value, json};
use vellum_core::base64;
use vellum_core::ports::{ImportError, ImportInput, Imported, Importer};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

const MAX_INFLATED: u64 = 96 * 1024 * 1024;
const MAX_NODES: usize = 20_000;
const MAX_DEPTH: usize = 64;
const MAGIC: &[u8; 8] = b"fig-kiwi";

pub struct FigmaClipboardImporter;

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The base64 text between `<!--(tag)` and `(/tag)-->`.
fn marked<'a>(html: &'a [u8], tag: &str) -> Option<&'a [u8]> {
    let open = format!("<!--({tag})");
    let close = format!("(/{tag})-->");
    let start = find(html, open.as_bytes())? + open.len();
    let end = start + find(&html[start..], close.as_bytes())?;
    Some(&html[start..end])
}

fn inflate(chunk: &[u8]) -> Result<Vec<u8>, ImportError> {
    if chunk.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return Err(ImportError::new(
            "This paste is compressed with zstd, which this version cannot read yet. Copy a smaller selection or export an SVG from Figma instead.",
        ));
    }
    let mut out = Vec::new();
    DeflateDecoder::new(chunk)
        .take(MAX_INFLATED + 1)
        .read_to_end(&mut out)
        .map_err(|_| {
            ImportError::new("The Figma paste is damaged: a section does not decompress.")
        })?;
    if out.len() as u64 > MAX_INFLATED {
        return Err(ImportError::new(
            "The Figma paste is larger than 96 MB once unpacked.",
        ));
    }
    Ok(out)
}

fn message(bytes: &[u8]) -> Result<Value, ImportError> {
    let damaged =
        |why: &str| ImportError::new(format!("The Figma paste could not be read: {why}."));
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Err(if bytes.starts_with(b"fig-jam.") {
            ImportError::new("This is a FigJam paste. Only Figma design layers are imported.")
        } else {
            damaged("it is not a fig-kiwi file")
        });
    }
    let mut chunks = Vec::new();
    let mut at = 12;
    while at + 4 <= bytes.len() && chunks.len() < 8 {
        let len =
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;
        at += 4;
        let chunk = bytes
            .get(at..at + len)
            .ok_or_else(|| damaged("a section is cut short"))?;
        chunks.push(chunk);
        at += len;
    }
    let [schema, data, ..] = chunks.as_slice() else {
        return Err(damaged("it has no data section"));
    };
    let schema = kiwi::Schema::parse(&inflate(schema)?).map_err(|e| damaged(&e))?;
    schema
        .decode("Message", &inflate(data)?)
        .map_err(|e| damaged(&e))
}

impl Importer for FigmaClipboardImporter {
    fn id(&self) -> &'static str {
        "figma-clipboard"
    }

    fn recognizes(&self, input: &ImportInput<'_>) -> bool {
        find(input.bytes, b"<!--(figma)").is_some()
    }

    fn import(&self, input: &ImportInput<'_>) -> Result<Imported, ImportError> {
        let text = marked(input.bytes, "figma").ok_or_else(|| {
            ImportError::new("This is not a Figma paste: the clipboard HTML has no Figma data.")
        })?;
        let packed = base64::decode(
            &String::from_utf8_lossy(text)
                .split_whitespace()
                .collect::<String>(),
        )
        .ok_or_else(|| {
            ImportError::new("The Figma paste is damaged: its data is not valid base64.")
        })?;
        let message = message(&packed)?;
        Convert::new(&message)?.run()
    }
}

struct Raw<'a> {
    v: &'a Value,
    parent: Option<String>,
    position: String,
}

fn key(guid: &Value) -> Option<String> {
    Some(format!(
        "{}:{}",
        guid.get("sessionID")?.as_u64()?,
        guid.get("localID")?.as_u64()?
    ))
}

struct Convert<'a> {
    nodes: Vec<Raw<'a>>,
    children: HashMap<String, Vec<usize>>,
    blobs: Vec<Vec<u8>>,
    count: BTreeMap<&'static str, usize>,
    made: usize,
}

fn num(v: f64) -> f64 {
    let r = (v * 1000.).round() / 1000.;
    if r == 0. { 0. } else { r }
}

fn f(v: &Value, name: &str) -> Option<f64> {
    v.get(name).and_then(Value::as_f64)
}

fn s<'v>(v: &'v Value, name: &str) -> Option<&'v str> {
    v.get(name).and_then(Value::as_str)
}

impl<'a> Convert<'a> {
    fn new(message: &'a Value) -> Result<Self, ImportError> {
        let changes = message
            .get("nodeChanges")
            .and_then(Value::as_array)
            .ok_or_else(|| ImportError::new("The Figma paste holds no layers."))?;
        if changes.len() > MAX_NODES {
            return Err(ImportError::new(format!(
                "The Figma paste has more than {MAX_NODES} layers."
            )));
        }
        let mut nodes = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for change in changes {
            let Some(id) = change.get("guid").and_then(key) else {
                continue;
            };
            if s(change, "phase") == Some("REMOVED") {
                continue;
            }
            let parent = change
                .get("parentIndex")
                .and_then(|p| p.get("guid"))
                .and_then(key);
            let position = change
                .get("parentIndex")
                .and_then(|p| s(p, "position"))
                .unwrap_or("")
                .to_owned();
            index.insert(id, nodes.len());
            nodes.push(Raw {
                v: change,
                parent,
                position,
            });
        }
        let mut children: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, node) in nodes.iter().enumerate() {
            if let Some(p) = &node.parent {
                children.entry(p.clone()).or_default().push(i);
            }
        }
        for list in children.values_mut() {
            list.sort_by(|a, b| nodes[*a].position.cmp(&nodes[*b].position).then(a.cmp(b)));
        }
        let blobs = message
            .get("blobs")
            .and_then(Value::as_array)
            .map(|blobs| {
                blobs
                    .iter()
                    .map(|b| s(b, "bytes").and_then(base64::decode).unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            nodes,
            children,
            blobs,
            count: BTreeMap::new(),
            made: 0,
        })
    }

    fn note(&mut self, key: &'static str) {
        *self.count.entry(key).or_default() += 1;
    }

    fn run(mut self) -> Result<Imported, ImportError> {
        let by_key: HashMap<String, usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| key(n.v.get("guid")?).map(|k| (k, i)))
            .collect();
        // Layers whose parent is not in the paste are the pasted selection; a page or document is looked through.
        let is_container =
            |i: usize| matches!(s(self.nodes[i].v, "type"), Some("DOCUMENT" | "CANVAS"));
        let mut roots: Vec<usize> = Vec::new();
        let mut stack: Vec<usize> = (0..self.nodes.len())
            .filter(|i| {
                self.nodes[*i]
                    .parent
                    .as_ref()
                    .is_none_or(|p| !by_key.contains_key(p))
            })
            .collect();
        stack.reverse();
        while let Some(i) = stack.pop() {
            if is_container(i) {
                let id =
                    key(self.nodes[i].v.get("guid").unwrap_or(&Value::Null)).unwrap_or_default();
                for c in self.children.get(&id).into_iter().flatten().rev() {
                    stack.push(*c);
                }
            } else {
                roots.push(i);
            }
        }
        let mut out: Vec<Value> = roots.iter().filter_map(|i| self.convert(*i, 0)).collect();
        if out.is_empty() {
            return Err(ImportError::new(
                "The Figma paste has no layers this importer can read.",
            ));
        }
        // The selection's top left is the origin.
        let at = |n: &Value| {
            let p = &n["position"];
            (p["x"].as_f64().unwrap_or(0.), p["y"].as_f64().unwrap_or(0.))
        };
        let (x0, y0) = out
            .iter()
            .map(at)
            .fold((f64::MAX, f64::MAX), |(a, b), (x, y)| (a.min(x), b.min(y)));
        for node in &mut out {
            let (x, y) = at(node);
            node["position"] = json!({"type": "absolute", "x": num(x - x0), "y": num(y - y0)});
        }
        let n = |k: &str| self.count.get(k).copied().unwrap_or(0);
        let mut notes = Vec::new();
        let images = n("image");
        notes.push(match images {
            0 => "A Figma paste does not carry images, so any image fills are left out.".to_owned(),
            1 => "A Figma paste does not carry images: 1 image fill was left out.".to_owned(),
            n => format!("A Figma paste does not carry images: {n} image fills were left out."),
        });
        let lines: [(&str, &str); 8] = [
            (
                "gradient",
                "gradient fill{s} became a flat colour; gradients are not imported yet.",
            ),
            (
                "auto-layout",
                "frame{s} used auto layout; children keep their positions but no longer reflow.",
            ),
            (
                "component",
                "component{s} or instance{s} became plain frames.",
            ),
            (
                "no-outline",
                "vector layer{s} had no outline data and {v} left out.",
            ),
            (
                "text-styles",
                "text layer{s} mixed styles; the first style was used for all of it.",
            ),
            (
                "unknown",
                "layer{s} of a kind that is not imported (slices, stickies, connectors and so on) {v} left out.",
            ),
            ("paint", "fill{s} of an unsupported kind {v} left out."),
            ("skew", "layer{s} {v} skewed; the skew was dropped."),
        ];
        for (k, text) in lines {
            let c = n(k);
            if c > 0 {
                let (s, v) = if c == 1 { ("", "was") } else { ("s", "were") };
                notes.push(format!("{c} {}", text.replace("{s}", s).replace("{v}", v)));
            }
        }
        Ok(Imported {
            nodes: out,
            assets: vec![],
            notes,
        })
    }

    fn convert(&mut self, i: usize, depth: usize) -> Option<Value> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.made += 1;
        if self.made > MAX_NODES {
            return None;
        }
        let v = self.nodes[i].v;
        let kind = s(v, "type").unwrap_or("");
        let size = v.get("size");
        let (w, h) = (
            size.and_then(|z| f(z, "x")).unwrap_or(0.).max(0.),
            size.and_then(|z| f(z, "y")).unwrap_or(0.).max(0.),
        );
        let mut map = Map::new();
        map.insert(
            "name".into(),
            json!(s(v, "name").filter(|n| !n.is_empty()).unwrap_or(kind)),
        );
        let (position, rotation, flip) = place(v.get("transform"), w, h, &mut self.count);
        map.insert("position".into(), position);
        if v.get("visible") == Some(&Value::Bool(false)) {
            map.insert("visible".into(), json!(false));
        }
        if let Some(o) = f(v, "opacity").filter(|o| *o < 1.) {
            map.insert("opacity".into(), json!(num(o.max(0.))));
        }
        if rotation.abs() > 0.001 {
            map.insert("rotation".into(), json!(num(rotation)));
        }
        if flip {
            map.insert("flipX".into(), json!(true));
        }
        let id = key(v.get("guid")?)?;
        let kids = |this: &mut Self, depth: usize| -> Vec<Value> {
            let list = this.children.get(&id).cloned().unwrap_or_default();
            list.into_iter()
                .filter_map(|c| this.convert(c, depth + 1))
                .collect()
        };
        let width_height = |map: &mut Map<String, Value>| {
            map.insert("width".into(), json!(num(w)));
            map.insert("height".into(), json!(num(h)));
        };
        match kind {
            "FRAME" | "SYMBOL" | "INSTANCE" | "SECTION" => {
                if matches!(kind, "SYMBOL" | "INSTANCE") {
                    self.note("component");
                }
                if matches!(s(v, "stackMode"), Some("HORIZONTAL" | "VERTICAL")) {
                    self.note("auto-layout");
                }
                map.insert("type".into(), json!("frame"));
                width_height(&mut map);
                if kind != "SECTION" && v.get("frameMaskDisabled") != Some(&Value::Bool(true)) {
                    map.insert("clip".into(), json!(true));
                }
                self.corners(v, &mut map);
                self.paints(v, &mut map);
                let children = kids(self, depth);
                if !children.is_empty() {
                    map.insert("children".into(), Value::Array(children));
                }
            }
            "GROUP" => {
                map.insert("type".into(), json!("group"));
                width_height(&mut map);
                let children = kids(self, depth);
                if children.is_empty() {
                    return None;
                }
                map.insert("children".into(), Value::Array(children));
                self.effects(v, &mut map);
            }
            "RECTANGLE" | "ROUNDED_RECTANGLE" => {
                map.insert("type".into(), json!("rect"));
                width_height(&mut map);
                self.corners(v, &mut map);
                self.paints(v, &mut map);
            }
            "ELLIPSE" => {
                map.insert("type".into(), json!("ellipse"));
                width_height(&mut map);
                self.paints(v, &mut map);
            }
            "LINE" => {
                map.insert("type".into(), json!("line"));
                map.insert("width".into(), json!(num(w)));
                map.insert("height".into(), json!(0));
                self.paints(v, &mut map);
            }
            "VECTOR" | "STAR" | "REGULAR_POLYGON" | "BOOLEAN_OPERATION" => {
                let (path, evenodd, from_stroke) = self.outline(v);
                if path.is_empty() {
                    self.note("no-outline");
                    return None;
                }
                map.insert("type".into(), json!("vector"));
                width_height(&mut map);
                map.insert("path".into(), json!(path));
                map.insert(
                    "viewBox".into(),
                    json!({"x": 0, "y": 0, "width": num(w.max(1.)), "height": num(h.max(1.))}),
                );
                if evenodd {
                    map.insert("fillRule".into(), json!("evenodd"));
                }
                if from_stroke {
                    // Only the outlined stroke is known: draw that shape in the stroke's colour.
                    let mut copy = v.clone();
                    copy["fillPaints"] = v.get("strokePaints").cloned().unwrap_or(Value::Null);
                    copy["strokePaints"] = Value::Null;
                    self.paints(&copy, &mut map);
                } else {
                    self.paints(v, &mut map);
                }
                if kind == "BOOLEAN_OPERATION" {
                    // Its children are the operands; the result is already in the outline.
                }
            }
            "TEXT" => {
                map.insert("type".into(), json!("text"));
                self.text(v, w, h, &mut map)?;
                self.paints(v, &mut map);
            }
            "DOCUMENT" | "CANVAS" => return None,
            _ => {
                self.note("unknown");
                return None;
            }
        }
        if kind != "GROUP" {
            self.effects(v, &mut map);
        }
        Some(Value::Object(map))
    }

    fn corners(&mut self, v: &Value, map: &mut Map<String, Value>) {
        let all = f(v, "cornerRadius").unwrap_or(0.);
        let each = [
            f(v, "rectangleTopLeftCornerRadius"),
            f(v, "rectangleTopRightCornerRadius"),
            f(v, "rectangleBottomRightCornerRadius"),
            f(v, "rectangleBottomLeftCornerRadius"),
        ];
        if v.get("rectangleCornerRadiiIndependent") == Some(&Value::Bool(true))
            && each.iter().any(Option::is_some)
        {
            let r: Vec<f64> = each.iter().map(|c| num(c.unwrap_or(all))).collect();
            if r.iter().any(|c| *c > 0.) {
                map.insert("cornerRadius".into(), json!(r));
            }
        } else if all > 0. {
            map.insert("cornerRadius".into(), json!(num(all)));
        }
    }

    /// `fills` and `stroke` from `fillPaints`, `strokePaints` and the stroke settings.
    fn paints(&mut self, v: &Value, map: &mut Map<String, Value>) {
        let mut fills = Vec::new();
        for paint in v
            .get("fillPaints")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(color) = self.paint(paint) {
                fills.push(json!({"type": "solid", "color": color}));
            }
        }
        if !fills.is_empty() {
            map.insert("fills".into(), Value::Array(fills));
        }
        let weight = f(v, "strokeWeight").unwrap_or(0.);
        let strokes: Vec<Value> = v
            .get("strokePaints")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| self.paint(p))
            .map(|color| json!({"type": "solid", "color": color}))
            .collect();
        if weight > 0. && !strokes.is_empty() {
            let mut stroke = Map::new();
            stroke.insert("paints".into(), Value::Array(strokes));
            stroke.insert("width".into(), json!(num(weight)));
            match s(v, "strokeAlign") {
                Some("INSIDE") => stroke.insert("align".into(), json!("inside")),
                Some("OUTSIDE") => stroke.insert("align".into(), json!("outside")),
                _ => None,
            };
            match s(v, "strokeCap") {
                Some("ROUND") => stroke.insert("cap".into(), json!("round")),
                Some("SQUARE") => stroke.insert("cap".into(), json!("square")),
                _ => None,
            };
            match s(v, "strokeJoin") {
                Some("ROUND") => stroke.insert("join".into(), json!("round")),
                Some("BEVEL") => stroke.insert("join".into(), json!("bevel")),
                _ => None,
            };
            let dash: Vec<f64> = v
                .get("dashPattern")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_f64)
                .map(num)
                .collect();
            if !dash.is_empty() && dash.iter().any(|d| *d > 0.) {
                stroke.insert("dash".into(), json!(dash));
            }
            map.insert("stroke".into(), Value::Object(stroke));
        }
    }

    /// One paint as `#RRGGBB[AA]`, or `None` when it is hidden or cannot be drawn.
    fn paint(&mut self, p: &Value) -> Option<String> {
        if p.get("visible") == Some(&Value::Bool(false)) {
            return None;
        }
        let opacity = f(p, "opacity").unwrap_or(1.);
        let (color, kind) = (p.get("color"), s(p, "type").unwrap_or("SOLID"));
        match kind {
            "SOLID" => hex(color?, opacity),
            "IMAGE" => {
                self.note("image");
                None
            }
            k if k.starts_with("GRADIENT") => {
                self.note("gradient");
                let stop = p.get("stops")?.as_array()?.first()?;
                hex(stop.get("color")?, opacity)
            }
            _ => {
                self.note("paint");
                None
            }
        }
    }

    fn effects(&mut self, v: &Value, map: &mut Map<String, Value>) {
        let mut out = Vec::new();
        for e in v
            .get("effects")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if e.get("visible") == Some(&Value::Bool(false)) {
                continue;
            }
            let radius = num(f(e, "radius").unwrap_or(0.));
            match s(e, "type") {
                Some(t @ ("DROP_SHADOW" | "INNER_SHADOW")) => {
                    let Some(color) = e.get("color").and_then(|c| hex(c, 1.)) else {
                        continue;
                    };
                    let offset = e.get("offset");
                    out.push(json!({
                        "type": if t == "DROP_SHADOW" { "drop-shadow" } else { "inner-shadow" },
                        "x": num(offset.and_then(|o| f(o, "x")).unwrap_or(0.)),
                        "y": num(offset.and_then(|o| f(o, "y")).unwrap_or(0.)),
                        "blur": radius,
                        "spread": num(f(e, "spread").unwrap_or(0.)),
                        "color": color,
                    }));
                }
                Some("FOREGROUND_BLUR") => {
                    out.push(json!({"type": "layer-blur", "radius": radius}))
                }
                Some("BACKGROUND_BLUR") => {
                    out.push(json!({"type": "background-blur", "radius": radius}))
                }
                _ => {}
            }
        }
        if !out.is_empty() {
            map.insert("effects".into(), Value::Array(out));
        }
    }

    /// The outline of a vector as path data: its fill geometry, or its stroke geometry when that is all there is.
    fn outline(&self, v: &Value) -> (String, bool, bool) {
        for (field, from_stroke) in [("fillGeometry", false), ("strokeGeometry", true)] {
            let mut d = String::new();
            let mut evenodd = false;
            for g in v.get(field).and_then(Value::as_array).into_iter().flatten() {
                let blob = g
                    .get("commandsBlob")
                    .and_then(Value::as_u64)
                    .and_then(|b| self.blobs.get(b as usize));
                if let Some(blob) = blob {
                    if s(g, "windingRule") == Some("ODD") {
                        evenodd = true;
                    }
                    if !d.is_empty() {
                        d.push(' ');
                    }
                    d.push_str(&commands(blob));
                }
            }
            let d = d.trim().to_owned();
            if !d.is_empty() {
                return (d, evenodd, from_stroke);
            }
        }
        (String::new(), false, false)
    }

    fn text(&mut self, v: &Value, w: f64, h: f64, map: &mut Map<String, Value>) -> Option<()> {
        let data = v.get("textData");
        let characters = data
            .and_then(|d| s(d, "characters"))
            .or_else(|| s(v, "characters"))?;
        if characters.is_empty() {
            return None;
        }
        if data
            .and_then(|d| d.get("styleOverrideTable"))
            .and_then(Value::as_array)
            .is_some_and(|t| !t.is_empty())
        {
            self.note("text-styles");
        }
        let size = f(v, "fontSize").unwrap_or(14.);
        let font = v.get("fontName");
        let family = font.and_then(|n| s(n, "family")).unwrap_or("Inter");
        let style = font
            .and_then(|n| s(n, "style"))
            .unwrap_or("Regular")
            .to_lowercase();
        let weight = [
            ("extralight", 200),
            ("ultralight", 200),
            ("extrabold", 800),
            ("ultrabold", 800),
            ("semibold", 600),
            ("demibold", 600),
            ("thin", 100),
            ("light", 300),
            ("medium", 500),
            ("bold", 700),
            ("black", 900),
            ("heavy", 900),
        ]
        .iter()
        .find(|(word, _)| style.replace([' ', '-'], "").contains(word))
        .map_or(400, |(_, w)| *w);
        let unit = |name: &str| -> Option<f64> {
            let n = v.get(name)?;
            let value = f(n, "value")?;
            match s(n, "units") {
                Some("PERCENT") => Some(value / 100. * size),
                Some("RAW") => Some(value * size),
                Some("PIXELS") => Some(value),
                _ => None,
            }
        };
        let mut run = Map::new();
        run.insert("text".into(), json!(characters));
        run.insert("font".into(), json!(family));
        if weight != 400 {
            run.insert("weight".into(), json!(weight));
        }
        if style.contains("italic") || style.contains("oblique") {
            run.insert("style".into(), json!("italic"));
        }
        run.insert("size".into(), json!(num(size)));
        if let Some(lh) = unit("lineHeight").filter(|l| *l > 0.) {
            run.insert("lineHeight".into(), json!(num(lh)));
        }
        if let Some(ls) = unit("letterSpacing").filter(|l| l.abs() > 1e-6) {
            run.insert("letterSpacing".into(), json!(num(ls)));
        }
        if matches!(s(v, "textDecoration"), Some("UNDERLINE")) {
            run.insert("underline".into(), json!(true));
        }
        if matches!(s(v, "textDecoration"), Some("STRIKETHROUGH")) {
            run.insert("strikethrough".into(), json!(true));
        }
        map.insert("content".into(), json!([Value::Object(run)]));
        match s(v, "textAlignHorizontal") {
            Some("CENTER") => map.insert("align".into(), json!("center")),
            Some("RIGHT") => map.insert("align".into(), json!("right")),
            Some("JUSTIFIED") => map.insert("align".into(), json!("justify")),
            _ => None,
        };
        match s(v, "textAlignVertical") {
            Some("CENTER") => map.insert("verticalAlign".into(), json!("middle")),
            Some("BOTTOM") => map.insert("verticalAlign".into(), json!("bottom")),
            _ => None,
        };
        match s(v, "textAutoResize") {
            Some("WIDTH_AND_HEIGHT") => {
                map.insert("width".into(), json!("hug"));
                map.insert("height".into(), json!("hug"));
                map.insert("resize".into(), json!("auto-width"));
            }
            Some("HEIGHT") => {
                map.insert("width".into(), json!(num(w)));
                map.insert("height".into(), json!("hug"));
                map.insert("resize".into(), json!("auto-height"));
            }
            _ => {
                map.insert("width".into(), json!(num(w)));
                map.insert("height".into(), json!(num(h)));
                map.insert("resize".into(), json!("fixed"));
            }
        }
        Some(())
    }
}

fn hex(color: &Value, opacity: f64) -> Option<String> {
    let channel = |n: &str| f(color, n).map(|c| (c.clamp(0., 1.) * 255.).round() as u8);
    let (r, g, b) = (channel("r")?, channel("g")?, channel("b")?);
    let a = (f(color, "a").unwrap_or(1.) * opacity).clamp(0., 1.);
    if a <= 0. {
        return None;
    }
    Some(if a < 1. {
        format!("#{r:02X}{g:02X}{b:02X}{:02X}", (a * 255.).round() as u8)
    } else {
        format!("#{r:02X}{g:02X}{b:02X}")
    })
}

/// Position, rotation and flip of a layer from its transform, with the position being where the unrotated box's
/// top left must sit so that rotating about the centre gives the same result.
fn place(
    transform: Option<&Value>,
    w: f64,
    h: f64,
    count: &mut BTreeMap<&'static str, usize>,
) -> (Value, f64, bool) {
    let m = |n: &str, default: f64| transform.and_then(|t| f(t, n)).unwrap_or(default);
    let (m00, m01, m02, m10, m11, m12) = (
        m("m00", 1.),
        m("m01", 0.),
        m("m02", 0.),
        m("m10", 0.),
        m("m11", 1.),
        m("m12", 0.),
    );
    let det = m00 * m11 - m01 * m10;
    let flip = det < 0.;
    let angle = if flip {
        (-m10).atan2(-m00)
    } else {
        m10.atan2(m00)
    };
    let (sin, cos) = angle.sin_cos();
    // After removing rotation and flip, what is left should be the identity; if not, there is skew.
    let ex = if flip { -1. } else { 1. };
    let off = |actual: f64, expected: f64| (actual - expected).abs() > 0.01;
    if off(m00, cos * ex) || off(m11, cos) || off(m01, -sin) || off(m10, sin * ex) {
        *count.entry("skew").or_default() += 1;
    }
    let (cx, cy) = (
        m00 * w / 2. + m01 * h / 2. + m02,
        m10 * w / 2. + m11 * h / 2. + m12,
    );
    let position = json!({"type": "absolute", "x": num(cx - w / 2.), "y": num(cy - h / 2.)});
    (position, angle.to_degrees(), flip)
}

/// A vector blob: one command byte, then little-endian f32 coordinates. 0 closes, 1 moves, 2 draws a line,
/// 3 a cubic curve, 4 a quadratic curve.
fn commands(blob: &[u8]) -> String {
    let mut out = Vec::new();
    let mut at = 0;
    let mut pt = (0., 0.);
    let read = |at: &mut usize, n: usize| -> Option<Vec<f64>> {
        let bytes = blob.get(*at..*at + 4 * n)?;
        *at += 4 * n;
        Some(
            bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f64::from(f32::from_le_bytes(*c)))
                .collect(),
        )
    };
    let n = |v: f64| format!("{}", num(v));
    while let Some(&cmd) = blob.get(at) {
        at += 1;
        let text = match cmd {
            0 => "Z".to_owned(),
            1 => match read(&mut at, 2) {
                Some(p) => {
                    pt = (p[0], p[1]);
                    format!("M{} {}", n(p[0]), n(p[1]))
                }
                None => break,
            },
            2 => match read(&mut at, 2) {
                Some(p) => {
                    pt = (p[0], p[1]);
                    format!("L{} {}", n(p[0]), n(p[1]))
                }
                None => break,
            },
            3 => match read(&mut at, 6) {
                Some(p) => {
                    pt = (p[4], p[5]);
                    format!(
                        "C{} {} {} {} {} {}",
                        n(p[0]),
                        n(p[1]),
                        n(p[2]),
                        n(p[3]),
                        n(p[4]),
                        n(p[5])
                    )
                }
                None => break,
            },
            4 => match read(&mut at, 4) {
                // Quadratic to cubic, so the path only ever holds M, L, C and Z.
                Some(p) => {
                    let (x0, y0) = pt;
                    pt = (p[2], p[3]);
                    format!(
                        "C{} {} {} {} {} {}",
                        n(x0 + 2. / 3. * (p[0] - x0)),
                        n(y0 + 2. / 3. * (p[1] - y0)),
                        n(p[2] + 2. / 3. * (p[0] - p[2])),
                        n(p[3] + 2. / 3. * (p[1] - p[3])),
                        n(p[2]),
                        n(p[3])
                    )
                }
                None => break,
            },
            _ => break,
        };
        out.push(text);
    }
    out.join(" ")
}
