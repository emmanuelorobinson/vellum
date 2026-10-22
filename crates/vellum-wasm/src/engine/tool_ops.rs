//! The editor's person transactions (`@vellum/engine-client` `ToolTransaction`) as core `Transaction`s. The
//! TypeScript `ToolOp` union names a subset of `vellum_core::ops::Op` in its own JSON spelling; this is the one
//! place that spelling is read. It is an in-process request shape, not a file or wire format.
use serde::de::DeserializeOwned;
use serde_json::Value;
use vellum_core::ops::{Op, Parent, Place, Shell, Transaction};
use vellum_types::{Actor, Origin};

/// Why a request could not be read. The engine answers `invalid` and changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable(pub String);

fn field<T: DeserializeOwned>(op: &Value, key: &str) -> Result<T, Unreadable> {
    let value = op
        .get(key)
        .ok_or_else(|| Unreadable(format!("missing {key}")))?;
    serde_json::from_value(value.clone()).map_err(|e| Unreadable(format!("{key}: {e}")))
}

fn place(op: &Value) -> Result<Place, Unreadable> {
    let to = op
        .get("to")
        .ok_or_else(|| Unreadable("missing to".into()))?;
    let parent = to
        .get("parent")
        .ok_or_else(|| Unreadable("missing to.parent".into()))?;
    let parent = match parent.get("type").and_then(Value::as_str) {
        Some("page") => Parent::Page(field(parent, "id")?),
        Some("node") => Parent::Node(field(parent, "id")?),
        _ => return Err(Unreadable("to.parent.type is page or node".into())),
    };
    Ok(Place {
        parent,
        index: field(to, "index")?,
    })
}

fn shell(value: &Value) -> Result<Shell, Unreadable> {
    Ok(Shell {
        meta: field(value, "meta")?,
        imports: field(value, "imports")?,
        themes: field(value, "themes")?,
        styles: field(value, "styles")?,
        fonts: field(value, "fonts")?,
        comments: field(value, "comments")?,
    })
}

/// One `ToolOp`.
pub fn op(op: &Value) -> Result<Op, Unreadable> {
    let kind = op
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| Unreadable("an op has a type".into()))?;
    Ok(match kind {
        "insertPage" => Op::InsertPage {
            index: field(op, "index")?,
            page: Box::new(field(op, "page")?),
        },
        "convertScript" => Op::ConvertScript {
            id: field(op, "id")?,
        },
        "makeSlot" => Op::MakeSlot {
            id: field(op, "id")?,
            suggested: field(op, "suggested")?,
        },
        "setComponent" => Op::SetComponent {
            id: field(op, "id")?,
            component: field::<Option<_>>(op, "component")?.map(Box::new),
        },
        "setVariables" => Op::SetVariables {
            set: field(op, "set")?,
            remove: field(op, "remove")?,
        },
        "setShell" => Op::SetShell {
            shell: Box::new(shell(
                op.get("shell")
                    .ok_or_else(|| Unreadable("missing shell".into()))?,
            )?),
        },
        "insert" => Op::Insert {
            to: place(op)?,
            node: Box::new(field(op, "node")?),
        },
        "update" => Op::Update {
            id: field(op, "id")?,
            patch: field(op, "patch")?,
        },
        "replace" => Op::Replace {
            id: field(op, "id")?,
            node: Box::new(field(op, "node")?),
        },
        "delete" => Op::Delete {
            id: field(op, "id")?,
        },
        "replaceInInstance" => Op::ReplaceInInstance {
            id: field(op, "id")?,
            path: field(op, "path")?,
            node: Box::new(field(op, "node")?),
        },
        "scale" => Op::Scale {
            id: field(op, "id")?,
            factor: field(op, "factor")?,
            anchor: field(op, "anchor")?,
        },
        "move" => Op::Move {
            id: field(op, "id")?,
            to: place(op)?,
        },
        "reorder" => Op::Reorder {
            id: field(op, "id")?,
            index: field(op, "index")?,
        },
        "detach" => Op::Detach {
            id: field(op, "id")?,
        },
        "updateComponent" => Op::UpdateComponent {
            id: field(op, "id")?,
            name: field(op, "name")?,
            props: field(op, "props")?,
            variants: field(op, "variants")?,
        },
        other => return Err(Unreadable(format!("unknown op type {other}"))),
    })
}

/// A whole `ToolTransaction`: a person's origin and at least one op. Only people edit through the editor's
/// engine; agents' and the file's changes arrive as sync updates.
pub fn transaction(request: &Value) -> Result<Transaction, Unreadable> {
    let origin: Origin = field(request, "origin")?;
    if !matches!(origin.actor, Actor::Person { .. }) {
        return Err(Unreadable(
            "the editor applies a person's changes only".into(),
        ));
    }
    let ops = request
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| Unreadable("missing ops".into()))?;
    if ops.is_empty() {
        return Err(Unreadable("a transaction has at least one op".into()));
    }
    Ok(Transaction::new(
        origin,
        ops.iter().map(op).collect::<Result<_, _>>()?,
    ))
}
