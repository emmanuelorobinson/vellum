use crate::{
    convert::Convert,
    error, relative_image,
    source::{self, Theme},
};
use serde_json::{Value, json};
use vellum_core::{
    base64,
    ports::{ImportError, ImportedAsset, asset_ref},
};
fn items(v: Option<&Value>) -> Vec<&Value> {
    match v {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(v) => vec![v],
        None => vec![],
    }
}
fn color(v: &Value, opacity: f64) -> Result<String, ImportError> {
    let s = v
        .as_str()
        .and_then(|s| s.strip_prefix('#'))
        .ok_or_else(|| error("An imported color is invalid."))?;
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) || !matches!(s.len(), 3 | 6 | 8) {
        return Err(error("An imported color is invalid."));
    }
    let s = if s.len() == 3 {
        s.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        s.to_string()
    };
    let alpha = if s.len() == 8 {
        u8::from_str_radix(&s[6..8], 16).map_err(|_| error("An imported color is invalid."))?
    } else {
        255
    };
    let alpha = (f64::from(alpha) * opacity.clamp(0., 1.)).round() as u8;
    Ok(if alpha == 255 {
        format!("#{}", s[..6].to_ascii_uppercase())
    } else {
        format!("#{}{:02X}", s[..6].to_ascii_uppercase(), alpha)
    })
}
impl Convert<'_> {
    pub fn paints(&mut self, value: Option<&Value>, t: &Theme) -> Result<Vec<Value>, ImportError> {
        let mut paints = vec![];
        for raw in items(value) {
            let p = source::resolve(self.root, raw, t, 0)?;
            if p.is_string() {
                paints.push(json!({"type":"solid","color":color(&p,1.)?}));
                continue;
            }
            let visible = self.boolean(&p, "enabled", true, t)?;
            if p["blendMode"].as_str().is_some_and(|s| s != "normal") {
                self.note("Per-paint blend modes were not converted.")
            }
            let opacity = self.number(&p, "opacity", 1., t)?;
            let mut paint = match p["type"].as_str().unwrap_or("") {
                "color" => json!({"type":"solid","color":color(&p["color"],1.)?}),
                "image" => {
                    let Some(url) = p["url"].as_str() else {
                        self.note("An image without an address was omitted.");
                        continue;
                    };
                    if let Some(path) = relative_image(url) {
                        self.requests.insert(path);
                        if self.requests.len() > 128 {
                            return Err(error("Too many companion images; import at most 128."));
                        }
                    }
                    let key = relative_image(url).unwrap_or_else(|| url.to_string());
                    let index = if let Some(i) = self.assets.get(&key) {
                        *i
                    } else {
                        let asset = if let Some((head, data)) =
                            url.strip_prefix("data:").and_then(|s| s.split_once(','))
                        {
                            let Some(mime) = head.strip_suffix(";base64").filter(|s| {
                                matches!(*s, "image/png" | "image/jpeg" | "image/webp")
                            }) else {
                                self.note("An unsupported inline image was omitted.");
                                continue;
                            };
                            let bytes = base64::decode(data)
                                .filter(|b| !b.is_empty() && b.len() <= 20 * 1024 * 1024)
                                .ok_or_else(|| {
                                    error("An inline image is malformed or too large.")
                                })?;
                            ImportedAsset {
                                mime: mime.into(),
                                bytes,
                            }
                        } else if let Some(path) = relative_image(url) {
                            let Some(a) = self.importer.assets.get(&path) else {
                                self.note("Some companion images were unavailable or not authorized; their fills were omitted.");
                                continue;
                            };
                            a.clone()
                        } else {
                            self.note("Unsafe or remote image addresses were omitted; no network request was made.");
                            continue;
                        };
                        if self.out.assets.len() >= 128
                            || self.out.assets.iter().map(|a| a.bytes.len()).sum::<usize>()
                                + asset.bytes.len()
                                > 64 * 1024 * 1024
                        {
                            return Err(error("Imported images exceed the asset budget."));
                        }
                        let index = self.out.assets.len();
                        self.out.assets.push(asset);
                        self.assets.insert(key, index);
                        index
                    };
                    let fit = match p["mode"].as_str().unwrap_or("cover") {
                        "cover" => "fill",
                        "contain" => "fit",
                        "stretch" => "stretch",
                        _ => return Err(error("An image fit mode is invalid.")),
                    };
                    let mut image = json!({"type":"image","asset":asset_ref(index),"fit":fit,"opacity":opacity});
                    if let Some(transform) = p.get("transform") {
                        image["crop"] = transform.clone()
                    }
                    image
                }
                "gradient" => {
                    let kind = p["gradientType"].as_str().unwrap_or("linear");
                    if !matches!(kind, "linear" | "radial" | "angular") {
                        self.note("An unsupported gradient was omitted.");
                        continue;
                    }
                    let stops = p["colors"]
                        .as_array()
                        .ok_or_else(|| error("A gradient has no stops."))?;
                    if stops.is_empty() || stops.len() > 256 {
                        return Err(error("A gradient stop list is invalid."));
                    }
                    let mut values = vec![];
                    for stop in stops {
                        values.push(json!({"color":color(&stop["color"],opacity)?,"position":source::number(&stop["position"])?}))
                    }
                    let cx = p["center"]
                        .get("x")
                        .map(source::number)
                        .transpose()?
                        .unwrap_or(0.5);
                    let cy = p["center"]
                        .get("y")
                        .map(source::number)
                        .transpose()?
                        .unwrap_or(0.5);
                    let w = p["size"]
                        .get("width")
                        .map(source::number)
                        .transpose()?
                        .unwrap_or(1.);
                    let h = p["size"]
                        .get("height")
                        .map(source::number)
                        .transpose()?
                        .unwrap_or(1.);
                    let angle = self.number(&p, "rotation", 0., t)?.to_radians();
                    let (s, c) = angle.sin_cos();
                    let (a, b, c, d) = if kind == "linear" {
                        (-s * h, -c * h, c, -s)
                    } else {
                        (c * w, -s * w, s * h, c * h)
                    };
                    self.note("Gradient transforms were converted; check their direction and extent after import.");
                    json!({"type":"gradient","kind":kind,"stops":values,"transform":[a,b,c,d,cx-0.5*(a+c),cy-0.5*(b+d)]})
                }
                "mesh_gradient" => {
                    let cols = self.number(&p, "columns", 2., t)? as usize;
                    let rows = self.number(&p, "rows", 2., t)? as usize;
                    if !(2..=64).contains(&cols) || !(2..=64).contains(&rows) {
                        return Err(error("A mesh size is invalid."));
                    }
                    let colors = p["colors"]
                        .as_array()
                        .filter(|a| a.len() == cols * rows)
                        .ok_or_else(|| error("Mesh colors do not match its grid."))?;
                    let positions = p.get("points").and_then(Value::as_array);
                    if positions.is_some_and(|a| a.len() != cols * rows) {
                        return Err(error("Mesh points do not match its grid."));
                    }
                    let mut points = vec![];
                    for (i, co) in colors.iter().enumerate() {
                        let raw = positions.and_then(|a| a.get(i));
                        let position = raw.and_then(|v| {
                            v.as_array()
                                .or_else(|| v.get("position").and_then(Value::as_array))
                        });
                        let (x, y) = if let Some(a) = position {
                            if a.len() != 2 {
                                return Err(error("A mesh point is invalid."));
                            }
                            (source::number(&a[0])?, source::number(&a[1])?)
                        } else {
                            (
                                (i % cols) as f64 / (cols - 1) as f64,
                                (i / cols) as f64 / (rows - 1) as f64,
                            )
                        };
                        let mut point = json!({"x":x,"y":y,"color":color(co,opacity)?});
                        if let Some(raw) = raw.filter(|v| v.is_object()) {
                            let mut handles = serde_json::Map::new();
                            let mut any = false;
                            for (from, to) in [
                                ("leftHandle", "left"),
                                ("rightHandle", "right"),
                                ("topHandle", "up"),
                                ("bottomHandle", "down"),
                            ] {
                                let (x, y) = if let Some(a) = raw
                                    .get(from)
                                    .and_then(Value::as_array)
                                    .filter(|a| a.len() == 2)
                                {
                                    any = true;
                                    (source::number(&a[0])?, source::number(&a[1])?)
                                } else {
                                    (0., 0.)
                                };
                                handles.insert(to.into(), json!({"x":x,"y":y}));
                            }
                            if any {
                                point["handles"] = Value::Object(handles)
                            }
                        }
                        points.push(point)
                    }
                    json!({"type":"mesh","columns":cols,"rows":rows,"points":points})
                }
                "shader" => {
                    self.note("Shader fills were omitted; no shader source was read or executed.");
                    continue;
                }
                _ => {
                    self.note("An unsupported paint was omitted.");
                    continue;
                }
            };
            paint["visible"] = json!(visible);
            paints.push(paint)
        }
        Ok(paints)
    }
    pub fn effects(&mut self, value: Option<&Value>, t: &Theme) -> Result<Vec<Value>, ImportError> {
        let mut effects = vec![];
        for raw in items(value) {
            let p = source::resolve(self.root, raw, t, 0)?;
            let visible = self.boolean(&p, "enabled", true, t)?;
            let effect = match p["type"].as_str().unwrap_or("") {
                "blur" | "background_blur" => {
                    json!({"type":if p["type"]=="blur"{"layer-blur"}else{"background-blur"},"radius":self.number(&p,"radius",0.,t)?,"visible":visible})
                }
                "shadow" => {
                    json!({"type":if p["shadowType"]=="inner"{"inner-shadow"}else{"drop-shadow"},"x":p["offset"].get("x").map(source::number).transpose()?.unwrap_or(0.),"y":p["offset"].get("y").map(source::number).transpose()?.unwrap_or(0.),"blur":self.number(&p,"blur",0.,t)?,"color":color(p.get("color").unwrap_or(&json!("#000000")),1.)?,"visible":visible})
                }
                "shader" => {
                    self.note(
                        "Shader effects were omitted; no shader source was read or executed.",
                    );
                    continue;
                }
                _ => {
                    self.note("An unsupported effect was omitted.");
                    continue;
                }
            };
            if p.get("blendMode").is_some() {
                self.note("Effect blend modes were not converted.")
            }
            effects.push(effect)
        }
        Ok(effects)
    }
}
