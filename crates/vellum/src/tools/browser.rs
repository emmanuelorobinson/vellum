//! Browsing is outside the canvas. Importing its captured layers is one attributed transaction.
use super::{ErrorKind, Tool, ToolCtx, ToolError, Verb, parse_input, schema_of};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::ports::{BrowserRequest, ImportInput, Importer};
pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(Browser));
}
/// Browsing addresses/selectors can contain credentials even when validation rejects the call.
/// The step ledger keeps action/device/destination context, never those raw strings.
pub(super) fn history_input(input: &Value) -> Value {
    let mut safe = json!({"url":"[omitted]"});
    for key in [
        "action", "document", "parent", "index", "width", "height", "device", "zoom", "scroll",
        "x", "y",
    ] {
        if let Some(value) = input.get(key) {
            safe[key] = value.clone();
        }
    }
    safe
}
struct Browser;
#[derive(Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Action {
    Screenshot,
    Import,
    ImportElement,
    Embed,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    url: String,
    action: Action,
    document: Option<String>,
    parent: Option<String>,
    index: Option<u32>,
    selector: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    origins: Option<Vec<String>>,
    x: Option<f64>,
    y: Option<f64>,
    device: Option<String>,
    zoom: Option<f64>,
    scroll: Option<[f64; 2]>,
}
impl Tool for Browser {
    fn name(&self) -> &'static str {
        "browser"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Capture an HTTP(S) page, including localhost dev servers, as a PNG or import it as editable layers. import_element needs a CSS selector. Give the viewport in pixels; additional asset origins need separate permission. embed adds a live website node. Imports need a parent and use one undoable transaction. Unsupported content is reported in notes. Open a document first so network permissions can be recorded."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<Input>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn open_world(&self) -> bool {
        true
    }
    fn destructive(&self) -> bool {
        false
    }
    fn verb(&self) -> Verb {
        Verb::AddedAsset
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, value: Value) -> Result<Value, ToolError> {
        let input: Input = parse_input(value)?;
        if input.action == Action::ImportElement && input.selector.is_none() {
            return Err(ToolError::invalid(
                "selector",
                "import_element needs a CSS selector.",
            ));
        }
        if input.action != Action::Screenshot && input.parent.is_none() {
            return Err(ToolError::invalid(
                "parent",
                "Give the page or frame to import into.",
            ));
        }
        let request = BrowserRequest {
            url: input.url,
            origins: input.origins.unwrap_or_default(),
            width: input
                .width
                .unwrap_or(if input.device.as_deref() == Some("phone") {
                    375
                } else {
                    1280
                }),
            height: input
                .height
                .unwrap_or(if input.device.as_deref() == Some("phone") {
                    812
                } else {
                    800
                }),
            selector: input.selector,
            scroll: input.scroll.unwrap_or([0., 0.]),
            zoom: input.zoom.unwrap_or(1.),
            snapshot: matches!(input.action, Action::Import | Action::ImportElement),
        };
        let origins = vellum_browser_cdp::authorized_origins(&request)
            .map_err(|e| ToolError::invalid("url", e.to_string()))?;
        // Validate destination before asking or launching; Guard validates the actual transaction too.
        if let Some(parent) = &input.parent {
            let (_, doc) = ctx.document(input.document.as_deref())?;
            super::batch::place(&doc, parent, input.index, None, "parent")?;
        }
        if input.action == Action::Embed {
            // An embed does not navigate until the host renders it through its own permission boundary.
            let imported = vellum_core::ports::Imported {
                nodes: vec![
                    json!({"type":"embed","name":"Website","url":request.url,"device":input.device,"zoom":request.zoom,"scroll":{"x":request.scroll[0],"y":request.scroll[1]},"width":request.width,"height":request.height}),
                ],
                ..Default::default()
            };
            return super::assets::apply_imported(
                ctx,
                input.document.as_deref(),
                input.parent.as_deref().expect("validated"),
                input.index,
                input.x,
                input.y,
                "embed",
                imported,
            );
        }
        let browser = ctx.docs().browser().ok_or_else(|| {
            ToolError::new(
                ErrorKind::Document,
                "No browser is configured in this host.",
            )
        })?;
        let outside = ctx.docs().outside().ok_or_else(|| {
            ToolError::new(
                ErrorKind::Refused,
                "Browsing needs a recorded network permission. No permission host is connected.",
            )
        })?;
        let origin = ctx.origin();
        for url in &origins {
            outside.network(&origin, url)?;
        }
        let capture = browser
            .capture(&request)
            .map_err(|e| ToolError::new(ErrorKind::Document, e.to_string()))?;
        if input.action == Action::Screenshot {
            return Ok(
                json!({"image":{"mime":"image/png","base64":vellum_core::base64::encode(&capture.png)},"width":capture.width,"height":capture.height}),
            );
        }
        let dom = capture.dom.ok_or_else(|| {
            ToolError::new(
                ErrorKind::Document,
                "The browser returned no computed DOM snapshot.",
            )
        })?;
        let bytes = serde_json::to_vec(&dom).map_err(|_| {
            ToolError::new(
                ErrorKind::Internal,
                "Could not encode the browser snapshot.",
            )
        })?;
        let imported = vellum_import_html::HtmlImporter
            .import(&ImportInput {
                bytes: &bytes,
                name: "Website",
            })
            .map_err(|e| ToolError::invalid("url", e.to_string()))?;
        super::assets::apply_imported(
            ctx,
            input.document.as_deref(),
            input.parent.as_deref().expect("validated"),
            input.index,
            input.x,
            input.y,
            "html",
            imported,
        )
    }
}
