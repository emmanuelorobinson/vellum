//! Importer: SVG as editable layers (ARCHITECTURE §3.6).
//!
//! One frame the size of the drawing, holding a layer per shape in paint order: `rect` and `ellipse` layers where
//! the shape stays axis-aligned, `vector` layers (path data with curves, arcs baked to Béziers, transforms
//! applied) for everything else, `text` layers and `image` layers for embedded pictures. Groups are flattened:
//! their transform, opacity and inherited style are applied to each shape. What is not imported (gradients,
//! patterns, clip paths, masks, filters, linked images) is counted in `notes`.
mod color;
mod css;
mod path;
mod xml;

use std::collections::{BTreeMap, HashMap};

use color::{PaintValue, Rgba, parse_color, parse_paint};
use path::{Affine, Seg};
use serde_json::{Map, Value, json};
use vellum_core::ports::{ImportError, ImportInput, Imported, ImportedAsset, Importer, asset_ref};
use xml::Element;

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The largest file read.
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// The most shapes imported from one file.
const MAX_SHAPES: usize = 5_000;
/// How deep `use` may reference `use`.
const MAX_USE_DEPTH: usize = 8;

pub struct SvgImporter;

impl Importer for SvgImporter {
    fn id(&self) -> &'static str {
        "svg"
    }

    fn recognizes(&self, input: &ImportInput<'_>) -> bool {
        let head = &input.bytes[..input.bytes.len().min(2048)];
        head.iter().all(|b| *b != 0) && String::from_utf8_lossy(head).contains("<svg")
    }

    fn import(&self, input: &ImportInput<'_>) -> Result<Imported, ImportError> {
        if input.bytes.len() > MAX_BYTES {
            return Err(ImportError::new(format!(
                "The SVG is {} KB; files over {} KB are not imported.",
                input.bytes.len() / 1024,
                MAX_BYTES / 1024
            )));
        }
        let text = std::str::from_utf8(input.bytes)
            .map_err(|_| ImportError::new("The SVG is not UTF-8 text."))?;
        let root = xml::parse(text)
            .map_err(|e| ImportError::new(format!("The SVG could not be read: {e}")))?;
        if root.name != "svg" {
            return Err(ImportError::new(format!(
                "The file is XML but its first element is <{}>, not <svg>.",
                root.name
            )));
        }
        Conv::new(&root).run(&root, input.name)
    }
}

#[derive(Default)]
struct Notes(BTreeMap<&'static str, usize>);

impl Notes {
    fn add(&mut self, key: &'static str) {
        *self.0.entry(key).or_default() += 1;
    }
    fn lines(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|(key, n)| {
                let n = *n;
                let (s, v) = if n == 1 { ("", "was") } else { ("s", "were") };
                match *key {
                    "paint" => format!("{n} gradient or pattern fill{s} became a flat colour; gradients and patterns are not imported yet."),
                    "effect" => format!("{n} clip path{s}, mask{s} or filter{s} {v} left out."),
                    "text-transform" => format!("{n} rotated or skewed text{s} {v} imported unrotated."),
                    "css" => format!("{n} CSS selector{s} or rule{s} {v} not understood and ignored."),
                    "link" => format!("{n} linked image{s} {v} left out; only embedded images are imported."),
                    "path" => format!("{n} path{s} had errors; the part before each error was kept."),
                    "foreign" => format!("{n} foreignObject{s} {v} left out."),
                    "transform" => format!("{n} transform{s} could not be read and {v} ignored."),
                    "shapes" => format!("Only the first {MAX_SHAPES} shapes were imported."),
                    "image" => format!("{n} embedded image{s} {v} not PNG, JPEG or WEBP and {v} left out."),
                    "dasharray" => format!("{n} dash pattern{s} could not be read and {v} ignored."),
                    other => other.to_owned(),
                }
            })
            .collect()
    }
}

/// The inherited and per-element presentation state.
#[derive(Clone)]
struct Style {
    fill: PaintValue,
    fill_opacity: f64,
    evenodd: bool,
    stroke: PaintValue,
    stroke_opacity: f64,
    stroke_width: f64,
    cap: &'static str,
    join: &'static str,
    dash: Vec<f64>,
    /// Multiplied down the tree (an element's `opacity` applies to it and everything in it).
    opacity: f64,
    color: Rgba,
    font_family: String,
    font_size: f64,
    font_weight: u16,
    italic: bool,
    anchor: &'static str,
    hidden: bool,
}

