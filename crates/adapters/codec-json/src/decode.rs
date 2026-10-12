//! Reading a file: the version first, then migrations, then the document, and a failure explained by path,
//! line and column.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value as Json;
use vellum_types::{
    BooleanNode, ContextNode, Document, EllipseNode, EmbedNode, FrameNode, GroupNode, IconNode,
    ImageNode, InstanceNode, LineNode, NoteNode, PolygonNode, PromptNode, RectNode, ScriptNode,
    StarNode, TextNode, VectorNode,
};

use crate::error::CodecError;
use crate::locate::{Index, offset_of, position};
use crate::migrate::Migrations;
use crate::tree::{Step, render_path};

const BYTE_ORDER_MARK: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Just enough of a file to know its version, whatever else is in it.
#[derive(Deserialize)]
struct Peek {
    meta: Option<PeekMeta>,
}

#[derive(Deserialize)]
struct PeekMeta {
    // `null` is a value (a wrong one), not a missing field.
    #[serde(rename = "formatVersion", default, deserialize_with = "present")]
    format_version: Option<Json>,
}

fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Json>, D::Error> {
    Json::deserialize(deserializer).map(Some)
}

pub(crate) fn decode(migrations: &Migrations, bytes: &[u8]) -> Result<Document, CodecError> {
    // A byte order mark is whitespace to us; the canonical form has none. Positions count from after it.
    let bytes = bytes.strip_prefix(BYTE_ORDER_MARK).unwrap_or(bytes);

    let peek: Peek = serde_json::from_slice(bytes).map_err(|e| from_serde(bytes, &e))?;
    let index = Index::new(bytes);
    let here = |offset: usize| Some(position(bytes, offset));

    let Some(meta) = peek.meta else {
        return Err(CodecError::Invalid {
            path: String::new(),
            position: here(0),
            message: "missing field `meta`".to_owned(),
        });
    };
    let version_offset = || {
        index.as_ref().and_then(|i| {
            let value = i.deepest(&[
                Step::Key("meta".to_owned()),
                Step::Key("formatVersion".to_owned()),
            ]);
            here(value.start())
        })
    };
    let Some(version) = meta.format_version else {
        return Err(CodecError::Invalid {
            path: "meta".to_owned(),
            position: index
                .as_ref()
                .and_then(|i| here(i.deepest(&[Step::Key("meta".to_owned())]).start())),
            message: "missing field `formatVersion`".to_owned(),
        });
    };
    let Some(found) = version
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v >= 1)
    else {
        return Err(CodecError::Invalid {
            path: "meta.formatVersion".to_owned(),
            position: version_offset(),
            message: format!("the format version must be a whole number from 1, not {version}"),
        });
    };
    if found > migrations.current() {
        return Err(CodecError::NewerFormat {
            found,
            supported: migrations.current(),
        });
    }

    if let Some((path, offset, key)) = index.as_ref().and_then(Index::first_duplicate) {
        return Err(CodecError::Invalid {
            path: render_path(&path),
            position: here(offset),
            message: format!("the key `{key}` appears twice"),
        });
    }

    if found < migrations.current() {
        let mut json: Json = serde_json::from_slice(bytes).map_err(|e| from_serde(bytes, &e))?;
        migrations.upgrade(&mut json)?;
        return serde_path_to_error::deserialize(json).map_err(|e| {
            let mut path = steps_of(e.path());
            let message = bare(&e.inner().to_string());
            leave_out_unknown_field(&mut path, &message);
            CodecError::Invalid {
                path: render_path(&path),
                position: None,
                message,
            }
        });
    }

    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    match serde_path_to_error::deserialize::<_, Document>(&mut deserializer) {
        Ok(document) => Ok(document),
        Err(error) => Err(explain(
            bytes,
            index.as_ref(),
            &steps_of(error.path()),
            &error.inner().to_string(),
        )),
    }
}

/// A serde_json failure to read the text at all, or to see its shape.
fn from_serde(bytes: &[u8], error: &serde_json::Error) -> CodecError {
    let at = position(bytes, offset_of(bytes, error.line().max(1), error.column()));
    let message = bare(&error.to_string());
    if error.is_data() {
        CodecError::Invalid {
            path: String::new(),
            position: Some(at),
            message,
        }
    } else {
        CodecError::Syntax {
            position: at,
            message,
        }
    }
}

/// serde_json ends its messages with " at line L column C"; the line and column are reported separately.
fn bare(message: &str) -> String {
    match message.rfind(" at line ") {
        Some(at) => message[..at].to_owned(),
        None => message.to_owned(),
    }
}

