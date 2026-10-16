//! Context tools: what is open, who holds it, and what the person has said to agents (`open_document`,
//! `get_app_state`; ARCHITECTURE §7.3).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::ports::TargetMode;
use vellum_types::{Document, Node};

use super::support::{Index, clip, parse_input, schema_of};
use super::{Tool, ToolCtx, ToolError};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(OpenDocument));
    tools.push(Box::new(GetAppState));
}

/// The longest note or context text shown whole.
const TEXT_LIMIT: usize = 2_000;
/// The most notes and context nodes listed.
const LIST_LIMIT: usize = 50;

fn mode(mode: TargetMode) -> &'static str {
    match mode {
        TargetMode::Live => "live",
        TargetMode::Headless => "headless",
    }
}

fn page_summaries(doc: &Document) -> Vec<Value> {
    let index = Index::build(doc);
    doc.pages
        .iter()
        .map(|page| {
            let nodes = index.scope(page.id.as_str()).map_or(0, |r| r.len());
            json!({"id": page.id.as_str(), "name": page.name, "kind": page.kind.as_str(), "nodes": nodes})
        })
        .collect()
}

struct OpenDocument;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenInput {
    /// The path of the .vellum file, relative to the repo.
    path: String,
    /// Make the file when it is not there. An existing file is opened, never replaced.
    #[serde(default)]
    create: bool,
    /// The name for a new document. Defaults to the file name.
    name: Option<String>,
}

impl Tool for OpenDocument {
    fn name(&self) -> &'static str {
        "open_document"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Open a .vellum document, or create it with `create: true`. If an editor has the file open, work happens \
         in that editor; otherwise the file is opened without one. Returns the pages. Calls that leave out \
         `document` use the document opened last."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<OpenInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        false
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let OpenInput { path, create, name } = parse_input(input)?;
        let path = ctx.resolve(&path)?;
        let mut created = false;
        if !ctx.docs().exists(&path)? {
            if !create {
                return Err(ToolError::not_found(format!(
                    "There is no document at {path}. Pass `create: true` to make it."
                )));
            }
            let name = name.unwrap_or_else(|| {
                let file = path.rsplit('/').next().unwrap_or(&path);
                file.split('.').next().unwrap_or(file).to_owned()
            });
            ctx.docs().create_blank(&path, &name)?;
            created = true;
        }
        let doc = ctx.snapshot(&path)?;
        let view = ctx.docs().view(&path)?;
        let mut out = json!({
            "path": path,
            "name": doc.meta.name,
            "mode": mode(view.mode),
            "created": created,
            "dirty": view.dirty,
            "pages": page_summaries(&doc),
        });
        if let Some(editor) = view.writer {
            out["editor"] = json!(editor.0);
        }
        Ok(out)
    }
}

struct GetAppState;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StateInput {
    /// The document whose selection, leases and notes to report. Defaults to the active document.
    document: Option<String>,
}