impl Style {
    fn root() -> Self {
        Self {
            fill: PaintValue::Color(Rgba::BLACK),
            fill_opacity: 1.,
            evenodd: false,
            stroke: PaintValue::None,
            stroke_opacity: 1.,
            stroke_width: 1.,
            cap: "butt",
            join: "miter",
            dash: vec![],
            opacity: 1.,
            color: Rgba::BLACK,
            font_family: "Inter".into(),
            font_size: 16.,
            font_weight: 400,
            italic: false,
            anchor: "start",
            hidden: false,
        }
    }
}

struct Conv<'a> {
    ids: HashMap<&'a str, &'a Element>,
    rules: Vec<css::Rule>,
    notes: Notes,
    nodes: Vec<Value>,
    assets: Vec<ImportedAsset>,
    shapes: usize,
    /// The viewport percentages refer to.
    viewport: (f64, f64),
}

fn collect<'a>(el: &'a Element, ids: &mut HashMap<&'a str, &'a Element>, css: &mut String) {
    if let Some(id) = el.attr("id") {
        ids.entry(id).or_insert(el);
    }
    if el.name == "style" {
        css.push_str(&el.text());
        css.push('\n');
    }
    for child in el.elements() {
        collect(child, ids, css);
    }
}

impl<'a> Conv<'a> {
    fn new(root: &'a Element) -> Self {
        let mut ids = HashMap::new();
        let mut sheet = String::new();
        collect(root, &mut ids, &mut sheet);
        let (rules, skipped) = css::parse(&sheet);
        let mut notes = Notes::default();
        for _ in 0..skipped {
            notes.add("css");
        }
        Self {
            ids,
            rules,
            notes,
            nodes: vec![],
            assets: vec![],
            shapes: 0,
            viewport: (100., 100.),
        }
    }

