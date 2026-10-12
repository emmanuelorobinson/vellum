//! Components (ARCHITECTURE §4.1 and §4.4): a node tree with typed props and variants. An instance points at
//! a component and chooses its variant and prop values (`InstanceNode`). Overrides change nodes inside the
//! tree, addressed by id path from its root. Resolving all of this is Prompt 17; the file only says what
//! each part is.

use std::collections::BTreeMap;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use ts_rs::TS;

use crate::ids::NodeIdPath;
use crate::node::{NODE_PROPERTIES, Node};

/// A component, listed in the document's `components` by id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Component {
    /// The name shown in the components list.
    pub name: String,
    /// The props an instance can set, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub props: BTreeMap<String, PropDef>,
    /// The default node tree; its root is the component's own frame.
    pub root: Node,
    /// The variants. Each lists a value for every variant axis; the one whose values equal an instance's
    /// choice applies its overrides to the tree.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<Variant>,
}

/// A prop a component declares. The `type` tag says what kind of value it takes, and the default has that
/// type too. Which node a prop drives is not in the format yet (Prompt 17).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export)]
pub enum PropDef {
    /// A switch, on or off.
    Boolean(BooleanProp),
    /// A line of text.
    Text(TextProp),
    /// A number.
    Number(NumberProp),
}

/// A boolean prop and its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BooleanProp {
    /// The value when an instance sets none.
    pub default: bool,
}

/// A text prop and its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TextProp {
    /// The value when an instance sets none.
    pub default: String,
}

/// A number prop and its default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct NumberProp {
    /// The value when an instance sets none.
    pub default: f64,
}

/// One combination of variant axis values, and the overrides it applies to the component's tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Variant {
    /// The axis values that select this variant (`{"State": "hover", "Size": "large"}`).
    pub properties: BTreeMap<String, String>,
    /// The overrides applied to the tree when this variant is chosen, by id path from its root.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<NodeIdPath, Override>,
}

/// What an instance does to one node inside its component's tree. Written as a single key: `props`,
/// `replace` or `children`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Override {
    /// Sets properties of the node. It keeps its id, its type and its children.
    Props(NodePatch),
    /// Replaces the node with this one. Boxed because a node is much larger than the other two.
    Replace(Box<Node>),
    /// Replaces the node's children with these.
    Children(Vec<Node>),
}

/// Property values to set on one node: a JSON object whose keys are node properties (`fills`, `visible`,
/// `content` and so on). Every key must be a property some node has, and a patch cannot set the node's id
/// or its children (a children override does that). The values are checked when the patch is applied to a
/// node, because only then is its kind known: the result must read back as that node.
#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[schemars(with = "BTreeMap<String, Json>")]
#[ts(export, type = "Record<string, unknown>")]
pub struct NodePatch(BTreeMap<String, Json>);

// Written out instead of `#[serde(transparent)]`, because ts-rs cannot read that attribute and warns on
// every build (the ids and colours do the same).
impl Serialize for NodePatch {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl NodePatch {
    /// A patch that sets `properties`, if every key is a property a node can have.
    pub fn new(properties: BTreeMap<String, Json>) -> Result<Self, PatchError> {
        for key in properties.keys() {
            match key.as_str() {
                "id" => return Err(PatchError::Id),
                "children" => return Err(PatchError::Children),
                name if NODE_PROPERTIES.contains(&name) => {}
                name => return Err(PatchError::Unknown(name.to_owned())),
            }
        }
        Ok(Self(properties))
    }

    /// The properties this patch sets, by name.
    pub fn properties(&self) -> &BTreeMap<String, Json> {
        &self.0
    }
}

impl<'de> Deserialize<'de> for NodePatch {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let properties = BTreeMap::<String, Json>::deserialize(deserializer)?;
        Self::new(properties).map_err(serde::de::Error::custom)
    }
}

/// Why a property patch was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    /// The id is the node's own and never changes.
    Id,
    /// Children are replaced with a children override, not a props override.
    Children,
    /// No node has a property with this name.
    Unknown(String),
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id => f.write_str("a props override cannot set the id"),
            Self::Children => {
                f.write_str("a props override cannot set the children; use a children override")
            }
            Self::Unknown(name) => write!(f, "{name:?} is not a property any node has"),
        }
    }
}

impl std::error::Error for PatchError {}
