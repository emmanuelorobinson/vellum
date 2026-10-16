//! Script output is derived data, separate from the document and undo. Hosts supply source bytes
//! after resolving the document-relative path; this pure module never opens a file.
mod header;
use crate::{
    doc_api::{API_VERSION, count, error, from_json, normalize_node, to_json},
    ops::{self, Op, Parent, Place, Transaction},
    ports::{
        IdGen, ScriptApi, ScriptDiagnostic, ScriptError, ScriptLimits, ScriptRuntime, ScriptValue,
    },
    resolver::variables::{self, ResolvedValue, Theme},
};
pub use header::{Header, Input, InputType};
use std::{collections::BTreeMap, rc::Rc};
use vellum_types::{
    Actor, Document, Node, NodeId, Origin, Page, PageId, PageKind, Scalar, Subsystem, Value,
};
pub const SCRIPT_NODE_LIMITS: ScriptLimits = ScriptLimits {
    time_ms: 2000,
    max_ops: 2000,
    max_nodes: 1000,
    memory_mb: 64,
};
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub nodes: Vec<Node>,
    pub clip: bool,
    pub header: Header,
    script: vellum_types::ScriptNode,
    inputs: BTreeMap<String, ScriptValue>,
}
struct Inputs(BTreeMap<String, ScriptValue>);
impl ScriptApi for Inputs {
    fn constants(&self) -> BTreeMap<String, ScriptValue> {
        BTreeMap::from([
            ("apiVersion".into(), ScriptValue::Text(API_VERSION.into())),
            ("inputs".into(), ScriptValue::Map(self.0.clone())),
        ])
    }
    fn node_cost(&self, _: &str, _: &[ScriptValue]) -> Result<u32, ScriptError> {
        Err(error(
            "Script nodes return nodes; they cannot edit the document",
        ))
    }
    fn call(&self, _: &str, _: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
        Err(error("Script nodes cannot edit the document"))
    }
}
pub fn evaluate(
    runtime: &dyn ScriptRuntime,
    document: &Document,
    node: &Node,
    source: &str,
    theme: &Theme,
    ids: &mut dyn IdGen,
) -> Result<Output, ScriptDiagnostic> {
    let diag = |e| ScriptDiagnostic::new(e, source, None);
    let Node::Script(script) = node else {
        return Err(diag(error("Expected script node")));
    };
    let header = Header::parse(source).map_err(diag)?;
    if script
        .inputs
        .keys()
        .any(|k| !header.inputs.iter().any(|i| &i.name == k))
    {
        return Err(diag(error("Unknown script input")));
    }
    let mut inputs = BTreeMap::new();
    for input in &header.inputs {
        let value = match script.inputs.get(&input.name) {
            Some(Value::Literal(value)) => value.clone(),
            Some(Value::Var(reference)) => {
                match variables::resolve_variable(document, reference.id(), theme)
                    .map_err(|e| diag(error(e)))?
                {
                    ResolvedValue::Number(v) => Scalar::Number(v),
                    ResolvedValue::Color(v) => Scalar::Text(v.to_string()),
                    ResolvedValue::String(v) => Scalar::Text(v),
                    ResolvedValue::Boolean(v) => Scalar::Bool(v),
                }
            }
            None => input
                .default
                .clone()
                .ok_or_else(|| diag(error(format!("Missing input {}", input.name))))?,
        };
        input.validate(&value).map_err(diag)?;
        if input.kind == InputType::Ref {
            validate_ref(document, input, &value).map_err(diag)?;
        }
        inputs.insert(
            input.name.clone(),
            from_json(serde_json::to_value(value).map_err(|e| diag(error(e)))?).map_err(diag)?,
        );
    }
    let result =
        runtime.run_diagnostic(source, Rc::new(Inputs(inputs.clone())), SCRIPT_NODE_LIMITS)?;
    let ScriptValue::List(values) = result.value else {
        return Err(diag(error("Script node must return an array of nodes")));
    };
    let mut nodes = Vec::new();
    let mut total = 0;
    for value in values {
        let mut json = to_json(&value).map_err(diag)?;
        // Caller-provided output ids are deliberately ephemeral and cannot refer to stored nodes.
        erase_ids(&mut json);
        let node = normalize_node(json, ids).map_err(diag)?;
        total += count(&node);
        if total > 1000 {
            return Err(diag(ScriptError::NodeLimit));
        }
        if contains_script(&node) {
            return Err(diag(error("Recursive script output is not supported")));
        }
        nodes.push(node);
    }
    let mut isolated = document.clone();
    let page_id = PageId::new("_script_output").expect("id");
    if isolated.pages.iter().any(|p| p.id == page_id) {
        return Err(diag(error("Reserved script output page id is in use")));
    }
    isolated.pages.push(Page {
        id: page_id.clone(),
        name: String::new(),
        kind: PageKind::Canvas,
        children: vec![],
    });
    let ops = nodes
        .iter()
        .enumerate()
        .map(|(index, n)| Op::Insert {
            to: Place {
                parent: Parent::Page(page_id.clone()),
                index,
            },
            node: Box::new(n.clone()),
        })
        .collect::<Vec<_>>();
    if !ops.is_empty() {
        ops::plan(
            &isolated,
            &Transaction::new(
                Origin {
                    actor: Actor::System {
                        subsystem: Subsystem::Importer,
                    },
                    session: None,
                    turn: None,
                },
                ops,
            ),
            ids,
        )
        .map_err(|e| diag(error(e)))?;
    }
    Ok(Output {
        nodes,
        clip: script.clip,
        header,
        script: script.clone(),
        inputs,
    })
}
/// Source file reloads must replace the host's output map. Core independently detects changed
/// inputs, variable values, references, inherited themes and source paths before displaying a cache.
pub(crate) fn is_current(output: &Output, doc: &Document, node: &Node, theme: &Theme) -> bool {
    let Node::Script(script) = node else {
        return false;
    };
    if script.src != output.script.src || script.inputs != output.script.inputs {
        return false;
    }
    output.header.inputs.iter().all(|input| {
        let value = match script.inputs.get(&input.name) {
            Some(Value::Literal(v)) => Some(v.clone()),
            Some(Value::Var(r)) => variables::resolve_variable(doc, r.id(), theme)
                .ok()
                .map(|v| match v {
                    ResolvedValue::Number(v) => Scalar::Number(v),
                    ResolvedValue::Color(v) => Scalar::Text(v.to_string()),
                    ResolvedValue::String(v) => Scalar::Text(v),
                    ResolvedValue::Boolean(v) => Scalar::Bool(v),
                }),
            None => input.default.clone(),
        };
        value.is_some_and(|v| {
            if input.kind == InputType::Ref && validate_ref(doc, input, &v).is_err() {
                return false;
            }
            serde_json::to_value(v)
                .ok()
                .and_then(|v| from_json(v).ok())
                .as_ref()
                == output.inputs.get(&input.name)
        })
    })
}
fn erase_ids(v: &mut serde_json::Value) {
    if let Some(o) = v.as_object_mut() {
        o.remove("id");
        if let Some(children) = o.get_mut("children").and_then(|v| v.as_array_mut()) {
            for child in children {
                erase_ids(child);
            }
        }
    }
}
fn contains_script(n: &Node) -> bool {
    matches!(n, Node::Script(_)) || n.children().iter().any(contains_script)
}
fn validate_ref(doc: &Document, input: &Input, value: &Scalar) -> Result<(), ScriptError> {
    let Scalar::Text(s) = value else {
        return Err(error("Expected ref id"));
    };
    let kind = input.args.get("kind").and_then(|v| v.as_str());
    let exists = match kind {
        Some("page") => doc.pages.iter().any(|p| p.id.as_str() == s),
        Some("component") => doc.components.keys().any(|id| id.as_str() == s),
        Some("variable") => doc.variables.keys().any(|id| id.as_str() == s),
        Some("node") => NodeId::new(s)
            .ok()
            .is_some_and(|id| ops::find(doc, &id).is_some()),
        _ => {
            doc.pages.iter().any(|p| p.id.as_str() == s)
                || NodeId::new(s)
                    .ok()
                    .is_some_and(|id| ops::find(doc, &id).is_some())
        }
    };
    if exists {
        Ok(())
    } else {
        Err(error(format!("Missing reference for input {}", input.name)))
    }
}
