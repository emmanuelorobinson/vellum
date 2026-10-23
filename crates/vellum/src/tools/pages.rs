//! Pages: the sheets a document is made of (ADR-034). An agent adds, names, reorders and removes them with the same
//! checks the editor's page list makes; a refused change names the item that caused it.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::ops::Op;
use vellum_types::{Document, Page, PageId, PageKind};

use super::batch::{Batch, Label, fresh, ids_in_use};
use super::support::{parse_input, schema_of};
use super::write::MAX_ITEMS;
use super::{Tool, ToolCtx, ToolError, Verb};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(ManagePages));
}

struct ManagePages;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PagesInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// Pages to add, first. Each goes at its index among the pages, counted as the list is when it is added; left
    /// out, it goes at the end.
    insert: Option<Vec<NewPage>>,
    /// Pages to rename or change the kind of. What is left out stays as it is.
    update: Option<Vec<PageChange>>,
    /// Pages to move, after the inserts and updates. An index counts the pages after the page has left its place.
    #[serde(rename = "move")]
    moves: Option<Vec<PageMove>>,
    /// Pages to remove, after everything else, each with every layer on it.
    delete: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewPage {
    /// The name shown in the page list.
    name: String,
    /// "canvas" (the default) or "slides".
    kind: Option<PageKind>,
    /// Where among the pages, counting from 0. Left out, the page goes at the end.
    index: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PageChange {
    /// The page.
    id: String,
    /// Its new name.
    name: Option<String>,
    /// Its new kind: "canvas" or "slides". A page with layers that the kind does not allow is refused.
    kind: Option<PageKind>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PageMove {
    /// The page.
    id: String,
    /// Where it ends up among the pages, counting from 0.
    index: u32,
}

impl Tool for ManagePages {
    fn name(&self) -> &'static str {
        "manage_pages"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Add, rename or change the kind of, move and remove pages. Inserts run first, then updates, moves and \
         removals, all as one change. Removing a page removes every layer on it, so read it first."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<PagesInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::Updated
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let PagesInput {
            document,
            insert,
            update,
            moves,
            delete,
        } = parse_input(input)?;
        let (insert, update, moves, delete) = (
            insert.unwrap_or_default(),
            update.unwrap_or_default(),
            moves.unwrap_or_default(),
            delete.unwrap_or_default(),
        );
        let total = insert.len() + update.len() + moves.len() + delete.len();
        if total == 0 {
            return Err(ToolError::invalid(
                "insert",
                "Give at least one page change: insert, update, move or delete.",
            ));
        }
        if total > MAX_ITEMS {
            return Err(ToolError::invalid(
                "insert",
                format!("At most {MAX_ITEMS} page changes in one call; split the batch."),
            ));
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut ids = ctx.docs().tool_ids();
        let mut taken = ids_in_use(&doc);
        let mut batch = Batch::default();
        let mut created = Vec::new();

        for (i, page) in insert.into_iter().enumerate() {
            let at = format!("insert[{i}]");
            let id = PageId::new(fresh(&mut ids, &mut taken)).map_err(|e| {
                ToolError::invalid(&at, format!("A new page id was not usable: {e}."))
            })?;
            // Inserts run first, one after another, so a page added earlier in this call is already counted.
            let index = page
                .index
                .map_or(doc.pages.len() + i, |index| index as usize);
            created.push(id.as_str().to_owned());
            batch.push(
                Op::InsertPage {
                    index,
                    page: Box::new(Page {
                        id,
                        name: page.name,
                        kind: page.kind.unwrap_or_default(),
                        children: Vec::new(),
                    }),
                },
                Label::new(i, at.clone()).rename("page", at),
            );
        }
        for (i, change) in update.into_iter().enumerate() {
            let at = format!("update[{i}]");
            let page =
                find_page(&doc, &change.id).map_err(|e| e.at_op(i).at_path(format!("{at}.id")))?;
            let id = PageId::new(change.id)
                .map_err(|e| ToolError::invalid(format!("{at}.id"), e.to_string()).at_op(i))?;
            batch.push(
                Op::UpdatePage {
                    id,
                    name: change.name.unwrap_or_else(|| page.name.clone()),
                    kind: change.kind.unwrap_or(page.kind),
                },
                Label::new(i, at),
            );
        }
        for (i, step) in moves.into_iter().enumerate() {
            let at = format!("move[{i}]");
            find_page(&doc, &step.id).map_err(|e| e.at_op(i).at_path(format!("{at}.id")))?;
            let id = PageId::new(step.id)
                .map_err(|e| ToolError::invalid(format!("{at}.id"), e.to_string()).at_op(i))?;
            batch.push(
                Op::MovePage {
                    id,
                    index: step.index as usize,
                },
                Label::new(i, at),
            );
        }
        for (i, id) in delete.into_iter().enumerate() {
            let at = format!("delete[{i}]");
            find_page(&doc, &id).map_err(|e| e.at_op(i).at_path(at.clone()))?;
            let id = PageId::new(id)
                .map_err(|e| ToolError::invalid(at.clone(), e.to_string()).at_op(i))?;
            batch.push(Op::DeletePage { id }, Label::new(i, at));
        }

        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "created": created, "changed": total}))
    }
}

/// The page with `id` in `doc`, or a not-found error that names it.
fn find_page<'a>(doc: &'a Document, id: &str) -> Result<&'a Page, ToolError> {
    doc.pages
        .iter()
        .find(|page| page.id.as_str() == id)
        .ok_or_else(|| ToolError::not_found(format!("There is no page {id:?} in this document.")))
}