    fn run(mut self, root: &'a Element, file: &str) -> Result<Imported, ImportError> {
        let view_box = root.attr("viewBox").and_then(|v| {
            let n: Vec<f64> = v
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().ok())
                .collect::<Option<_>>()?;
            (n.len() == 4 && n[2] > 0. && n[3] > 0.).then(|| [n[0], n[1], n[2], n[3]])
        });
        let reference = view_box.map_or((300., 150.), |v| (v[2], v[3]));
        let width = root.attr("width").and_then(|v| length(v, reference.0, 16.));
        let height = root
            .attr("height")
            .and_then(|v| length(v, reference.1, 16.));
        let (w, h) = match (view_box, width, height) {
            (_, Some(w), Some(h)) => (w, h),
            (Some(v), Some(w), None) => (w, w * v[3] / v[2]),
            (Some(v), None, Some(h)) => (h * v[2] / v[3], h),
            (Some(v), None, None) => (v[2], v[3]),
            (None, w, h) => (w.unwrap_or(0.), h.unwrap_or(0.)),
        };
        if view_box.is_none()
            && (w <= 0. || h <= 0.)
            && root.attr("width").is_none()
            && root.attr("height").is_none()
        {
            // No size at all: the drawing's own extent decides, below.
        } else if !(w > 0.
            && h > 0.
            && w.is_finite()
            && h.is_finite()
            && w <= 65_536.
            && h <= 65_536.)
        {
            return Err(ImportError::new(
                "The SVG has no usable size: give it a width and height or a viewBox.",
            ));
        }
        let base = match view_box {
            Some(v) if w > 0. && h > 0. => {
                let none = root
                    .attr("preserveAspectRatio")
                    .is_some_and(|p| p.trim().starts_with("none"));
                let (sx, sy) = (w / v[2], h / v[3]);
                if none {
                    Affine([sx, 0., 0., sy, -v[0] * sx, -v[1] * sy])
                } else {
                    let s = sx.min(sy);
                    Affine([
                        s,
                        0.,
                        0.,
                        s,
                        (w - v[2] * s) / 2. - v[0] * s,
                        (h - v[3] * s) / 2. - v[1] * s,
                    ])
                }
            }
            _ => Affine::IDENTITY,
        };
        self.viewport = view_box.map_or((w.max(1.), h.max(1.)), |v| (v[2], v[3]));
        self.walk(root, &Style::root(), base, 0, true);
        let (mut fw, mut fh) = (w, h);
        if fw <= 0. || fh <= 0. {
            for n in &self.nodes {
                let (x, y) = n["position"].as_object().map_or((0., 0.), |p| {
                    (p["x"].as_f64().unwrap_or(0.), p["y"].as_f64().unwrap_or(0.))
                });
                fw = fw.max(x + n["width"].as_f64().unwrap_or(0.));
                fh = fh.max(y + n["height"].as_f64().unwrap_or(0.));
            }
            fw = fw.ceil().max(1.);
            fh = fh.ceil().max(1.);
        }
        if self.nodes.is_empty() {
            return Err(ImportError::new(
                "The SVG draws nothing this importer can read.",
            ));
        }
        let stem = file.rsplit(['/', '\\']).next().unwrap_or("");
        let stem = stem
            .strip_suffix(".svg")
            .or_else(|| stem.strip_suffix(".SVG"))
            .unwrap_or(stem);
        let name = if stem.trim().is_empty() { "SVG" } else { stem };
        let frame = json!({
            "type": "frame",
            "name": name,
            "width": path::num(fw).parse::<f64>().unwrap_or(fw),
            "height": path::num(fh).parse::<f64>().unwrap_or(fh),
            "clip": true,
            "children": self.nodes,
        });
        Ok(Imported {
            nodes: vec![frame],
            assets: self.assets,
            notes: self.notes.lines(),
        })
    }

    fn style_of(&mut self, el: &Element, parent: &Style) -> Style {
        let mut st = parent.clone();
        let mut decls: Vec<(String, String)> = Vec::new();
        for (name, value) in &el.attrs {
            if PROPERTIES.contains(&name.as_str()) {
                decls.push((name.clone(), value.trim().to_owned()));
            }
        }
        for rule in &self.rules {
            if rule.matches(el) {
                decls.extend(rule.decls.iter().cloned());
            }
        }
        if let Some(style) = el.attr("style") {
            decls.extend(css::declarations(style));
        }
        let mut own_opacity = 1.;
        for (name, value) in decls {
            match name.as_str() {
                "fill" => {
                    if let Some(p) = parse_paint(&value) {
                        st.fill = p;
                    }
                }
                "stroke" => {
                    if let Some(p) = parse_paint(&value) {
                        st.stroke = p;
                    }
                }
                "fill-opacity" => st.fill_opacity = unit(&value).unwrap_or(st.fill_opacity),
                "stroke-opacity" => st.stroke_opacity = unit(&value).unwrap_or(st.stroke_opacity),
                "fill-rule" => st.evenodd = value == "evenodd",
                "stroke-width" => {
                    if let Some(w) = length(&value, self.viewport.0, st.font_size) {
                        st.stroke_width = w.max(0.);
                    }
                }
                "stroke-linecap" => {
                    st.cap = match value.as_str() {
                        "round" => "round",
                        "square" => "square",
                        _ => "butt",
                    }
                }
                "stroke-linejoin" => {
                    st.join = match value.as_str() {
                        "round" => "round",
                        "bevel" => "bevel",
                        _ => "miter",
                    }
                }
                "stroke-dasharray" => {
                    if value == "none" {
                        st.dash.clear();
                    } else {
                        let parsed: Option<Vec<f64>> = value
                            .split(|c: char| c == ',' || c.is_whitespace())
                            .filter(|s| !s.is_empty())
                            .map(|s| length(s, self.viewport.0, st.font_size))
                            .collect();
                        match parsed {
                            Some(mut d)
                                if d.iter().all(|v| *v >= 0.) && d.iter().any(|v| *v > 0.) =>
                            {
                                if d.len() % 2 == 1 {
                                    let again = d.clone();
                                    d.extend(again);
                                }
                                st.dash = d;
                            }
                            _ => {
                                st.dash.clear();
                                self.notes.add("dasharray");
                            }
                        }
                    }
                }
                "opacity" => own_opacity = unit(&value).unwrap_or(1.),
                "color" => {
                    if let Some(c) = parse_color(&value) {
                        st.color = c;
                    }
                }
                "font-family" => {
                    let first = value
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches(['"', '\'']);
                    if !first.is_empty() {
                        st.font_family = match first {
                            "sans-serif" | "system-ui" => "Inter".into(),
                            other => other.to_owned(),
                        };
                    }
                }
                "font-size" => {
                    if let Some(s) = length(&value, st.font_size, st.font_size).filter(|s| *s > 0.)
                    {
                        st.font_size = s;
                    }
                }
                "font-weight" => {
                    st.font_weight = match value.as_str() {
                        "bold" => 700,
                        "normal" => 400,
                        other => other
                            .parse()
                            .ok()
                            .filter(|w| (100..=900).contains(w))
                            .unwrap_or(st.font_weight),
                    }
                }
                "font-style" => st.italic = matches!(value.as_str(), "italic" | "oblique"),
                "text-anchor" => {
                    st.anchor = match value.as_str() {
                        "middle" => "middle",
                        "end" => "end",
                        _ => "start",
                    }
                }
                "display" => {
                    if value == "none" {
                        st.hidden = true;
                    }
                }
                "visibility" => {
                    if matches!(value.as_str(), "hidden" | "collapse") {
                        st.hidden = true;
                    } else if value == "visible" {
                        st.hidden = false;
                    }
                }
                _ => {}
            }
        }
        st.opacity *= own_opacity;
        st
    }

    fn walk(&mut self, el: &'a Element, parent: &Style, ctm: Affine, depth: usize, is_root: bool) {
        let name = el.name.as_str();
        if matches!(
            name,
            "defs"
                | "title"
                | "desc"
                | "metadata"
                | "style"
                | "linearGradient"
                | "radialGradient"
                | "pattern"
                | "clipPath"
                | "mask"
                | "filter"
                | "marker"
                | "script"
                | "symbol"
                | "stop"
                | "font"
        ) {
            return;
        }
        let st = self.style_of(el, parent);
        if st.hidden {
            return;
        }
        let mut ctm = ctm;
        if let Some(t) = el.attr("transform") {
            match path::parse_transform(t) {
                Some(local) => ctm = ctm.then_after(local),
                None => self.notes.add("transform"),
            }
        }
        for effect in ["clip-path", "mask", "filter"] {
            if el.attr(effect).is_some_and(|v| v.trim() != "none") {
                self.notes.add("effect");
            }
        }
        if self.shapes >= MAX_SHAPES {
            self.notes.add("shapes");
            return;
        }
        match name {
            "svg" | "g" | "a" | "switch" => {
                let mut ctm = ctm;
                if !is_root && name == "svg" {
                    let x = el
                        .attr("x")
                        .and_then(|v| length(v, self.viewport.0, 16.))
                        .unwrap_or(0.);
                    let y = el
                        .attr("y")
                        .and_then(|v| length(v, self.viewport.1, 16.))
                        .unwrap_or(0.);
                    ctm = ctm.then_after(Affine::translate(x, y));
                }
                let st = st.clone();
                for child in el.elements() {
                    self.walk(child, &st, ctm, depth, false);
                }
            }
            "use" => self.use_element(el, &st, ctm, depth),
            "rect" => self.rect(el, &st, ctm),
            "circle" | "ellipse" => self.ellipse(el, &st, ctm),
            "line" | "polyline" | "polygon" | "path" => self.path_shape(el, &st, ctm),
            "text" => self.text(el, &st, ctm),
            "image" => self.image(el, &st, ctm),
            "foreignObject" => self.notes.add("foreign"),
            _ => {}
        }
    }

    fn use_element(&mut self, el: &'a Element, st: &Style, ctm: Affine, depth: usize) {
        if depth >= MAX_USE_DEPTH {
            return;
        }
        let href = el
            .attr("href")
            .or_else(|| el.attr("xlink:href"))
            .unwrap_or("");
        let Some(target) = href
            .strip_prefix('#')
            .and_then(|id| self.ids.get(id).copied())
        else {
            return;
        };
        let x = el
            .attr("x")
            .and_then(|v| length(v, self.viewport.0, st.font_size))
            .unwrap_or(0.);
        let y = el
            .attr("y")
            .and_then(|v| length(v, self.viewport.1, st.font_size))
            .unwrap_or(0.);
        let ctm = ctm.then_after(Affine::translate(x, y));
        if matches!(target.name.as_str(), "symbol" | "svg") {
            // The referenced container's own children draw; its viewBox scaling is not applied.
            let inner = self.style_of(target, st);
            for child in target.elements() {
                self.walk(child, &inner, ctm, depth + 1, false);
            }
        } else {
            self.walk(target, st, ctm, depth + 1, false);
        }
    }

    fn resolve(&mut self, paint: &PaintValue, st: &Style) -> Option<Rgba> {
        match paint {
            PaintValue::None => None,
            PaintValue::Color(c) => Some(*c),
            PaintValue::CurrentColor => Some(st.color),
            PaintValue::Url(id, fallback) => {
                self.notes.add("paint");
                let from_stops = self
                    .ids
                    .get(id.as_str())
                    .copied()
                    .and_then(|g| self.first_stop(g, 0));
                from_stops.or(*fallback)
            }
        }
    }

    fn first_stop(&self, gradient: &Element, hops: usize) -> Option<Rgba> {
        if let Some(stop) = gradient.elements().find(|e| e.name == "stop") {
            let mut color = Rgba::BLACK;
            let mut alpha = 1.;
            let mut apply = |name: &str, value: &str| match name {
                "stop-color" => color = parse_color(value).unwrap_or(color),
                "stop-opacity" => alpha = unit(value).unwrap_or(1.),
                _ => {}
            };
            for (n, v) in &stop.attrs {
                apply(n, v);
            }
            if let Some(style) = stop.attr("style") {
                for (n, v) in css::declarations(style) {
                    apply(&n, &v);
                }
            }
            return Some(color.with_alpha(alpha));
        }
        // A gradient may borrow its stops from another.
        let href = gradient
            .attr("href")
            .or_else(|| gradient.attr("xlink:href"))?;
        if hops > 4 {
            return None;
        }
        let next = self.ids.get(href.strip_prefix('#')?).copied()?;
        self.first_stop(next, hops + 1)
    }

    /// The `fills`, `stroke` and `opacity` properties of a node for `st`, with lengths scaled by `scale`.
    fn paints(&mut self, st: &Style, scale: f64, map: &mut Map<String, Value>) {
        if let Some(c) = self.resolve(&st.fill.clone(), st) {
            let c = c.with_alpha(st.fill_opacity);
            if c.a > 0. {
                map.insert("fills".into(), json!([{"type": "solid", "color": c.hex()}]));
            }
        }
        if let Some(c) = self.resolve(&st.stroke.clone(), st) {
            let c = c.with_alpha(st.stroke_opacity);
            let width = st.stroke_width * scale;
            if c.a > 0. && width > 0. {
                let mut stroke = Map::new();
                stroke.insert(
                    "paints".into(),
                    json!([{"type": "solid", "color": c.hex()}]),
                );
                stroke.insert("width".into(), json!(num(width)));
                if st.cap != "butt" {
                    stroke.insert("cap".into(), json!(st.cap));
                }
                if st.join != "miter" {
                    stroke.insert("join".into(), json!(st.join));
                }
                if !st.dash.is_empty() {
                    stroke.insert(
                        "dash".into(),
                        json!(st.dash.iter().map(|d| num(d * scale)).collect::<Vec<_>>()),
                    );
                }
                map.insert("stroke".into(), Value::Object(stroke));
            }
        }
        if st.opacity < 1. {
            map.insert("opacity".into(), json!(num(st.opacity)));
        }
    }

    fn add(&mut self, node: Value) {
        self.shapes += 1;
        self.nodes.push(node);
    }

    fn rect(&mut self, el: &Element, st: &Style, ctm: Affine) {
        let (vw, vh) = self.viewport;
        let get = |n: &str, r: f64| el.attr(n).and_then(|v| length(v, r, st.font_size));
        let (x, y) = (get("x", vw).unwrap_or(0.), get("y", vh).unwrap_or(0.));
        let (Some(w), Some(h)) = (get("width", vw), get("height", vh)) else {
            return;
        };
        if w <= 0. || h <= 0. {
            return;
        }
        let (mut rx, mut ry) = (get("rx", vw), get("ry", vh));
        match (rx, ry) {
            (Some(a), None) => ry = Some(a),
            (None, Some(b)) => rx = Some(b),
            _ => {}
        }
        let (rx, ry) = (
            rx.unwrap_or(0.).clamp(0., w / 2.),
            ry.unwrap_or(0.).clamp(0., h / 2.),
        );
        let name = layer_name(el, "Rectangle");
        let [a, _, _, d, ..] = ctm.0;
        let mut map = Map::new();
        if ctm.is_axis_aligned() && (rx * a.abs() - ry * d.abs()).abs() < 1e-6 {
            let ((x0, y0), (x1, y1)) = (ctm.apply(x, y), ctm.apply(x + w, y + h));
            map.insert("type".into(), json!("rect"));
            map.insert("name".into(), json!(name));
            put_box(
                &mut map,
                x0.min(x1),
                y0.min(y1),
                (x1 - x0).abs(),
                (y1 - y0).abs(),
            );
            if rx > 0. {
                map.insert("cornerRadius".into(), json!(num(rx * a.abs())));
            }
            self.paints(st, ctm.mean_scale(), &mut map);
            self.add(Value::Object(map));
            return;
        }
        let mut segs = Vec::new();
        if rx == 0. || ry == 0. {
            segs.extend([
                Seg::M(x, y),
                Seg::L(x + w, y),
                Seg::L(x + w, y + h),
                Seg::L(x, y + h),
                Seg::Z,
            ]);
        } else {
            segs.push(Seg::M(x + rx, y));
            segs.push(Seg::L(x + w - rx, y));
            path::arc(
                &mut segs,
                x + w - rx,
                y,
                rx,
                ry,
                0.,
                false,
                true,
                x + w,
                y + ry,
            );
            segs.push(Seg::L(x + w, y + h - ry));
            path::arc(
                &mut segs,
                x + w,
                y + h - ry,
                rx,
                ry,
                0.,
                false,
                true,
                x + w - rx,
                y + h,
            );
            segs.push(Seg::L(x + rx, y + h));
            path::arc(
                &mut segs,
                x + rx,
                y + h,
                rx,
                ry,
                0.,
                false,
                true,
                x,
                y + h - ry,
            );
            segs.push(Seg::L(x, y + ry));
            path::arc(&mut segs, x, y + ry, rx, ry, 0., false, true, x + rx, y);
            segs.push(Seg::Z);
        }
        self.vector(&name, &segs, st, ctm);
    }

    fn ellipse(&mut self, el: &Element, st: &Style, ctm: Affine) {
        let (vw, vh) = self.viewport;
        let get = |n: &str, r: f64| el.attr(n).and_then(|v| length(v, r, st.font_size));
        let (cx, cy) = (get("cx", vw).unwrap_or(0.), get("cy", vh).unwrap_or(0.));
        let (rx, ry) = if el.name == "circle" {
            let r = get("r", (vw * vw + vh * vh).sqrt() / std::f64::consts::SQRT_2).unwrap_or(0.);
            (r, r)
        } else {
            let (rx, ry) = (get("rx", vw), get("ry", vh));
            (rx.or(ry).unwrap_or(0.), ry.or(rx).unwrap_or(0.))
        };
        if rx <= 0. || ry <= 0. {
            return;
        }
        let name = layer_name(
            el,
            if el.name == "circle" {
                "Circle"
            } else {
                "Ellipse"
            },
        );
        if ctm.is_axis_aligned() {
            let ((x0, y0), (x1, y1)) = (ctm.apply(cx - rx, cy - ry), ctm.apply(cx + rx, cy + ry));
            let mut map = Map::new();
            map.insert("type".into(), json!("ellipse"));
            map.insert("name".into(), json!(name));
            put_box(
                &mut map,
                x0.min(x1),
                y0.min(y1),
                (x1 - x0).abs(),
                (y1 - y0).abs(),
            );
            self.paints(st, ctm.mean_scale(), &mut map);
            self.add(Value::Object(map));
            return;
        }
        let mut segs = vec![Seg::M(cx + rx, cy)];
        path::arc(&mut segs, cx + rx, cy, rx, ry, 0., false, true, cx, cy + ry);
        path::arc(&mut segs, cx, cy + ry, rx, ry, 0., false, true, cx - rx, cy);
        path::arc(&mut segs, cx - rx, cy, rx, ry, 0., false, true, cx, cy - ry);
        path::arc(&mut segs, cx, cy - ry, rx, ry, 0., false, true, cx + rx, cy);
        segs.push(Seg::Z);
        self.vector(&name, &segs, st, ctm);
    }

    fn path_shape(&mut self, el: &Element, st: &Style, ctm: Affine) {
        let (vw, vh) = self.viewport;
        let num_attr = |n: &str, r: f64| {
            el.attr(n)
                .and_then(|v| length(v, r, st.font_size))
                .unwrap_or(0.)
        };
        let segs = match el.name.as_str() {
            "line" => vec![
                Seg::M(num_attr("x1", vw), num_attr("y1", vh)),
                Seg::L(num_attr("x2", vw), num_attr("y2", vh)),
            ],
            "polyline" | "polygon" => {
                let nums: Vec<f64> = el
                    .attr("points")
                    .unwrap_or("")
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                let mut segs: Vec<Seg> = nums
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        if i == 0 {
                            Seg::M(p[0], p[1])
                        } else {
                            Seg::L(p[0], p[1])
                        }
                    })
                    .collect();
                if el.name == "polygon" && !segs.is_empty() {
                    segs.push(Seg::Z);
                }
                segs
            }
            _ => {
                let (segs, error) = path::parse(el.attr("d").unwrap_or(""));
                if error.is_some() {
                    self.notes.add("path");
                }
                segs
            }
        };
        if segs.is_empty() {
            return;
        }
        let name = layer_name(
            el,
            match el.name.as_str() {
                "line" => "Line",
                "polyline" => "Polyline",
                "polygon" => "Polygon",
                _ => "Path",
            },
        );
        self.vector(&name, &segs, st, ctm);
    }

    fn vector(&mut self, name: &str, local: &[Seg], st: &Style, ctm: Affine) {
        let segs = path::transform(local, &ctm);
        let Some([bx, by, bw, bh]) =
            path::bounds(&segs).filter(|b| b.iter().all(|v| v.is_finite()))
        else {
            return;
        };
        let (w, h) = (bw.max(1.), bh.max(1.));
        let mut map = Map::new();
        map.insert("type".into(), json!("vector"));
        map.insert("name".into(), json!(name));
        put_box(&mut map, bx, by, w, h);
        map.insert("path".into(), json!(path::write(&segs)));
        map.insert(
            "viewBox".into(),
            json!({"x": num(bx), "y": num(by), "width": num(w), "height": num(h)}),
        );
        if st.evenodd {
            map.insert("fillRule".into(), json!("evenodd"));
        }
        self.paints(st, ctm.mean_scale(), &mut map);
        self.add(Value::Object(map));
    }

    fn text(&mut self, el: &Element, st: &Style, ctm: Affine) {
        let content: String = el.text().split_whitespace().collect::<Vec<_>>().join(" ");
        if content.is_empty() {
            return;
        }
        let (vw, vh) = self.viewport;
        let first = |n: &str, r: f64| {
            el.attr(n)
                .and_then(|v| {
                    v.split(|c: char| c == ',' || c.is_whitespace())
                        .find(|s| !s.is_empty())
                })
                .and_then(|v| length(v, r, st.font_size))
        };
        let (x, y) = (first("x", vw).unwrap_or(0.), first("y", vh).unwrap_or(0.));
        if !ctm.is_axis_aligned() {
            self.notes.add("text-transform");
        }
        let scale = ctm.0[0]
            .abs()
            .max(1e-9)
            .min(ctm.mean_scale().max(1e-9) * 4.);
        let size = (st.font_size * scale).max(1.);
        let (px, py) = ctm.apply(x, y);
        // The anchor is the text's baseline start; a layer's position is its top left. Widths are estimated
        // (half an em per character) because shaping is the renderer's job.
        let width = content.chars().count() as f64 * size * 0.5;
        let left = match st.anchor {
            "middle" => px - width / 2.,
            "end" => px - width,
            _ => px,
        };
        let mut run = Map::new();
        run.insert("text".into(), json!(content));
        run.insert("font".into(), json!(st.font_family));
        if st.font_weight != 400 {
            run.insert("weight".into(), json!(st.font_weight));
        }
        if st.italic {
            run.insert("style".into(), json!("italic"));
        }
        run.insert("size".into(), json!(num(size)));
        let mut map = Map::new();
        map.insert("type".into(), json!("text"));
        map.insert(
            "name".into(),
            json!(layer_name(
                el,
                &content.chars().take(40).collect::<String>()
            )),
        );
        map.insert(
            "position".into(),
            json!({"type": "absolute", "x": num(left), "y": num(py - size * 0.8)}),
        );
        map.insert("width".into(), json!("hug"));
        map.insert("height".into(), json!("hug"));
        map.insert("content".into(), json!([Value::Object(run)]));
        map.insert("resize".into(), json!("auto-width"));
        if st.anchor != "start" {
            map.insert(
                "align".into(),
                json!(if st.anchor == "middle" {
                    "center"
                } else {
                    "right"
                }),
            );
        }
        self.paints(
            &Style {
                stroke: PaintValue::None,
                ..st.clone()
            },
            1.,
            &mut map,
        );
        self.add(Value::Object(map));
    }

    fn image(&mut self, el: &Element, st: &Style, ctm: Affine) {
        let href = el
            .attr("href")
            .or_else(|| el.attr("xlink:href"))
            .unwrap_or("");
        let (vw, vh) = self.viewport;
        let get = |n: &str, r: f64| el.attr(n).and_then(|v| length(v, r, st.font_size));
        let Some(data) = href.strip_prefix("data:") else {
            self.notes.add("link");
            return;
        };
        let Some((meta, body)) = data.split_once(',') else {
            return;
        };
        let bytes = if meta.ends_with(";base64") {
            vellum_core::base64::decode(&body.split_whitespace().collect::<String>())
        } else {
            None
        };
        let Some(bytes) = bytes else {
            self.notes.add("image");
            return;
        };
        let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(&[0xff, 0xd8]) {
            "image/jpeg"
        } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            "image/webp"
        } else {
            self.notes.add("image");
            return;
        };
        let (x, y) = (get("x", vw).unwrap_or(0.), get("y", vh).unwrap_or(0.));
        let (Some(w), Some(h)) = (get("width", vw), get("height", vh)) else {
            return;
        };
        if w <= 0. || h <= 0. {
            return;
        }
        let ((x0, y0), (x1, y1)) = (ctm.apply(x, y), ctm.apply(x + w, y + h));
        let fit = match el.attr("preserveAspectRatio").map(str::trim) {
            Some(p) if p.starts_with("none") => "stretch",
            Some(p) if p.ends_with("slice") => "fill",
            _ => "fit",
        };
        self.assets.push(ImportedAsset {
            mime: mime.to_owned(),
            bytes,
        });
        let mut map = Map::new();
        map.insert("type".into(), json!("image"));
        map.insert("name".into(), json!(layer_name(el, "Image")));
        put_box(
            &mut map,
            x0.min(x1),
            y0.min(y1),
            (x1 - x0).abs(),
            (y1 - y0).abs(),
        );
        map.insert("asset".into(), json!(asset_ref(self.assets.len() - 1)));
        map.insert("fit".into(), json!(fit));
        if st.opacity < 1. {
            map.insert("opacity".into(), json!(num(st.opacity)));
        }
        self.add(Value::Object(map));
    }
}

