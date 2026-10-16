//! Data-only, synchronous document scripting. Nothing touches DocStore until a successful run.
//! Reads see staged Ops; dry runs, exceptions and guard rejections discard everything, including leases.
mod registry;
mod scene;
use crate::{
    diff,
    guard::{GuardState, Verdict, Write},
    ops::{self, Op, Parent, Place, Plan, Transaction},
    ports::{
        ChangeSet, DocStore, IdGen, ScriptApi, ScriptDiagnostic, ScriptError, ScriptLimits,
        ScriptRuntime, ScriptValue, Timestamp,
    },
};
pub use registry::{API_VERSION, Descriptor, Method, REGISTRY, declarations};
pub use scene::{RenderRead, Rendered, SceneConfig, SceneServices};
use serde_json::{Value as Json, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use vellum_types::{Actor, Document, Node, NodeId, NodePatch, Origin};

pub const EXECUTE_LIMITS: ScriptLimits = ScriptLimits {
    time_ms: 5000,
    max_ops: 2000,
    max_nodes: 2000,
    memory_mb: 64,
};
/// Only bounded in-memory operations. Implementations use the same scene/renderer as the editor.
/// `export` returns data; writing files, fetching images and network access are not capabilities here.
pub trait Services {
    fn read(
        &self,
        method: Method,
        document: &Document,
        args: &[ScriptValue],
    ) -> Result<ScriptValue, ScriptError>;
}
pub struct NoServices;
impl Services for NoServices {
    fn read(
        &self,
        method: Method,
        _: &Document,
        _: &[ScriptValue],
    ) -> Result<ScriptValue, ScriptError> {
        Err(error(format!(
            "{method:?} requires a host scene/render service"
        )))
    }
}
pub struct ExecuteContext<'a> {
    pub ids: Rc<RefCell<dyn IdGen>>,
    pub services: Rc<dyn Services>,
    pub guard: &'a mut GuardState,
    pub now: Timestamp,
}
pub struct ExecuteRequest<'a> {
    pub source: &'a str,
    pub origin: Origin,
    pub validate_only: bool,
    pub limits: ScriptLimits,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ExecuteOutput {
    pub value: ScriptValue,
    pub document: Document,
    pub changes: Option<ChangeSet>,
    pub op_count: usize,
    pub committed: bool,
}
struct Pending {
    name: String,
    args: Vec<ScriptValue>,
    plan: Plan,
    value: ScriptValue,
}
struct State {
    document: Document,
    guard: GuardState,
    pending: Option<Pending>,
    calls: usize,
}
struct Api {
    state: RefCell<State>,
    origin: Origin,
    ids: Rc<RefCell<dyn IdGen>>,
    services: Rc<dyn Services>,
    now: Timestamp,
}

