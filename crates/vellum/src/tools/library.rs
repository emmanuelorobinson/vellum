//! What the document defines for reuse: `get_variables`, `list_components`, `list_fonts` (ARCHITECTURE §7.3).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::resolver::variables::{ResolvedValue, Theme, resolve_variable};
use vellum_types::{Node, PropDef, TextRun};

use super::support::{Index, parse_input, schema_of};
use super::{Tool, ToolCtx, ToolError};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(GetVariables));
    tools.push(Box::new(ListComponents));
    tools.push(Box::new(ListFonts));
}

struct GetVariables;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct VariablesInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// Theme axis values to resolve under, such as {"Mode": "dark"}. Axes left out take their first value.
    theme: Option<BTreeMap<String, String>>,
    /// Only variables whose name contains this, ignoring case.
    name: Option<String>,
}

fn resolved(value: &ResolvedValue) -> Value {
    match value {
        ResolvedValue::Color(color) => json!(color),
        ResolvedValue::Number(n) => json!(n),
        ResolvedValue::String(s) => json!(s),
        ResolvedValue::Boolean(b) => json!(b),
    }
}

impl Tool for GetVariables {
    fn name(&self) -> &'static str {
        "get_variables"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "List the document's variables (colours, numbers, strings, booleans) and its theme axes. Each shows its \
         id, name and type, the value it has under the theme, and every value it can take. Bind a property to \
         a variable with {\"$var\": id}; prefer a variable over a literal that matches one."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<VariablesInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let VariablesInput {
            document,
            theme,
            name,
        } = parse_input(input)?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let theme = Theme::defaults(&doc)
            .with(&doc, &theme.unwrap_or_default())
            .map_err(|e| ToolError::invalid("theme", e.to_string()))?;
        let name = name.map(|n| n.to_lowercase());
        let variables: Vec<Value> = doc
            .variables
            .iter()
            .filter(|(_, v)| {
                name.as_deref()
                    .is_none_or(|n| v.name().to_lowercase().contains(n))
            })
            .map(|(id, variable)| {
                let mut out = json!({
                    "id": id.as_str(),
                    "name": variable.name(),
                    "type": variable.variable_type().as_str(),
                    "values": serde_json::to_value(variable).ok().and_then(|v| v.get("values").cloned()),
                });
                match resolve_variable(&doc, id, &theme) {
                    Ok(value) => out["value"] = resolved(&value),
                    Err(error) => out["error"] = json!(error.to_string()),
                }
                out
            })
            .collect();
        let chosen: BTreeMap<_, _> = theme.iter().collect();
        Ok(json!({
            "document": path,
            "themes": doc.themes,
            "theme": chosen,
            "variables": variables,
        }))
    }
}

struct ListComponents;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ComponentsInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// Only components whose name contains this, ignoring case.
    name: Option<String>,
}

impl Tool for ListComponents {
    fn name(&self) -> &'static str {
        "list_components"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "List the document's components: id, name, root node, the props an instance can set, the variants, the \
         slots that accept other components, and how many instances use each. Use a component instead of \
         drawing the same thing again."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ComponentsInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ComponentsInput { document, name } = parse_input(input)?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let index = Index::build(&doc);
        let name = name.map(|n| n.to_lowercase());
        let mut uses: BTreeMap<&str, usize> = BTreeMap::new();
        for entry in &index.entries {
            if let Node::Instance(instance) = entry.node
                && instance.library.is_none()
            {
                *uses.entry(instance.component_id.as_str()).or_default() += 1;
            }
        }
        let components: Vec<Value> = doc
            .components
            .iter()
            .filter(|(_, c)| {
                name.as_deref()
                    .is_none_or(|n| c.name.to_lowercase().contains(n))
            })
            .map(|(id, component)| {
                let props: BTreeMap<_, _> = component
                    .props
                    .iter()
                    .map(|(prop, def)| {
                        let (kind, default) = match def {
                            PropDef::Boolean(p) => ("boolean", json!(p.default)),
                            PropDef::Text(p) => ("text", json!(p.default)),
                            PropDef::Number(p) => ("number", json!(p.default)),
                        };
                        (prop.clone(), json!({"type": kind, "default": default}))
                    })
                    .collect();
                let mut axes: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
                for variant in &component.variants {
                    for (axis, value) in &variant.properties {
                        axes.entry(axis).or_default().insert(value);
                    }
                }
                // Slots are frames inside the component that accept other components.
                let slots: Vec<&str> = index
                    .scope(id.as_str())
                    .into_iter()
                    .flatten()
                    .filter(|&at| index.entries[at].node.metadata().contains_key("slot"))
                    .map(|at| index.entries[at].node.id().as_str())
                    .collect();
                json!({
                    "id": id.as_str(),
                    "name": component.name,
                    "root": component.root.id().as_str(),
                    "props": props,
                    "variants": axes,
                    "slots": slots,
                    "instances": uses.get(id.as_str()).copied().unwrap_or(0),
                })
            })
            .collect();
        Ok(json!({"document": path, "components": components}))
    }
}

struct ListFonts;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FontsInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
}

impl Tool for ListFonts {
    fn name(&self) -> &'static str {
        "list_fonts"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "List the fonts text can use: the families the engine can draw, the fonts stored in this document, and \
         the families its text already uses. A text run names a family; one that is not listed is drawn with a \
         fallback and looks different from what you asked for."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<FontsInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let FontsInput { document } = parse_input(input)?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let custom: Vec<Value> = doc
            .fonts
            .iter()
            .map(|(asset, face)| {
                json!({
                    "asset": asset.as_str(),
                    "family": face.family,
                    "weight": face.weight,
                    "style": face.style.as_str(),
                })
            })
            .collect();
        let index = Index::build(&doc);
        let used: BTreeSet<&str> = index
            .entries
            .iter()
            .filter_map(|entry| match entry.node {
                Node::Text(text) => {
                    Some(text.content.iter().map(|run: &TextRun| run.font.as_str()))
                }
                _ => None,
            })
            .flatten()
            .collect();
        let available = ctx.docs().font_families();
        Ok(json!({"document": path, "available": available, "custom": custom, "used": used}))
    }
}