fn steps_of(path: &serde_path_to_error::Path) -> Vec<Step> {
    use serde_path_to_error::Segment;
    path.iter()
        .filter_map(|segment| match segment {
            Segment::Seq { index } => Some(Step::Index(*index)),
            Segment::Map { key } => Some(Step::Key(key.clone())),
            Segment::Enum { variant } => Some(Step::Key(variant.clone())),
            Segment::Unknown => None,
        })
        .collect()
}

/// Whether the value at `path` is a node: a child, a component's root, a replacement or slot content.
fn is_node_position(path: &[Step]) -> bool {
    match path {
        [.., Step::Key(parent), Step::Index(_)] if parent == "children" => true,
        [.., Step::Key(slots), Step::Key(_), Step::Index(_)] if slots == "slotContent" => true,
        [.., Step::Key(last)] => last == "root" || last == "replace",
        _ => false,
    }
}

fn check<T: DeserializeOwned>(value: Json) -> Option<(Vec<Step>, String)> {
    serde_path_to_error::deserialize::<_, T>(value)
        .err()
        .map(|e| (steps_of(e.path()), bare(&e.inner().to_string())))
}

/// A node is a tagged enum, so serde reads it through a buffer and reports a fault inside it at the node's
/// own path. Reading the node again as its own kind finds the field.
fn refine_node(mut node: Json) -> Option<(Vec<Step>, String)> {
    let kind = node.as_object_mut()?.remove("type")?;
    let value = node;
    match kind.as_str()? {
        "frame" => check::<FrameNode>(value),
        "group" => check::<GroupNode>(value),
        "rect" => check::<RectNode>(value),
        "ellipse" => check::<EllipseNode>(value),
        "polygon" => check::<PolygonNode>(value),
        "star" => check::<StarNode>(value),
        "line" => check::<LineNode>(value),
        "vector" => check::<VectorNode>(value),
        "boolean" => check::<BooleanNode>(value),
        "text" => check::<TextNode>(value),
        "image" => check::<ImageNode>(value),
        "icon" => check::<IconNode>(value),
        "instance" => check::<InstanceNode>(value),
        "script" => check::<ScriptNode>(value),
        "note" => check::<NoteNode>(value),
        "context" => check::<ContextNode>(value),
        "prompt" => check::<PromptNode>(value),
        "embed" => check::<EmbedNode>(value),
        _ => None,
    }
}

fn json_at<'a>(root: &'a Json, path: &[Step]) -> Option<&'a Json> {
    let mut here = root;
    for step in path {
        here = match step {
            Step::Key(key) => here.get(key.as_str())?,
            Step::Index(i) => here.get(*i)?,
        };
    }
    Some(here)
}

/// The names in "expected one of `a`, `b`" and "expected `a`".
fn expected_names(message: &str) -> Vec<String> {
    let Some((_, list)) = message.split_once("expected") else {
        return Vec::new();
    };
    list.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

fn unknown_field(message: &str) -> Option<&str> {
    message.strip_prefix("unknown field `")?.split('`').next()
}

/// serde_path_to_error ends the path at an unknown field itself; the fault is the object that holds it.
fn leave_out_unknown_field(path: &mut Vec<Step>, message: &str) {
    if let (Some(field), Some(Step::Key(last))) = (unknown_field(message), path.last())
        && last == field
    {
        path.pop();
    }
}

fn explain(bytes: &[u8], index: Option<&Index<'_>>, outer: &[Step], message: &str) -> CodecError {
    let mut path = outer.to_vec();
    let mut message = bare(message);

    leave_out_unknown_field(&mut path, &message);

    if let Ok(root) = serde_json::from_slice::<Json>(bytes) {
        for _ in 0..64 {
            if !is_node_position(&path) {
                break;
            }
            let Some((inner, inner_message)) = json_at(&root, &path).cloned().and_then(refine_node)
            else {
                break;
            };
            path.extend(inner);
            message = inner_message;
            leave_out_unknown_field(&mut path, &message);
        }
    }

    let offset = index.map(|index| {
        let target = index.deepest(&path);
        if let Some(field) = unknown_field(&message)
            && let Some(at) = index.find_unknown(target, field, &expected_names(&message))
        {
            return at;
        }
        if message.starts_with("unknown variant")
            && let Some(at) = index.member_value_start(target, "type")
        {
            return at;
        }
        target.start()
    });

    CodecError::Invalid {
        path: render_path(&path),
        position: offset.map(|offset| position(bytes, offset)),
        message,
    }
}
