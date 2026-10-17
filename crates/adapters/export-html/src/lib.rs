//! Deterministic DOM and CSS from resolved layout. Tailwind uses arbitrary-property utilities with their
//! compiled rules included, so a scaffold works offline without JavaScript or a CDN. Complex appearance is
//! preserved with an explicit SVG fallback rather than approximated silently.
mod styles;
use std::collections::BTreeMap;
use styles::*;
use vellum_core::{export::*, ports::*, resolver::ResolvedNode};
use vellum_types::Node;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub struct Html {
    format: &'static str,
}
impl Html {
    pub fn new(format: &str) -> Option<Self> {
        Some(Self {
            format: match format {
                "html-css" => "html-css",
                "html-tailwind" => "html-tailwind",
                _ => return None,
            },
        })
    }
}
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
struct Writer {
    tailwind: bool,
    css: BTreeMap<String, String>,
    next: usize,
    assets: AssetMode,
    files: Vec<ExportFile>,
    warnings: Vec<String>,
}
impl Writer {
    fn attrs(&mut self, declarations: Vec<(String, String)>) -> String {
        if !self.tailwind {
            let id = format!("v{}", self.next);
            self.next += 1;
            self.css.insert(
                format!(".{id}"),
                declarations
                    .iter()
                    .map(|(k, v)| format!("{k}:{v};"))
                    .collect(),
            );
            return format!("class=\"{id}\"");
        }
        let mut classes = vec![];
        for (key, value) in declarations {
            let class = format!(
                "[{key}:{}]",
                value
                    .replace('\\', "\\\\")
                    .replace('_', "\\_")
                    .replace(' ', "_")
            );
            let selector = class
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c.to_string()
                    } else {
                        format!("\\{:x} ", c as u32)
                    }
                })
                .collect::<String>();
            self.css
                .insert(format!(".{selector}"), format!("{key}:{value};"));
            classes.push(class);
        }
        format!("class=\"{}\"", escape(&classes.join(" ")))
    }
    fn node(
        &mut self,
        r: &ResolvedNode,
        f: &ExportFrame,
        root: bool,
    ) -> Result<String, ExportError> {
        if !r.value.visible {
            return Ok(String::new());
        }
        let b = *f
            .layout
            .boxes
            .get(&r.address)
            .ok_or_else(|| ExportError::new("Missing HTML layout"))?;
        let n = &r.value.node;
        let mut style = node_style(n, b)?;
        if root {
            style.push(("left".into(), px(b.x - f.bounds.x)));
            style.push(("top".into(), px(b.y - f.bounds.y)));
        }
        let mut content = String::new();
        if let Node::Text(text) = n {
            let visual = f.text_visuals.get(&r.address);
            if visual.is_some() {
                content.push_str("<span style=\"opacity:0\">");
            }
            for run in &text.content {
                let attrs = self.attrs(run_style(run)?);
                content.push_str(&format!("<span {attrs}>{}</span>", escape(&run.text)));
            }
            if let Some(visual) = visual {
                let bytes = &visual.bytes;
                content.push_str("</span>");
                let src = match self.assets {
                    AssetMode::Inline => format!(
                        "data:image/png;base64,{}",
                        vellum_core::base64::encode(bytes)
                    ),
                    AssetMode::Files => {
                        let name = format!("assets/text-{}.png", self.files.len() + 1);
                        self.files.push(ExportFile {
                            name: name.clone(),
                            mime: "image/png".into(),
                            bytes: bytes.clone(),
                        });
                        name
                    }
                };
                content.push_str(&format!("<img aria-hidden=\"true\" alt=\"\" style=\"position:absolute;left:{}px;top:{}px\" width=\"{}\" height=\"{}\" src=\"{src}\">",visual.bounds.x,visual.bounds.y,visual.bounds.width.ceil(),visual.bounds.height.ceil()));
                self.warnings.push(format!("{} uses a rendered text layer for consistent rendering; the original text remains in HTML",r.address));
            }
        }
        for child in &r.children {
            content.push_str(&self.node(child, f, false)?);
        }
        let attrs = self.attrs(style);
        Ok(format!(
            "<div data-node=\"{}\" {attrs}>{content}</div>",
            escape(&r.address.to_string())
        ))
    }
}
impl CodeExporter for Html {
    fn id(&self) -> &'static str {
        self.format
    }
    fn export(
        &self,
        scene: &ExportScene,
        options: &CodeOptions,
        renderer: &dyn ExportRenderer,
    ) -> Result<ExportBundle, ExportError> {
        scene.check(1.)?;
        let mut writer = Writer {
            tailwind: self.format == "html-tailwind",
            css: BTreeMap::new(),
            next: 0,
            assets: options.assets,
            files: vec![],
            warnings: vec![],
        };
        let mut files = vec![];
        let mut warnings = vec![];
        let mut body = String::new();
        let mut font_css = String::new();
        let mut seen_fonts = std::collections::BTreeSet::new();
        for (i, frame) in scene.frames.iter().enumerate() {
            let f = frame.at_scale(1.);
            let fallback = f.fallback
                || f.roots.iter().any(|r| !supported(r))
                || f.faces
                    .iter()
                    .any(|f| f.data.index != 0 || f.data.bytes.starts_with(b"ttcf"));
            if !fallback {
                for face in f
                    .faces
                    .iter()
                    .filter(|face| face.embed || f.text_visuals.is_empty())
                {
                    if !seen_fonts.insert((face.family.clone(), face.weight, face.style.as_str())) {
                        continue;
                    }
                    let otf = face.data.bytes.starts_with(b"OTTO");
                    let (mime, ext) = if otf {
                        ("font/otf", "otf")
                    } else {
                        ("font/ttf", "ttf")
                    };
                    let src = match options.assets {
                        AssetMode::Inline => format!(
                            "data:{mime};base64,{}",
                            vellum_core::base64::encode(&face.data.bytes)
                        ),
                        AssetMode::Files => {
                            let name = format!("assets/font-{}.{}", seen_fonts.len(), ext);
                            files.push(ExportFile {
                                name: name.clone(),
                                mime: mime.into(),
                                bytes: face.data.bytes.to_vec(),
                            });
                            name
                        }
                    };
                    font_css.push_str(&format!("@font-face{{font-family:{};font-weight:{};font-style:{};src:url(\"{src}\")}}\n",css_string(&face.family),face.weight,face.style.as_str()));
                }
            }
            let attrs = writer.attrs(vec![
                ("position".into(), "relative".into()),
                ("width".into(), px(f.bounds.width)),
                ("height".into(), px(f.bounds.height)),
                ("overflow".into(), "hidden".into()),
                ("isolation".into(), "isolate".into()),
            ]);
            body.push_str(&format!(
                "<section aria-label=\"{}\" {attrs}>",
                escape(&f.name)
            ));
            if fallback {
                let raster = f
                    .output
                    .list
                    .resources
                    .iter()
                    .any(|r| matches!(r, vellum_types::display::Resource::Glyphs(_)));
                let (format, mime) = if raster {
                    ("png", "image/png")
                } else {
                    ("svg", "image/svg+xml")
                };
                let bytes = renderer.render(std::slice::from_ref(&f), format)?;
                let src = match options.assets {
                    AssetMode::Inline => {
                        format!("data:{mime};base64,{}", vellum_core::base64::encode(&bytes))
                    }
                    AssetMode::Files => {
                        let name = format!("assets/frame-{}.{format}", i + 1);
                        files.push(ExportFile {
                            name: name.clone(),
                            mime: mime.into(),
                            bytes,
                        });
                        name
                    }
                };
                body.push_str(&format!(
                    "<img alt=\"{}\" width=\"{}\" height=\"{}\" src=\"{src}\">",
                    escape(&f.name),
                    f.size().0,
                    f.size().1
                ));
                warnings.push(format!(
                    "{} uses a {format} asset to preserve appearance",
                    f.id
                ));
            } else {
                for root in &f.roots {
                    body.push_str(&writer.node(root, &f, true)?);
                }
            }
            body.push_str("</section>\n");
        }
        files.append(&mut writer.files);
        warnings.append(&mut writer.warnings);
        let css = writer
            .css
            .iter()
            .map(|(k, v)| format!("{k}{{{v}}}\n"))
            .collect::<String>();
        // All CSS string input is escaped at its point of construction. No document text becomes CSS syntax.
        let style = format!("<style>body{{margin:0}}\n{font_css}{css}</style>\n");
        let text = if options.scaffold {
            format!(
                "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>Export</title>{style}</head><body>{body}</body></html>\n"
            )
        } else {
            format!("{style}{body}")
        };
        files.insert(
            0,
            ExportFile {
                name: "index.html".into(),
                mime: "text/html".into(),
                bytes: text.into_bytes(),
            },
        );
        if files.iter().map(|f| f.bytes.len()).sum::<usize>() > 32 * 1024 * 1024 {
            return Err(ExportError::new("Export exceeds the 32 MiB bundle budget"));
        }
        Ok(ExportBundle { files, warnings })
    }
}
