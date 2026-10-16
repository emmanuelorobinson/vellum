use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

use serde::Deserialize;
use vellum_core::ports::{
    ContentError, FileSystem, StyleDensity, StyleLibrary, StyleMeta, StyleType, VisualStyle,
};

use super::{path, read};

const BUILTINS: &[&str] = &[
    include_str!("../../../../packages/styles/builtin/quiet.json"),
    include_str!("../../../../packages/styles/builtin/editorial.json"),
    include_str!("../../../../packages/styles/builtin/technical.json"),
    include_str!("../../../../packages/styles/builtin/rounded.json"),
    include_str!("../../../../packages/styles/builtin/earth.json"),
    include_str!("../../../../packages/styles/builtin/high-contrast.json"),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    name: String,
    description: String,
    palette: BTreeMap<String, String>,
    typography: Typography,
    radius: f64,
    density: Density,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Typography {
    body: String,
    heading: String,
    mono: String,
    base_size: f64,
    scale: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Density {
    gap: f64,
    padding: f64,
    control_height: f64,
}

fn parse(text: &str) -> Result<VisualStyle, ContentError> {
    let r: Recipe = serde_json::from_str(text)
        .map_err(|e| ContentError::Invalid(format!("style JSON: {e}")))?;
    let positive = [
        r.typography.base_size,
        r.typography.scale,
        r.density.gap,
        r.density.padding,
        r.density.control_height,
    ];
    let valid_color = |s: &String| {
        matches!(s.len(), 7 | 9)
            && s.starts_with('#')
            && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
    };
    if r.name.is_empty()
        || r.name.len() > 80
        || !r
            .name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || r.description.trim().is_empty()
        || r.description.len() > 1024
        || [
            &r.typography.body,
            &r.typography.heading,
            &r.typography.mono,
        ]
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 256)
        || !r.radius.is_finite()
        || r.radius < 0.0
        || positive.iter().any(|v| !v.is_finite() || *v <= 0.0)
        || ["background", "surface", "text", "muted", "accent", "border"]
            .iter()
            .any(|role| !r.palette.contains_key(*role))
        || !r.palette.values().all(valid_color)
    {
        return Err(ContentError::Invalid("style needs a name, description, six palette roles, fonts and finite positive sizes (radius may be zero)".into()));
    }
    Ok(VisualStyle {
        name: r.name,
        description: r.description,
        palette: r.palette,
        typography: StyleType {
            body: r.typography.body,
            heading: r.typography.heading,
            mono: r.typography.mono,
            base_size: r.typography.base_size,
            scale: r.typography.scale,
        },
        radius: r.radius,
        density: StyleDensity {
            gap: r.density.gap,
            padding: r.density.padding,
            control_height: r.density.control_height,
        },
    })
}

fn metas(styles: Vec<VisualStyle>) -> Result<Vec<StyleMeta>, ContentError> {
    let mut names = BTreeSet::new();
    styles
        .into_iter()
        .map(|s| {
            if !names.insert(s.name.clone()) {
                return Err(ContentError::Invalid(format!("duplicate style {}", s.name)));
            }
            Ok(StyleMeta {
                name: s.name,
                description: s.description,
            })
        })
        .collect()
}

/// Six optional canvas recipes; these are not editor chrome or bundled design-system tokens.
pub struct BuiltinStyles;
impl StyleLibrary for BuiltinStyles {
    fn list(&self) -> Result<Vec<StyleMeta>, ContentError> {
        metas(
            BUILTINS
                .iter()
                .map(|s| parse(s))
                .collect::<Result<_, _>>()?,
        )
    }
    fn read(&self, name: &str) -> Result<VisualStyle, ContentError> {
        for text in BUILTINS {
            let style = parse(text)?;
            if style.name == name {
                return Ok(style);
            }
        }
        Err(ContentError::NotFound(name.into()))
    }
}

/// Explicitly registered JSON recipe files under an authorized filesystem root, refreshed each call.
pub struct FolderStyles {
    fs: Arc<dyn FileSystem>,
    files: Vec<String>,
}
impl FolderStyles {
    pub fn new(fs: Arc<dyn FileSystem>, files: Vec<String>) -> Result<Self, ContentError> {
        let mut seen = BTreeSet::new();
        for file in &files {
            path(file)?;
            if !file.ends_with(".json") || !seen.insert(file) {
                return Err(ContentError::Invalid(
                    "style files must be distinct .json paths".into(),
                ));
            }
        }
        Ok(Self { fs, files })
    }
    fn styles(&self) -> Result<Vec<VisualStyle>, ContentError> {
        self.files
            .iter()
            .map(|p| {
                parse(&read(&*self.fs, p)?).map_err(|e| ContentError::Invalid(format!("{p}: {e}")))
            })
            .collect()
    }
}
impl StyleLibrary for FolderStyles {
    fn list(&self) -> Result<Vec<StyleMeta>, ContentError> {
        metas(self.styles()?)
    }
    fn read(&self, name: &str) -> Result<VisualStyle, ContentError> {
        let styles = self.styles()?;
        metas(styles.clone())?;
        styles
            .into_iter()
            .find(|s| s.name == name)
            .ok_or_else(|| ContentError::NotFound(name.into()))
    }
}

/// Later libraries own duplicate names, with the same error semantics as skill stacking.
pub struct StackedStyles(Vec<Rc<dyn StyleLibrary>>);
impl StackedStyles {
    pub fn new(sources: Vec<Rc<dyn StyleLibrary>>) -> Self {
        Self(sources)
    }
}
impl StyleLibrary for StackedStyles {
    fn list(&self) -> Result<Vec<StyleMeta>, ContentError> {
        let mut result: Vec<StyleMeta> = Vec::new();
        for source in &self.0 {
            for meta in source.list()? {
                result.retain(|s| s.name != meta.name);
                result.push(meta);
            }
        }
        Ok(result)
    }
    fn read(&self, name: &str) -> Result<VisualStyle, ContentError> {
        for source in self.0.iter().rev() {
            if source.list()?.iter().any(|m| m.name == name) {
                return source.read(name);
            }
        }
        Err(ContentError::NotFound(name.into()))
    }
}
