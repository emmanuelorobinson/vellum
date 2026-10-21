use crate::{
    MAX_DEPTH, MAX_NODES, PenImporter, error,
    source::{self, Theme},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use vellum_core::ports::{ImportError, Imported};
pub(super) struct Convert<'a> {
    pub root: &'a Value,
    pub importer: &'a PenImporter,
    pub notes: BTreeSet<String>,
    pub out: Imported,
    pub assets: BTreeMap<String, usize>,
    pub requests: BTreeSet<String>,
    index: BTreeMap<String, &'a Value>,
    count: usize,
    expanded_bytes: usize,
}
pub fn run(root: &Value, importer: &PenImporter) -> Result<Imported, ImportError> {
    run_with_paths(root, importer).map(|(imported, _)| imported)
}
pub fn run_with_paths(
    root: &Value,
    importer: &PenImporter,
) -> Result<(Imported, Vec<String>), ImportError> {
    let mut c = Convert {
        root,
        importer,
        notes: BTreeSet::new(),
        out: Imported::default(),
        assets: BTreeMap::new(),
        requests: BTreeSet::new(),
        index: source::index(root)?,
        count: 0,
        expanded_bytes: 0,
    };
    if root.get("variables").is_some() || root.get("themes").is_some() {
        c.note("Variables and themes were resolved to values; their bindings were not retained.")
    }
    if root.get("imports").is_some() {
        c.note("External design libraries were not loaded.")
    }
    let theme = source::defaults(root);
    for v in root["children"].as_array().expect("validated") {
        if let Some(n) = c.node(v, &theme, false, 0, &[], &BTreeMap::new(), "")? {
            c.out.nodes.push(n)
        }
    }
    c.out.notes = c.notes.into_iter().collect();
    Ok((c.out, c.requests.into_iter().collect()))
}
impl Convert<'_> {
    pub fn note(&mut self, s: &str) {
        self.notes.insert(s.to_string());
    }
    pub fn prop(&self, n: &Value, k: &str, d: Value, t: &Theme) -> Result<Value, ImportError> {
        source::prop(self.root, n, k, d, t)
    }
    pub fn number(&self, n: &Value, k: &str, d: f64, t: &Theme) -> Result<f64, ImportError> {
        source::number(&self.prop(n, k, json!(d), t)?)
    }
    pub fn boolean(&self, n: &Value, k: &str, d: bool, t: &Theme) -> Result<bool, ImportError> {
        self.prop(n, k, json!(d), t)?
            .as_bool()
            .ok_or_else(|| error("An imported boolean is invalid."))
    }
    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        original: &Value,
        parent_theme: &Theme,
        in_layout: bool,
        depth: usize,
        refs: &[String],
        overrides: &BTreeMap<String, Value>,
        prefix: &str,
    ) -> Result<Option<Value>, ImportError> {
        self.count += 1;
        if self.count > MAX_NODES || depth > MAX_DEPTH {
            return Err(error("Expanded layers exceed the import limit."));
        }
        let mut v = std::borrow::Cow::Borrowed(original);
        let id = v["id"].as_str().unwrap_or("").to_string();
        let key = if prefix == "@" {
            String::new()
        } else if prefix.is_empty() {
            id.clone()
        } else {
            format!("{prefix}/{id}")
        };
        if let Some(patch) = overrides.get(&key) {
            if patch.get("type").is_some() {
                v = std::borrow::Cow::Borrowed(patch)
            } else {
                self.charge(original)?;
                self.charge(patch)?;
                merge(v.to_mut(), patch)?
            }
        }
        let ty = v["type"]
            .as_str()
            .ok_or_else(|| error("An imported layer has no type."))?
            .to_string();
        if ty == "ref" {
            let target = v["ref"]
                .as_str()
                .ok_or_else(|| error("A component reference is invalid."))?
                .to_string();
            if refs.contains(&target) || refs.len() > 32 {
                return Err(error(
                    "Component references are cyclic or too deeply nested.",
                ));
            }
            let Some(base) = self.index.get(&target).copied() else {
                self.note("A missing or external component instance was omitted.");
                return Ok(None);
            };
            self.charge(base)?;
            let mut expanded = base.clone();
            let object = expanded
                .as_object_mut()
                .ok_or_else(|| error("A component must be an object."))?;
            for (k, value) in v
                .as_object()
                .ok_or_else(|| error("An instance must be an object."))?
            {
                if !matches!(k.as_str(), "type" | "ref" | "descendants" | "reusable") {
                    self.charge(value)?;
                    object.insert(k.clone(), value.clone());
                }
            }
            // Definition coordinates locate its palette copy, not an instance on the canvas.
            for axis in ["x", "y"] {
                object.insert(axis.into(), v.get(axis).cloned().unwrap_or(json!(0)));
            }
            let mut local = BTreeMap::new();
            if let Some(d) = v.get("descendants") {
                for (k, v) in d
                    .as_object()
                    .ok_or_else(|| error("Instance overrides must be an object."))?
                {
                    self.charge(v)?;
                    local.insert(k.clone(), v.clone());
                }
            }
            // An enclosing instance's overrides win over this instance's defaults.
            for (k, v) in overrides {
                if key.is_empty() {
                    self.charge(v)?;
                    local.insert(k.clone(), v.clone());
                } else if let Some(k) = k.strip_prefix(&format!("{key}/")) {
                    self.charge(v)?;
                    local.insert(k.to_string(), v.clone());
                }
            }
            let mut next = refs.to_vec();
            next.push(target);
            self.note("Component instances were expanded into editable layers.");
            return self.node(
                &expanded,
                parent_theme,
                in_layout,
                depth + 1,
                &next,
                &local,
                "@",
            );
        }
        if matches!(ty.as_str(), "script" | "browser") {
            self.note("Executable script and live browser layers were omitted; no referenced content was loaded.");
            return Ok(None);
        }
        let kind = match ty.as_str() {
            "frame" => "frame",
            "group" => "group",
            "rectangle" => "rect",
            "ellipse" => "ellipse",
            "polygon" => "polygon",
            "path" => "vector",
            "text" | "note" | "context" | "prompt" => "text",
            "icon" => "icon",
            _ => {
                self.note("Unknown layer types were omitted.");
                return Ok(None);
            }
        };
        let theme = source::theme(parent_theme, &v)?;
        let width_default = if matches!(
            ty.as_str(),
            "frame" | "group" | "text" | "note" | "context" | "prompt"
        ) {
            json!("fit_content")
        } else {
            json!(0)
        };
        let mut n = json!({"type":kind,"name":v["name"].as_str().unwrap_or(&ty),"width":source::length(&self.prop(&v,"width",width_default.clone(),&theme)?)?,"height":source::length(&self.prop(&v,"height",width_default,&theme)?)?,"metadata":{"importedFrom":{"format":"pen","nodeId":id}}});
        if v.get("metadata").is_some() {
            self.note("Source metadata was omitted; imported layer ids are retained as provenance.")
        }
        let absolute = !in_layout || v["layoutPosition"] == "absolute";
        if absolute {
            n["position"] = json!({"type":"absolute","x":self.number(&v,"x",0.,&theme)?,"y":self.number(&v,"y",0.,&theme)?})
        }
        n["visible"] = json!(self.boolean(&v, "enabled", true, &theme)?);
        n["opacity"] = json!(self.number(&v, "opacity", 1., &theme)?);
        n["flipX"] = json!(self.boolean(&v, "flipX", false, &theme)?);
        n["flipY"] = json!(self.boolean(&v, "flipY", false, &theme)?);
        let angle = self.number(&v, "rotation", 0., &theme)?;
        n["rotation"] = json!(-angle);
        if angle != 0. {
            self.note("Rotation uses Vellum's center pivot; rotated layers may need repositioning.")
        }
        n["fills"] = json!(self.paints(v.get("fill"), &theme)?);
        n["effects"] = json!(self.effects(v.get("effect"), &theme)?);
        if v.get("stroke").is_some() {
            let paints = self.paints(v.get("stroke"), &theme)?;
            if !paints.is_empty() {
                let raw = self.prop(&v, "strokeWidth", json!(1), &theme)?;
                let width = if let Some(m) = raw.as_object() {
                    json!([
                        source::number(m.get("top").unwrap_or(&json!(0)))?,
                        source::number(m.get("right").unwrap_or(&json!(0)))?,
                        source::number(m.get("bottom").unwrap_or(&json!(0)))?,
                        source::number(m.get("left").unwrap_or(&json!(0)))?
                    ])
                } else {
                    json!(source::number(&raw)?)
                };
                n["stroke"] = json!({"paints":paints,"width":width,"align":match v["strokeAlignment"].as_str(){Some("inner")=>"inside",Some("outer")=>"outside",_=>"center"},"cap":v["strokeLinecap"].as_str().unwrap_or("butt"),"join":v["strokeLinejoin"].as_str().unwrap_or("miter")});
            }
        }
        if matches!(kind, "rect" | "frame")
            && let Some(r) = v.get("cornerRadius")
        {
            n["cornerRadius"] = source::resolve(self.root, r, &theme, 0)?;
        }
        match kind {
            "frame" | "group" => {
                let layout = if kind == "frame" {
                    v["layout"].as_str().unwrap_or("horizontal")
                } else {
                    "none"
                };
                if kind == "frame" {
                    n["layout"] = json!(match layout {
                        "none" => "none",
                        "horizontal" => "row",
                        "vertical" => "column",
                        _ => return Err(error("An imported layout mode is invalid.")),
                    });
                    n["gap"] = json!(self.number(&v, "gap", 0., &theme)?);
                    n["padding"] = self.prop(&v, "padding", json!(0), &theme)?;
                    n["justify"] = json!(
                        v["justifyContent"]
                            .as_str()
                            .unwrap_or("start")
                            .replace('_', "-")
                    );
                    n["align"] = json!(v["alignItems"].as_str().unwrap_or("start"));
                    n["clip"] = json!(self.boolean(&v, "clip", false, &theme)?);
                    n["layoutIncludesStroke"] =
                        json!(v["layoutIncludeStroke"].as_bool().unwrap_or(false));
                    if v.get("slot").is_some() {
                        self.note("Slots were imported as ordinary editable frames.")
                    }
                }
                let mut children = vec![];
                if let Some(kids) = v.get("children") {
                    for child in kids
                        .as_array()
                        .ok_or_else(|| error("Children must be a list."))?
                    {
                        if let Some(child) = self.node(
                            child,
                            &theme,
                            layout != "none",
                            depth + 1,
                            refs,
                            overrides,
                            &key,
                        )? {
                            children.push(child)
                        }
                    }
                }
                n["children"] = json!(children);
            }
            "text" => self.text(&v, &theme, &mut n)?,
            "ellipse" => {
                n["innerRadius"] = json!(self.number(&v, "innerRadius", 0., &theme)?);
                n["startAngle"] = json!(-self.number(&v, "startAngle", 0., &theme)?);
                n["sweepAngle"] = json!(-self.number(&v, "sweepAngle", 360., &theme)?);
            }
            "polygon" => {
                n["sides"] = json!(self.number(&v, "polygonCount", 3., &theme)? as u32);
                n["cornerRadius"] = json!(self.number(&v, "cornerRadius", 0., &theme)?);
            }
            "vector" => {
                let data = v["geometry"]
                    .as_str()
                    .ok_or_else(|| error("A path has no geometry."))?;
                if data.len() > 1024 * 1024 {
                    return Err(error("An imported path exceeds 1 MB."));
                }
                let bounds = if let Some(b) = v.get("viewBox") {
                    let a = b
                        .as_array()
                        .filter(|a| a.len() == 4)
                        .ok_or_else(|| error("A path view box is invalid."))?;
                    [
                        source::number(&a[0])?,
                        source::number(&a[1])?,
                        source::number(&a[2])?,
                        source::number(&a[3])?,
                    ]
                } else if let Some(b) = self.importer.bounds.and_then(|f| f(data)) {
                    b
                } else {
                    self.note("A path without usable bounds was omitted.");
                    return Ok(None);
                };
                if bounds[2] < 0. || bounds[3] < 0. {
                    return Err(error("A path view box has a negative size."));
                }
                if v.get("width").is_none() {
                    n["width"] = json!(bounds[2].max(1e-6));
                }
                if v.get("height").is_none() {
                    n["height"] = json!(bounds[3].max(1e-6));
                }
                n["path"] = json!(data);
                n["viewBox"] = json!({"x":bounds[0],"y":bounds[1],"width":bounds[2].max(1e-6),"height":bounds[3].max(1e-6)});
                n["fillRule"] = json!(v["fillRule"].as_str().unwrap_or("nonzero"));
            }
            "icon" => {
                let lib = self.prop(&v, "library", json!("lucide"), &theme)?;
                let lib = lib
                    .as_str()
                    .ok_or_else(|| error("An icon library is invalid."))?;
                n["library"] = json!(match lib {
                    "Material Symbols Outlined" => "material-symbols-outlined",
                    "Material Symbols Rounded" => "material-symbols-rounded",
                    "Material Symbols Sharp" => "material-symbols-sharp",
                    other => other,
                });
                n["icon"] = self.prop(&v, "icon", json!(""), &theme)?;
                n["weight"] = json!(self.number(&v, "weight", 400., &theme)? as u16);
                n["fill"] = json!(false);
            }
            _ => {}
        }
        // Count each layer's own properties once: repeated component text/path data must not amplify
        // a small source into an unbounded output tree. Children were already charged as they were built.
        for (key, value) in n.as_object().expect("node object") {
            if key != "children" {
                self.charge(value)?;
            }
        }
        Ok(Some(n))
    }
    fn charge(&mut self, value: &Value) -> Result<(), ImportError> {
        // Source-tree copies used by overrides/references are charged as well as final own properties.
        // This bounds transient expansion memory for a graph of large nested reusable definitions.
        let size = serde_json::to_vec(value)
            .map_err(|_| error("An imported property is invalid."))?
            .len();
        self.expanded_bytes = self
            .expanded_bytes
            .checked_add(size)
            .ok_or_else(|| error("Expanded layers exceed the byte budget."))?;
        if self.expanded_bytes > 64 * 1024 * 1024 {
            return Err(error("Expanded layers exceed the byte budget."));
        }
        Ok(())
    }
    fn text(&mut self, v: &Value, t: &Theme, n: &mut Value) -> Result<(), ImportError> {
        if v["type"] != "text" {
            self.note("Notes, contexts and prompts were imported as inert text; no agent request was started.")
        }
        let text = self.prop(v, "content", json!(""), t)?;
        let text = text
            .as_str()
            .ok_or_else(|| error("Text content must resolve to a string."))?;
        let family = self.prop(v, "fontFamily", json!("Inter"), t)?;
        let size = self.number(v, "fontSize", 14., t)?;
        let weight = self.prop(v, "fontWeight", json!("normal"), t)?;
        let weight = match weight.as_str().unwrap_or("") {
            "normal" | "regular" => 400,
            "bold" => 700,
            s => s
                .parse::<u16>()
                .map_err(|_| error("A text weight is invalid."))?,
        };
        let style = self.prop(v, "fontStyle", json!("normal"), t)?;
        let mut run = json!({"text":text,"font":family,"size":size,"weight":weight,"style":style,"letterSpacing":self.number(v,"letterSpacing",0.,t)?,"underline":self.boolean(v,"underline",false,t)?,"strikethrough":self.boolean(v,"strikethrough",false,t)?});
        if v.get("lineHeight").is_some() {
            run["lineHeight"] = json!(self.number(v, "lineHeight", 1., t)? * size)
        }
        if v.get("href").is_some() {
            self.note("Text hyperlinks were omitted.")
        }
        n["content"] = json!([run]);
        n["align"] = json!(v["textAlign"].as_str().unwrap_or("left"));
        n["verticalAlign"] = json!(v["textAlignVertical"].as_str().unwrap_or("top"));
        match v["textGrowth"].as_str().unwrap_or("auto") {
            "auto" => {
                n["width"] = json!("hug");
                n["height"] = json!("hug");
                n["resize"] = json!("auto-width")
            }
            "fixed-width" => {
                n["height"] = json!("hug");
                n["resize"] = json!("auto-height")
            }
            "fixed-width-height" => n["resize"] = json!("fixed"),
            _ => return Err(error("A text sizing mode is invalid.")),
        }
        self.note("Fonts were retained by family; font files were not imported.");
        Ok(())
    }
}
fn merge(v: &mut Value, patch: &Value) -> Result<(), ImportError> {
    let object = v
        .as_object_mut()
        .ok_or_else(|| error("A layer must be an object."))?;
    for (k, value) in patch
        .as_object()
        .ok_or_else(|| error("An override must be an object."))?
    {
        if k != "id" {
            object.insert(k.clone(), value.clone());
        }
    }
    Ok(())
}
