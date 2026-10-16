//! Content tools need no document. They are the same tools over MCP, CLI and the built-in engine.
use super::support::{parse_input, schema_of};
use super::{ErrorKind, Tool, ToolCtx, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::rc::Rc;
use vellum_core::ports::{ContentError, SkillGroup, SkillSource, StyleLibrary};

pub(super) fn register(
    tools: &mut Vec<Box<dyn Tool>>,
    skills: Rc<dyn SkillSource>,
    styles: Rc<dyn StyleLibrary>,
) {
    tools.push(Box::new(Skills {
        source: skills.clone(),
        read: false,
    }));
    tools.push(Box::new(Skills {
        source: skills,
        read: true,
    }));
    tools.push(Box::new(Styles(styles)));
}

fn failure(error: ContentError, field: &str) -> ToolError {
    let kind = match &error {
        ContentError::NotFound(_) => ErrorKind::NotFound,
        ContentError::Invalid(_) => ErrorKind::InvalidInput,
        ContentError::Unavailable(_) => ErrorKind::Document,
    };
    ToolError::new(kind, error.to_string()).at_path(field)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListInput {}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadInput {
    /// An exact path returned by list_skills. Omit to list. Built-ins also accept the .md path without its extension.
    path: Option<String>,
}
struct Skills {
    source: Rc<dyn SkillSource>,
    read: bool,
}
impl Tool for Skills {
    fn name(&self) -> &'static str {
        if self.read {
            "read_skill"
        } else {
            "list_skills"
        }
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        if self.read {
            "Read instructions by a path from list_skills: format.md, execute.md, vellum-doc.d.ts, workflows, design-system guidelines or added SKILL.md folders. Omit path to list available skills. Read execute.md before scripting; read relevant guidelines and notes before designing."
        } else {
            "List available skills with paths, descriptions and source groups: built-in, design system, yours. Read only the instructions relevant to the task with read_skill; no document needs to be open."
        }
    }
    fn input_schema(&self) -> schemars::Schema {
        if self.read {
            schema_of::<ReadInput>()
        } else {
            let mut schema = schema_of::<ListInput>();
            // Keep the registry's object-schema shape even for a tool with no fields.
            schema
                .as_object_mut()
                .expect("object input")
                .insert("properties".into(), json!({}));
            schema
        }
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, _: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let requested = if self.read {
            parse_input::<ReadInput>(input)?.path
        } else {
            let _: ListInput = parse_input(input)?;
            None
        };
        if let Some(path) = requested {
            if path.trim().is_empty() {
                return Err(ToolError::invalid(
                    "path",
                    "Use a path from list_skills, or omit path to list.",
                ));
            }
            let canonical = if ["format", "execute"].contains(&path.as_str()) {
                format!("{path}.md")
            } else {
                path
            };
            let text = self
                .source
                .read(&canonical)
                .map_err(|e| failure(e, "path"))?;
            return Ok(json!({"path":canonical, "content":text}));
        }
        let list = self.source.list().map_err(|e| failure(e, ""))?;
        let skills: Vec<_> = list.into_iter().map(|m| json!({"path":m.path, "name":m.name, "description":m.description,
            "group":match m.group { SkillGroup::Builtin=>"builtin", SkillGroup::DesignSystem=>"design_system", SkillGroup::User=>"user" }})).collect();
        Ok(json!({"skills":skills}))
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StyleInput {
    /// A name returned by get_style. Omit to list the available recipes.
    name: Option<String>,
}
struct Styles(Rc<dyn StyleLibrary>);
impl Tool for Styles {
    fn name(&self) -> &'static str {
        "get_style"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "List visual styles, or load one by name: palette, typography, radius and density. Use a recipe when no design system is set; existing document variables and system guidelines take precedence. Loading a style does not edit the document or install fonts."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<StyleInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, _: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let StyleInput { name } = parse_input(input)?;
        if let Some(name) = name {
            if name.trim().is_empty() {
                return Err(ToolError::invalid(
                    "name",
                    "Use a listed style name, or omit name to list.",
                ));
            }
            let s = self.0.read(&name).map_err(|e| failure(e, "name"))?;
            Ok(
                json!({"style": {"name":s.name, "description":s.description, "palette":s.palette,
                "typography":{"body":s.typography.body, "heading":s.typography.heading, "mono":s.typography.mono, "baseSize":s.typography.base_size, "scale":s.typography.scale},
                "radius":s.radius, "density":{"gap":s.density.gap, "padding":s.density.padding, "controlHeight":s.density.control_height}}}),
            )
        } else {
            let styles: Vec<_> = self
                .0
                .list()
                .map_err(|e| failure(e, ""))?
                .into_iter()
                .map(|s| json!({"name":s.name, "description":s.description}))
                .collect();
            Ok(json!({"styles":styles}))
        }
    }
}
