//! Design to code and back: `get_spec`, `export_tokens`, `import_tokens` (ARCHITECTURE §7.3).
//!
//! None of them touches a file. A token file is text in and text out; reading it from the repository and writing
//! it back is the caller's (the editor's token dialog, the CLI, or the agent's own file tools), where the
//! `PermissionBroker` already stands.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::ops::{Op, Shell};
use vellum_core::ports::{SpecFormat, TokenFormat};
use vellum_core::spec::{LayoutSpecV1, StackProfiles};
use vellum_core::tokens::{ModeSelector, TokenOptions, TokenWarning, collect, plan};
use vellum_export_tokens::format_by_id;
use vellum_types::{Document, VariableId};

use super::batch::{Batch, Label, fresh, ids_in_use, node_id, sentence};
use super::support::{parse_input, schema_of};
use super::{ErrorKind, Tool, ToolCtx, ToolError, Verb};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>, stacks: StackProfiles) {
    tools.push(Box::new(GetSpec { stacks }));
    tools.push(Box::new(ExportTokens));
    tools.push(Box::new(ImportTokens));
}

// ---- get_spec ---------------------------------------------------------------------------------------------

struct GetSpec {
    stacks: StackProfiles,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SpecInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The frame (or any layer) to describe, by id.
    frame: String,
    /// The stack you will write the code in: `shadcn`, `react-native` or `swiftui`. Adds that stack's hints on how
    /// to write each part. Without it, the answer lists the stacks there are.
    stack: Option<String>,
}

impl Tool for GetSpec {
    fn name(&self) -> &'static str {
        "get_spec"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Describe a frame so you can write its code: its layers with layout, sizing, fills and text; every design \
         token it uses, by name, with its value under each theme; and the components its instances use, with \
         props and definition. Pass `stack` (shadcn, react-native or swiftui) to also get that stack's hints for \
         writing each part. The spec is the design's own words, not code. Use tokens by name instead of copying \
         their values, and use snapshot_layout when you need rendered sizes."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<SpecInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let SpecInput {
            document,
            frame,
            stack,
        } = parse_input(input)?;
        let profile = stack
            .as_deref()
            .map(|id| {
                self.stacks
                    .require(id)
                    .map_err(|m| ToolError::invalid("stack", sentence(&m)))
            })
            .transpose()?;
        let id = node_id(&frame, "frame")?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut spec = LayoutSpecV1
            .build(&doc, &id)
            .map_err(|e| ToolError::not_found(sentence(&e.message)))?;
        ctx.note_target(&doc, &frame);
        let map = spec.as_object_mut().expect("a spec is an object");
        map.insert("document".into(), json!(path));
        match profile {
            Some(profile) => map.insert("stack".into(), profile.to_json()),
            None => map.insert(
                "stacks".into(),
                json!(
                    self.stacks
                        .iter()
                        .map(|p| p.id.as_str())
                        .collect::<Vec<_>>()
                ),
            ),
        };
        Ok(spec)
    }
}

// ---- export_tokens and import_tokens ----------------------------------------------------------------------

/// The token file formats.
#[derive(Deserialize, JsonSchema, Clone, Copy)]
enum FormatName {
    /// CSS custom properties: `:root { --color-primary: #2563EB; }` and a block per theme value.
    #[serde(rename = "css-vars")]
    CssVars,
    /// A Tailwind v4 stylesheet: `@theme { … }` and a block per theme value.
    #[serde(rename = "tailwind-v4")]
    TailwindV4,
    /// W3C Design Tokens JSON.
    #[serde(rename = "dtcg-json")]
    DtcgJson,
}

impl FormatName {
    fn format(self) -> Box<dyn TokenFormat> {
        let id = match self {
            Self::CssVars => "css-vars",
            Self::TailwindV4 => "tailwind-v4",
            Self::DtcgJson => "dtcg-json",
        };
        format_by_id(id).expect("every format the tools name ships")
    }
}

/// Which selector a theme value is written under.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MappingRow {
    /// The theme axis, such as `Mode`.
    axis: String,
    /// Its value, such as `dark`.
    mode: String,
    /// The CSS selector: `.dark`, `[data-theme="dark"]`. The first value of an axis is its default and is always
    /// written under `:root`.
    selector: String,
}

/// The settings both tools share.
#[derive(Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
struct FormatSettings {
    /// Rows to change in the mapping of theme values to selectors. Only the rows you name change; a value the
    /// document lacks is added to its axis.
    mapping: Option<Vec<MappingRow>>,
    /// Token namespaces written under another name in the file, such as {"space": "spacing"}.
    namespaces: Option<BTreeMap<String, String>>,
    /// Name prefixes whose numbers are lengths and so are written with `px`. Replaces the defaults (space, spacing,
    /// radius, size, width, height, gap, border, stroke, blur, container, breakpoint, text, font/size).
    lengths: Option<Vec<String>>,
}

fn options(
    format: &dyn TokenFormat,
    doc: &Document,
    settings: FormatSettings,
) -> Result<TokenOptions, ToolError> {
    let mut options = format.default_options(&doc.themes);
    if let Some(rows) = settings.mapping {
        let rows: Vec<ModeSelector> = rows
            .into_iter()
            .map(|r| ModeSelector::new(r.axis, r.mode, r.selector))
            .collect();
        options.mapping = options
            .mapping
            .with(&rows)
            .map_err(|e| ToolError::invalid("mapping", sentence(&e.to_string())))?;
    }
    if let Some(namespaces) = settings.namespaces {
        options.namespaces.extend(namespaces);
    }
    if let Some(lengths) = settings.lengths {
        options.lengths = lengths;
    }
    options.known = doc
        .variables
        .values()
        .map(|v| (v.name().to_owned(), v.variable_type()))
        .collect();
    Ok(options)
}