/// Presentation attributes that are also CSS properties.
const PROPERTIES: &[&str] = &[
    "fill",
    "fill-opacity",
    "fill-rule",
    "stroke",
    "stroke-opacity",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-dasharray",
    "opacity",
    "color",
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "text-anchor",
    "display",
    "visibility",
];

fn layer_name(el: &Element, fallback: &str) -> String {
    for key in ["inkscape:label", "data-name", "id"] {
        if let Some(v) = el.attr(key).map(str::trim).filter(|v| !v.is_empty()) {
            return v.chars().take(80).collect();
        }
    }
    fallback.to_owned()
}

fn num(v: f64) -> f64 {
    path::num(v).parse().unwrap_or(0.)
}

fn put_box(map: &mut Map<String, Value>, x: f64, y: f64, w: f64, h: f64) {
    map.insert(
        "position".into(),
        json!({"type": "absolute", "x": num(x), "y": num(y)}),
    );
    map.insert("width".into(), json!(num(w)));
    map.insert("height".into(), json!(num(h)));
}

fn unit(text: &str) -> Option<f64> {
    let t = text.trim();
    match t.strip_suffix('%') {
        Some(p) => p.parse::<f64>().ok().map(|v| (v / 100.).clamp(0., 1.)),
        None => t.parse::<f64>().ok().map(|v| v.clamp(0., 1.)),
    }
    .filter(|v| v.is_finite())
}

/// A length in user units. `reference` is what a percentage is of (and the font size for `em`).
fn length(text: &str, reference: f64, font_size: f64) -> Option<f64> {
    let t = text.trim();
    let split = t
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(t.len());
    let (number, unit) = t.split_at(split);
    let v: f64 = number.trim().parse().ok().filter(|v: &f64| v.is_finite())?;
    Some(match unit {
        "" | "px" => v,
        "%" => v / 100. * reference,
        "em" => v * font_size,
        "ex" => v * font_size / 2.,
        "pt" => v * 4. / 3.,
        "pc" => v * 16.,
        "mm" => v * 96. / 25.4,
        "cm" => v * 96. / 2.54,
        "in" => v * 96.,
        _ => return None,
    })
}