impl Tool for GetAppState {
    fn name(&self) -> &'static str {
        "get_app_state"
    }
    fn version(&self) -> u32 {
        2
    }
    fn description(&self) -> &'static str {
        "Report what is open and what the person sees: open documents, the active one, its selection and \
         viewport, which frames are leased (and by which agent; `you` is this client's), the planned frames \
         other agents have reserved (placeholders: leave their areas free), which agents are paused, and the \
         notes for agents and context nodes that apply to the selection (or the page, with no selection). Call \
         it first, and again before starting on a new request."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<StateInput>()
    }
    fn read_only(&self) -> bool {
        true
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let StateInput { document } = parse_input(input)?;
        let paths = ctx.visible_paths();
        let focus = match document {
            Some(named) => Some(ctx.resolve(&named)?),
            None => ctx.default_document(),
        };

        let mut documents = Vec::new();
        for path in &paths {
            let mut entry = json!({"path": path, "active": Some(path) == focus.as_ref()});
            match (
                ctx.docs().view(path),
                ctx.docs().target(path).and_then(|t| t.snapshot()),
            ) {
                (Ok(view), Ok(doc)) => {
                    entry["name"] = json!(doc.meta.name);
                    entry["mode"] = json!(mode(view.mode));
                    entry["dirty"] = json!(view.dirty);
                    entry["pages"] = json!(doc.pages.len());
                    if let Some(editor) = view.writer {
                        entry["editor"] = json!(editor.0);
                    }
                }
                (Err(error), _) | (_, Err(error)) => entry["error"] = json!(error.to_string()),
            }
            documents.push(entry);
        }

        let you = json!({
            "agent": ctx.session().agent().map(|a| a.as_str()),
            "name": ctx.session().agent_name(),
        });
        let agents: Vec<Value> = ctx
            .docs()
            .agents()
            .iter()
            .map(|(id, name)| json!({"agent": id.as_str(), "name": name}))
            .collect();
        let mut out = json!({
            "you": you,
            "agents": agents,
            "documents": documents,
            "active": focus,
            "selection": [],
            "viewport": null,
            "leases": [],
            "placeholders": [],
            "paused": {"all": false, "agents": [], "stopped": []},
            "scope": null,
            "notes": [],
            "context": [],
            "truncated": false,
        });
        let Some(path) = focus else {
            return Ok(out);
        };
        // A document named but not open is opened (headless) so the answer is about it.
        let doc = ctx.snapshot(&path)?;
        let view = ctx.docs().view(&path)?;
        let state = &view.state;
        out["selection"] = json!(state.selection);
        out["viewport"] = json!(state.viewport);
        out["leases"] = json!(state.leases);
        let names = ctx.docs().agents().clone();
        out["placeholders"] = ctx
            .docs()
            .placeholders(&path)
            .iter()
            .map(|planned| {
                let p = &planned.placeholder;
                json!({
                    "item": p.item,
                    "name": p.name,
                    "frame": p.frame.as_str(),
                    "page": p.page.as_str(),
                    "bounds": {"x": p.bounds.x, "y": p.bounds.y, "width": p.bounds.width, "height": p.bounds.height},
                    "agent": p.agent.as_str(),
                    "agentName": names.get(&p.agent),
                    "filling": planned.filling,
                })
            })
            .collect();
        out["paused"] = json!(state.paused);

        let index = Index::build(&doc);
        let selected: Vec<usize> = state
            .selection
            .iter()
            .filter_map(|id| index.position(id.as_str()))
            .collect();
        let page = selected
            .first()
            .and_then(|at| index.entries[*at].page)
            .map(|p| p.id.clone())
            .or_else(|| state.page.clone())
            .or_else(|| doc.pages.first().map(|p| p.id.clone()));
        out["scope"] = json!({
            "page": page,
            "nodes": selected.iter().map(|at| index.entries[*at].node.id().as_str()).collect::<Vec<_>>(),
        });

        // What is in scope: the selection's subtrees, or the whole page; plus the selection's ancestors,
        // whose notes and context still apply to what is inside them.
        let mut within = BTreeSet::new();
        let mut above = BTreeSet::new();
        if selected.is_empty() {
            if let Some(range) = page.as_ref().and_then(|p| index.scope(p.as_str())) {
                within.extend(range);
            }
        } else {
            for &at in &selected {
                within.extend(at..index.entries[at].end);
                above.extend(index.ancestors_or_self(at));
            }
        }
        let applies = |at: usize| within.contains(&at) || above.contains(&at);

        let mut truncated = false;
        let mut notes = Vec::new();
        let mut context = Vec::new();
        for (at, entry) in index.entries.iter().enumerate() {
            if let Some(text) = entry.node.agent_notes()
                && applies(at)
            {
                if notes.len() == LIST_LIMIT {
                    truncated = true;
                } else {
                    let (text, cut) = clip(text, TEXT_LIMIT);
                    truncated |= cut;
                    notes.push(json!({"node": entry.node.id().as_str(), "name": entry.node.name(), "text": text}));
                }
            }
            let Node::Context(node) = entry.node else {
                continue;
            };
            // A context node speaks for the node it points at, otherwise for the frame (or page) it sits in.
            let target = node
                .points_at
                .as_ref()
                .map(|id| id.as_str().to_owned())
                .or_else(|| entry.parent.map(|p| p.as_str().to_owned()))
                .or_else(|| entry.page.map(|p| p.id.as_str().to_owned()));
            let hit = target.as_deref().is_some_and(|target| {
                index.position(target).map_or_else(
                    || page.as_ref().is_some_and(|p| p.as_str() == target) && selected.is_empty(),
                    applies,
                )
            });
            if !hit {
                continue;
            }
            if context.len() == LIST_LIMIT {
                truncated = true;
            } else {
                let (text, cut) = clip(&node.content, TEXT_LIMIT);
                truncated |= cut;
                context.push(json!({"node": node.id.as_str(), "appliesTo": target, "text": text}));
            }
        }
        out["notes"] = json!(notes);
        out["context"] = json!(context);
        out["truncated"] = json!(truncated);
        Ok(out)
    }
}