pub fn execute(
    runtime: &dyn ScriptRuntime,
    store: &mut dyn DocStore,
    request: ExecuteRequest<'_>,
    context: ExecuteContext<'_>,
) -> Result<ExecuteOutput, ScriptDiagnostic> {
    let before = store.snapshot();
    let api = Rc::new(Api {
        state: RefCell::new(State {
            document: before.clone(),
            guard: context.guard.clone(),
            pending: None,
            calls: 0,
        }),
        origin: request.origin.clone(),
        ids: context.ids,
        services: context.services,
        now: context.now,
    });
    let limits = ScriptLimits {
        time_ms: request.limits.time_ms.min(EXECUTE_LIMITS.time_ms),
        max_ops: request.limits.max_ops.min(EXECUTE_LIMITS.max_ops),
        max_nodes: request.limits.max_nodes.min(EXECUTE_LIMITS.max_nodes),
        memory_mb: request.limits.memory_mb.min(EXECUTE_LIMITS.memory_mb),
    };
    let output = runtime.run_diagnostic(request.source, api.clone(), limits)?;
    let mut state = api.state.borrow_mut();
    let diagnostic = |e| ScriptDiagnostic::new(e, request.source, None);
    // Recheck the full final transaction and the live guard. Diff emits explicit ids, so this plan
    // consumes no new ids and preserves precisely the document the script inspected.
    let ops = diff::diff(&before, &state.document).map_err(|e| diagnostic(error(e)))?;
    let op_count = state.calls;
    let mut changes = None;
    let mut committed = false;
    if !ops.is_empty() {
        let tx = Transaction::new(request.origin.clone(), ops);
        let plan = ops::plan(&before, &tx, &mut *api.ids.borrow_mut())
            .map_err(|e| diagnostic(error(e)))?;
        // Script leases cannot bypass the live pause/stop/lease state at commit.
        for guard in [&*context.guard, &state.guard] {
            if let Verdict::Reject(why) = guard.check(
                &Write {
                    origin: &tx.origin,
                    op_count,
                    before: &before,
                    edits: &plan.edits,
                },
                context.now,
            ) {
                return Err(diagnostic(error(why)));
            }
        }
        changes = Some(plan.changes.clone());
        if !request.validate_only {
            if let Actor::Agent { agent } = &tx.origin.actor
                && let Verdict::Allow { own_frames } = state.guard.check(
                    &Write {
                        origin: &tx.origin,
                        op_count,
                        before: &before,
                        edits: &plan.edits,
                    },
                    context.now,
                )
            {
                for frame in own_frames {
                    state.guard.leases.activity(&frame, agent, context.now);
                }
            }
            ops::commit(store, tx.origin, plan);
            committed = true;
        }
    }
    if !request.validate_only {
        *context.guard = state.guard.clone();
    }
    Ok(ExecuteOutput {
        value: output.value,
        document: state.document.clone(),
        changes,
        op_count,
        committed,
    })
}
impl ScriptApi for Api {
    fn functions(&self) -> &[&str] {
        registry::FUNCTIONS
    }
    fn constants(&self) -> BTreeMap<String, ScriptValue> {
        BTreeMap::from([("apiVersion".into(), ScriptValue::Text(API_VERSION.into()))])
    }
    fn node_cost(&self, name: &str, args: &[ScriptValue]) -> Result<u32, ScriptError> {
        let d = descriptor(name)?;
        if !d.mutates {
            return Ok(0);
        }
        let mut state = self.state.borrow_mut();
        let (op, mut value) = self.operation(d.method, &state.document, args)?;
        let plan = ops::plan(
            &state.document,
            &Transaction::new(self.origin.clone(), vec![op]),
            &mut *self.ids.borrow_mut(),
        )
        .map_err(error)?;
        if let Some(id) = plan.created.get(&0) {
            value = ScriptValue::Text(id.as_str().into());
        }
        // Count inserted trees in pages AND component definitions, even if an existing id is reused.
        let cost = created_cost(&plan.edits)?;
        state.pending = Some(Pending {
            name: name.into(),
            args: args.into(),
            plan,
            value,
        });
        Ok(cost)
    }
    fn call(&self, name: &str, args: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
        let d = descriptor(name)?;
        let mut state = self.state.borrow_mut();
        state.calls += 1;
        if d.mutates {
            let pending = state
                .pending
                .take()
                .filter(|p| p.name == name && p.args == args)
                .ok_or_else(|| error("Call was not costed"))?;
            if let Verdict::Reject(why) = state.guard.check(
                &Write {
                    origin: &self.origin,
                    op_count: state.calls,
                    before: &state.document,
                    edits: &pending.plan.edits,
                },
                self.now,
            ) {
                return Err(error(why));
            }
            state.document = pending.plan.document;
            return Ok(pending.value);
        }
        let doc = &state.document;
        match d.method {
            Method::Page => {
                let name = text(args, 0)?;
                let by_id = doc.pages.iter().find(|p| p.id.as_str() == name);
                let matches = doc
                    .pages
                    .iter()
                    .filter(|p| p.name == name)
                    .collect::<Vec<_>>();
                let p = by_id
                    .or_else(|| {
                        if matches.len() == 1 {
                            Some(matches[0])
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| error("Page missing or name ambiguous"))?;
                Ok(ScriptValue::Text(p.id.as_str().into()))
            }
            Method::Get => Ok(ops::find(doc, &node_id(args, 0)?)
                .map_or(Ok(ScriptValue::Null), |n| {
                    from_json(serde_json::to_value(n).map_err(error)?)
                })?),
            Method::Query => {
                let filter = args.first().map(to_json).transpose()?.unwrap_or(json!({}));
                let filter = filter
                    .as_object()
                    .ok_or_else(|| error("Expected query object"))?;
                if filter
                    .keys()
                    .any(|k| !matches!(k.as_str(), "type" | "name" | "scope"))
                {
                    return Err(error("Unknown query filter"));
                }
                for value in filter.values() {
                    if !value.is_string() {
                        return Err(error("Query values must be strings"));
                    }
                }
                let mut nodes = Vec::new();
                if let Some(scope) = filter.get("scope").and_then(Json::as_str) {
                    if let Some(p) = doc.pages.iter().find(|p| p.id.as_str() == scope) {
                        walk(&p.children, &mut nodes);
                    } else {
                        let id = NodeId::new(scope).map_err(error)?;
                        let node =
                            ops::find(doc, &id).ok_or_else(|| error("Unknown query scope"))?;
                        walk(std::slice::from_ref(node), &mut nodes);
                    }
                } else {
                    for p in &doc.pages {
                        walk(&p.children, &mut nodes);
                    }
                    for c in doc.components.values() {
                        walk(std::slice::from_ref(&c.root), &mut nodes);
                    }
                }
                from_json(Json::Array(
                    nodes
                        .into_iter()
                        .filter(|n| {
                            filter
                                .get("type")
                                .is_none_or(|v| v.as_str() == Some(n.kind().as_str()))
                                && filter
                                    .get("name")
                                    .is_none_or(|v| v.as_str() == Some(n.name()))
                        })
                        .map(|n| serde_json::to_value(n).expect("node JSON"))
                        .collect(),
                ))
            }
            Method::Claim | Method::Release => {
                let id = node_id(args, 0)?;
                if !matches!(ops::find(doc, &id), Some(Node::Frame(_))) {
                    return Err(error("A lease needs a frame"));
                }
                let Actor::Agent { agent } = &self.origin.actor else {
                    return Err(error("Only agents claim leases"));
                };
                state.guard.pause.check(agent).map_err(error)?;
                if d.method == Method::Claim {
                    let blocked = state.guard.blocked.clone();
                    state
                        .guard
                        .leases
                        .claim(&id, agent.clone(), &blocked, self.now)
                        .map_err(error)?;
                } else {
                    state
                        .guard
                        .leases
                        .release(&id, agent, self.now)
                        .map_err(error)?;
                }
                Ok(ScriptValue::Null)
            }
            _ => self.services.read(d.method, doc, args),
        }
    }
}
impl Api {
    fn operation(
        &self,
        m: Method,
        doc: &Document,
        args: &[ScriptValue],
    ) -> Result<(Op, ScriptValue), ScriptError> {
        let id_result = |id: &NodeId| ScriptValue::Text(id.as_str().into());
        let id = || node_id(args, 0);
        let parse_node = |i| normalize_node(to_json(arg(args, i)?)?, &mut *self.ids.borrow_mut());
        Ok(match m {
            Method::Insert => {
                let node = parse_node(1)?;
                let value = id_result(node.id());
                (
                    Op::Insert {
                        to: place(doc, args, 0, 2, None)?,
                        node: Box::new(node),
                    },
                    value,
                )
            }
            Method::Update => (
                Op::Update {
                    id: id()?,
                    patch: serde_json::from_value::<NodePatch>(to_json(arg(args, 1)?)?)
                        .map_err(error)?,
                },
                id_result(&id()?),
            ),
            Method::Copy => (
                Op::Copy {
                    id: id()?,
                    to: place(doc, args, 1, 2, None)?,
                },
                ScriptValue::Null,
            ),
            Method::Replace => {
                let node = parse_node(1)?;
                let value = id_result(node.id());
                (
                    Op::Replace {
                        id: id()?,
                        node: Box::new(node),
                    },
                    value,
                )
            }
            Method::Move => (
                Op::Move {
                    id: id()?,
                    to: place(doc, args, 1, 2, Some(&id()?))?,
                },
                id_result(&id()?),
            ),
            Method::Delete => (Op::Delete { id: id()? }, ScriptValue::Null),
            Method::SetVariables => (
                Op::SetVariables {
                    set: serde_json::from_value(to_json(arg(args, 0)?)?).map_err(error)?,
                    remove: args
                        .get(1)
                        .map(|v| serde_json::from_value(to_json(v)?).map_err(error))
                        .transpose()?
                        .unwrap_or_default(),
                },
                ScriptValue::Null,
            ),
            Method::CreateComponent => {
                let component_id = match args.get(1) {
                    Some(_) => text(args, 1)?.parse().map_err(error)?,
                    None => self.ids.borrow_mut().component_id(),
                };
                let value = ScriptValue::Text(component_id.to_string());
                (
                    Op::CreateComponent {
                        id: id()?,
                        component_id,
                    },
                    value,
                )
            }
            Method::MakeSlot => (
                Op::MakeSlot {
                    id: id()?,
                    suggested: args
                        .get(1)
                        .map(|v| serde_json::from_value(to_json(v)?).map_err(error))
                        .transpose()?
                        .unwrap_or_default(),
                },
                id_result(&id()?),
            ),
            Method::Instance | Method::Icon => {
                let mut spec = args.get(2).map(to_json).transpose()?.unwrap_or(json!({}));
                let obj = spec
                    .as_object_mut()
                    .ok_or_else(|| error("Expected options object"))?;
                let (kind, key) = if m == Method::Instance {
                    ("instance", "componentId")
                } else {
                    ("icon", "icon")
                };
                if obj.contains_key("type") || obj.contains_key(key) || obj.contains_key("id") {
                    return Err(error("Options cannot override identity"));
                }
                obj.insert("type".into(), json!(kind));
                obj.insert(key.into(), json!(text(args, 1)?));
                if m == Method::Instance && !obj.contains_key("library") {
                    let component_id =
                        vellum_types::ComponentId::new(text(args, 1)?).map_err(error)?;
                    let root = &doc
                        .components
                        .get(&component_id)
                        .ok_or_else(|| error("Unknown local component"))?
                        .root;
                    obj.entry("width")
                        .or_insert(serde_json::to_value(root.width()).map_err(error)?);
                    obj.entry("height")
                        .or_insert(serde_json::to_value(root.height()).map_err(error)?);
                    obj.entry("name").or_insert(json!(root.name()));
                }
                if m == Method::Icon {
                    let (library, icon) = text(args, 1)?
                        .split_once(':')
                        .ok_or_else(|| error("An icon uses library:name"))?;
                    if library.is_empty() || icon.is_empty() {
                        return Err(error("An icon needs a library and name"));
                    }
                    obj.entry("library").or_insert_with(|| json!(library));
                    obj.insert("icon".into(), json!(icon));
                    obj.entry("weight").or_insert(json!(400));
                }
                let node = normalize_node(spec, &mut *self.ids.borrow_mut())?;
                let value = id_result(node.id());
                (
                    Op::Insert {
                        to: place(doc, args, 0, usize::MAX, None)?,
                        node: Box::new(node),
                    },
                    value,
                )
            }
            Method::FillImage => {
                let asset = vellum_types::AssetId::new(text(args, 1)?).map_err(error)?;
                let patch =
                    serde_json::from_value(json!({"fills":[{"type":"image","asset":asset}]}))
                        .map_err(error)?;
                (Op::Update { id: id()?, patch }, id_result(&id()?))
            }
            _ => return Err(error("Not a document mutation")),
        })
    }
}
fn descriptor(name: &str) -> Result<&'static Descriptor, ScriptError> {
    REGISTRY
        .iter()
        .find(|d| d.name == name)
        .ok_or_else(|| error("Unknown doc API method"))
}
fn arg(args: &[ScriptValue], index: usize) -> Result<&ScriptValue, ScriptError> {
    args.get(index)
        .ok_or_else(|| error(format!("Missing argument {}", index + 1)))
}
fn text(args: &[ScriptValue], index: usize) -> Result<&str, ScriptError> {
    match arg(args, index)? {
        ScriptValue::Text(s) => Ok(s),
        _ => Err(error("Expected a string")),
    }
}
fn node_id(args: &[ScriptValue], index: usize) -> Result<NodeId, ScriptError> {
    NodeId::new(text(args, index)?).map_err(error)
}
fn place(
    doc: &Document,
    args: &[ScriptValue],
    parent: usize,
    index: usize,
    moving: Option<&NodeId>,
) -> Result<Place, ScriptError> {
    let raw = text(args, parent)?;
    let (parent, children) = if let Some(p) = doc.pages.iter().find(|p| p.id.as_str() == raw) {
        (Parent::Page(p.id.clone()), &p.children[..])
    } else {
        let id = NodeId::new(raw).map_err(error)?;
        let n = ops::find(doc, &id).ok_or_else(|| error("Unknown parent"))?;
        (Parent::Node(id), n.children())
    };
    let len = children.len()
        - usize::from(moving.is_some_and(|id| children.iter().any(|n| n.id() == id)));
    let index = match args.get(index) {
        None => len,
        Some(ScriptValue::Number(n))
            if n.is_finite() && *n >= 0. && n.fract() == 0. && *n <= len as f64 =>
        {
            *n as usize
        }
        _ => return Err(error("Invalid child index")),
    };
    Ok(Place { parent, index })
}
fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
    for n in nodes {
        out.push(n);
        walk(n.children(), out);
    }
}
pub(crate) fn count(node: &Node) -> usize {
    let mut ids = Vec::new();
    ops::edit::subtree_ids(node, &mut ids);
    ids.len()
}
fn created_cost(edits: &[ops::Edit]) -> Result<u32, ScriptError> {
    let count = edits
        .iter()
        .map(|e| match e {
            ops::Edit::Insert { node, .. } => count(node),
            ops::Edit::InsertPage { page, .. } => page.children.iter().map(count).sum(),
            ops::Edit::SetComponent {
                component: Some(c), ..
            } => count(&c.root),
            _ => 0,
        })
        .sum::<usize>();
    u32::try_from(count).map_err(|_| ScriptError::NodeLimit)
}
pub(crate) fn normalize_node(mut json: Json, ids: &mut dyn IdGen) -> Result<Node, ScriptError> {
    fn fill(v: &mut Json, ids: &mut dyn IdGen, depth: usize) -> Result<(), ScriptError> {
        if depth > 32 {
            return Err(error("Node nesting exceeds 32"));
        }
        let o = v
            .as_object_mut()
            .ok_or_else(|| error("Expected node object"))?;
        o.entry("name").or_insert_with(|| json!(""));
        o.entry("id").or_insert_with(|| json!(ids.node_id()));
        if let Some(children) = o.get_mut("children") {
            for child in children
                .as_array_mut()
                .ok_or_else(|| error("Expected children array"))?
            {
                fill(child, ids, depth + 1)?;
            }
        }
        Ok(())
    }
    fill(&mut json, ids, 0)?;
    serde_json::from_value(json).map_err(error)
}
pub fn error(message: impl std::fmt::Display) -> ScriptError {
    ScriptError::Api {
        message: message.to_string(),
    }
}
pub fn to_json(v: &ScriptValue) -> Result<Json, ScriptError> {
    Ok(match v {
        ScriptValue::Null => Json::Null,
        ScriptValue::Bool(v) => json!(v),
        ScriptValue::Number(v) => serde_json::Number::from_f64(*v)
            .map(Json::Number)
            .ok_or_else(|| error("Nonfinite number"))?,
        ScriptValue::Text(v) => json!(v),
        ScriptValue::List(v) => Json::Array(v.iter().map(to_json).collect::<Result<_, _>>()?),
        ScriptValue::Map(v) => Json::Object(
            v.iter()
                .map(|(k, v)| Ok((k.clone(), to_json(v)?)))
                .collect::<Result<_, ScriptError>>()?,
        ),
    })
}
pub fn from_json(v: Json) -> Result<ScriptValue, ScriptError> {
    Ok(match v {
        Json::Null => ScriptValue::Null,
        Json::Bool(v) => ScriptValue::Bool(v),
        Json::Number(v) => {
            ScriptValue::Number(v.as_f64().ok_or_else(|| error("Number out of range"))?)
        }
        Json::String(v) => ScriptValue::Text(v),
        Json::Array(v) => {
            ScriptValue::List(v.into_iter().map(from_json).collect::<Result<_, _>>()?)
        }
        Json::Object(v) => ScriptValue::Map(
            v.into_iter()
                .map(|(k, v)| Ok((k, from_json(v)?)))
                .collect::<Result<_, ScriptError>>()?,
        ),
    })
}