fn warnings(list: &[TokenWarning]) -> Vec<Value> {
    list.iter()
        .map(|w| match &w.token {
            Some(token) => json!({"token": token, "message": sentence(&w.message)}),
            None => json!({"message": sentence(&w.message)}),
        })
        .collect()
}

fn mapping(options: &TokenOptions) -> Vec<Value> {
    options
        .mapping
        .rows()
        .iter()
        .map(|r| json!({"axis": r.axis, "mode": r.mode, "selector": r.selector}))
        .collect()
}

struct ExportTokens;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExportInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The file format.
    format: FormatName,
    #[serde(flatten)]
    settings: FormatSettings,
}

impl Tool for ExportTokens {
    fn name(&self) -> &'static str {
        "export_tokens"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Write the document's variables as a token file: CSS variables, a Tailwind v4 `@theme` block, or Design \
         Tokens (DTCG) JSON. Returns the file's text and a file name to suggest; write it into the project \
         yourself. Each theme value (such as dark) is written under the selector in `mapping`, which you can \
         change. `warnings` names every token or value that could not be written."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ExportInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ExportInput {
            document,
            format,
            settings,
        } = parse_input(input)?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let format = format.format();
        let options = options(&*format, &doc, settings)?;
        let (set, mut notes) = collect(&doc);
        let out = format
            .export(&set, &options)
            .map_err(|e| ToolError::new(ErrorKind::Internal, e.to_string()))?;
        notes.extend(out.warnings);
        Ok(json!({
            "document": path,
            "format": format.id(),
            "file": format.file_name(),
            "text": out.text,
            "tokens": set.tokens.len(),
            "mapping": mapping(&options),
            "warnings": warnings(&notes),
        }))
    }
}

struct ImportTokens;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImportInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// The file format.
    format: FormatName,
    /// The file's text. Read the file yourself and pass its contents.
    text: String,
    #[serde(flatten)]
    settings: FormatSettings,
    /// Show the diff and change nothing.
    #[serde(default)]
    dry_run: bool,
    /// Also remove the variables the file lacks. They are listed under `removed` either way; one that is still
    /// used cannot be removed, and then nothing is imported.
    #[serde(default)]
    remove_missing: bool,
}

impl Tool for ImportTokens {
    fn name(&self) -> &'static str {
        "import_tokens"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Read a token file (CSS variables, Tailwind v4 or DTCG JSON) and make the document's variables match it, \
         as one change. A token whose name the document has keeps its variable; new names become new variables; \
         theme values the document lacks are added to its themes. Returns the diff: added, changed, removed. \
         `dry_run` shows it without changing anything. Variables the file lacks are only listed unless \
         `remove_missing` is set. Pass the file's text, not its path."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ImportInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::UpdatedVariables
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ImportInput {
            document,
            format,
            text,
            settings,
            dry_run,
            remove_missing,
        } = parse_input(input)?;
        let (path, doc) = ctx.document(document.as_deref())?;
        let format = format.format();
        let options = options(&*format, &doc, settings)?;
        let read = format.import(&text, &options).map_err(|e| {
            let error = ToolError::invalid("text", sentence(&e.message));
            match e.line {
                Some(line) => error.at_line(line, None),
                None => error,
            }
        })?;

        let mut ids = ctx.docs().tool_ids();
        let mut taken = ids_in_use(&doc);
        let plan = plan(&doc, &read.set, remove_missing, &mut || {
            VariableId::new(fresh(&mut ids, &mut taken)).expect("generated ids are valid")
        })
        .map_err(|e| ToolError::invalid("text", sentence(&e.to_string())))?;

        let diff = json!({
            "added": plan.diff.added,
            "changed": plan.diff.changed.iter()
                .map(|c| json!({"name": c.name, "what": c.what}))
                .collect::<Vec<_>>(),
            "removed": plan.diff.removed,
            "unchanged": plan.diff.unchanged.len(),
        });
        let mut out = json!({
            "document": path,
            "format": format.id(),
            "applied": false,
            "diff": diff,
            "warnings": warnings(&read.warnings),
        });
        if let Some(themes) = &plan.themes {
            out["themes"] = json!(themes);
        }
        if dry_run || (plan.set.is_empty() && plan.remove.is_empty() && plan.themes.is_none()) {
            return Ok(out);
        }

        let mut batch = Batch::default();
        if let Some(themes) = plan.themes {
            let mut shell = Shell::of(&doc);
            shell.themes = themes;
            batch.push(
                Op::SetShell {
                    shell: Box::new(shell),
                },
                Label::whole("text"),
            );
        }
        batch.push(
            Op::SetVariables {
                set: plan.set,
                remove: plan.remove,
            },
            Label::whole("text"),
        );
        ctx.commit(&path, batch)?;
        out["applied"] = json!(true);
        out["ids"] = json!(
            plan.ids
                .iter()
                .map(|(name, id)| (name.clone(), json!(id.as_str())))
                .collect::<BTreeMap<_, _>>()
        );
        Ok(out)
    }
}
