//! Arrangement (ADR-034): grouping, frames, flips, alignment and distribution as one batch. A wrapper takes its place
//! from its children's fixed positions and sizes, so layers can be grouped when they all sit at fixed positions or
//! all follow a layout, never both at once. Layers that follow a layout have no position to measure.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vellum_core::ops::{Alignment, Axis, Op};
use vellum_types::{Node, NodeId, Point};

use super::batch::{Batch, Label, ids_in_use, new_node};
use super::support::{parse_input, schema_of};
use super::write::{MAX_ITEMS, check_items};
use super::{Tool, ToolCtx, ToolError, Verb};

pub(super) fn register(tools: &mut Vec<Box<dyn Tool>>) {
    tools.push(Box::new(ArrangeNodes));
}

struct ArrangeNodes;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArrangeInput {
    /// The document. Defaults to the one opened last.
    document: Option<String>,
    /// Layers to wrap in a new group, or in a frame with `into: "frame"`. The wrapper goes where the frontmost of
    /// them was, and the layers keep where they are.
    group: Option<Vec<GroupItem>>,
    /// Groups to take apart: each child goes where its group was, and the group is removed.
    ungroup: Option<Vec<String>>,
    /// Layers to mirror about their own centre.
    flip: Option<Vec<FlipItem>>,
    /// Layers to line up at one edge or at their centre, within their collective bounds.
    align: Option<Vec<AlignItem>>,
    /// Three or more layers to space with equal gaps. The outermost ones stay where they are.
    distribute: Option<Vec<DistributeItem>>,
    /// Layers to scale about an anchor on their own box, with everything inside them.
    scale: Option<Vec<ScaleItem>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GroupItem {
    /// The layers to wrap. They must be siblings, all at fixed positions with fixed sizes or all in one layout.
    ids: Vec<String>,
    /// The wrapper's name. Left out, it is named after its kind.
    name: Option<String>,
    /// "group" (the default) or "frame".
    into: Option<Container>,
}

#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Container {
    /// A group: it has no fill and no size of its own.
    Group,
    /// A frame: it has a size and can have a fill.
    Frame,
}

#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Direction {
    /// Left to right: the x axis.
    Horizontal,
    /// Top to bottom: the y axis.
    Vertical,
}

impl Direction {
    fn axis(self) -> Axis {
        match self {
            Self::Horizontal => Axis::Horizontal,
            Self::Vertical => Axis::Vertical,
        }
    }
}

#[derive(Deserialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Edge {
    /// The left edge, or the top one on the vertical axis.
    Start,
    /// The centre.
    Center,
    /// The right edge, or the bottom one on the vertical axis.
    End,
}

impl Edge {
    fn alignment(self) -> Alignment {
        match self {
            Self::Start => Alignment::Start,
            Self::Center => Alignment::Center,
            Self::End => Alignment::End,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FlipItem {
    /// The layer.
    id: String,
    /// The axis to mirror across.
    axis: Direction,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AlignItem {
    /// The layers, siblings at fixed positions with fixed sizes.
    ids: Vec<String>,
    /// The axis: "horizontal" lines up left or right edges (or centres), "vertical" lines up tops or bottoms.
    axis: Direction,
    /// Which edge or the centre.
    alignment: Edge,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DistributeItem {
    /// Three or more layers, siblings at fixed positions with fixed sizes.
    ids: Vec<String>,
    /// The axis to space them along.
    axis: Direction,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ScaleItem {
    /// The layer.
    id: String,
    /// The factor, above 0: 2 doubles every length.
    factor: f64,
    /// The fixed point of the scale, each from 0 to 1 across the layer's box. Left out, it is the top left corner.
    anchor: Option<Anchor>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Anchor {
    /// Across the box: 0 at the left, 1 at the right.
    x: f64,
    /// Down the box: 0 at the top, 1 at the bottom.
    y: f64,
}

impl Tool for ArrangeNodes {
    fn name(&self) -> &'static str {
        "arrange_nodes"
    }
    fn version(&self) -> u32 {
        1
    }
    fn description(&self) -> &'static str {
        "Group or frame layers, ungroup, flip, scale, align or distribute them. Positions come from each layer's own \
         fixed position and size, so layers in a layout can be grouped only with each other. One change for the batch."
    }
    fn input_schema(&self) -> schemars::Schema {
        schema_of::<ArrangeInput>()
    }
    fn read_only(&self) -> bool {
        false
    }
    fn destructive(&self) -> bool {
        // Ungrouping keeps every layer; nothing is removed.
        false
    }
    fn verb(&self) -> Verb {
        Verb::Updated
    }
    fn run(&self, ctx: &mut ToolCtx<'_>, input: Value) -> Result<Value, ToolError> {
        let ArrangeInput {
            document,
            group,
            ungroup,
            flip,
            align,
            distribute,
            scale,
        } = parse_input(input)?;
        let (group, ungroup, flip, align, distribute, scale) = (
            group.unwrap_or_default(),
            ungroup.unwrap_or_default(),
            flip.unwrap_or_default(),
            align.unwrap_or_default(),
            distribute.unwrap_or_default(),
            scale.unwrap_or_default(),
        );
        let total =
            group.len() + ungroup.len() + flip.len() + align.len() + distribute.len() + scale.len();
        if total == 0 {
            return Err(ToolError::invalid(
                "group",
                "Give at least one arrangement: group, ungroup, flip, scale, align or distribute.",
            ));
        }
        if total > MAX_ITEMS {
            return Err(ToolError::invalid(
                "group",
                format!("At most {MAX_ITEMS} arrangements in one call; split the batch."),
            ));
        }
        let (path, doc) = ctx.document(document.as_deref())?;
        let mut ids = ctx.docs().tool_ids();
        let mut taken = ids_in_use(&doc);
        let mut batch = Batch::default();
        let mut created = Vec::new();

        for (i, item) in group.into_iter().enumerate() {
            let at = format!("group[{i}]");
            let members = node_ids(&item.ids, &at).map_err(|e| e.at_op(i))?;
            let layers = members
                .iter()
                .enumerate()
                .map(|(k, id)| {
                    vellum_core::ops::find(&doc, id).ok_or_else(|| {
                        ToolError::not_found(format!(
                            "There is no layer {:?} in this document.",
                            item.ids[k]
                        ))
                        .at_op(i)
                        .at_path(format!("{at}.ids[{k}]"))
                    })
                })
                .collect::<Result<Vec<&Node>, ToolError>>()?;
            let container = item.into.unwrap_or(Container::Group);
            let wrapper = wrapper_of(container, item.name, &layers, &at, i)?;
            let node =
                new_node(wrapper, &doc, &mut ids, &mut taken, &at).map_err(|e| e.at_op(i))?;
            created.push(node.id().as_str().to_owned());
            let op = match container {
                Container::Group => Op::Group {
                    ids: members,
                    group: Box::new(node),
                },
                Container::Frame => Op::WrapInFrame {
                    ids: members,
                    frame: Box::new(node),
                },
            };
            batch.push(
                op,
                Label::new(i, at.clone())
                    .rename("group", at.clone())
                    .rename("frame", at),
            );
        }
        for (i, id) in ungroup.iter().enumerate() {
            let at = format!("ungroup[{i}]");
            let id = node_id(id, &at).map_err(|e| e.at_op(i))?;
            batch.push(
                Op::Ungroup { id },
                Label::new(i, at.clone()).rename("id", at),
            );
        }
        for (i, item) in flip.into_iter().enumerate() {
            let at = format!("flip[{i}]");
            let id = node_id(&item.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            batch.push(
                Op::Flip {
                    id,
                    axis: item.axis.axis(),
                },
                Label::new(i, at),
            );
        }
        for (i, item) in align.into_iter().enumerate() {
            let at = format!("align[{i}]");
            let ids = node_ids(&item.ids, &at).map_err(|e| e.at_op(i))?;
            batch.push(
                Op::Align {
                    ids,
                    axis: item.axis.axis(),
                    alignment: item.alignment.alignment(),
                },
                Label::new(i, at),
            );
        }
        for (i, item) in distribute.into_iter().enumerate() {
            let at = format!("distribute[{i}]");
            let ids = node_ids(&item.ids, &at).map_err(|e| e.at_op(i))?;
            batch.push(
                Op::Distribute {
                    ids,
                    axis: item.axis.axis(),
                },
                Label::new(i, at),
            );
        }

        for (i, item) in scale.into_iter().enumerate() {
            let at = format!("scale[{i}]");
            let id = node_id(&item.id, &format!("{at}.id")).map_err(|e| e.at_op(i))?;
            let anchor = item.anchor.map_or_else(Point::default, |anchor| Point {
                x: anchor.x,
                y: anchor.y,
            });
            batch.push(
                Op::Scale {
                    id,
                    factor: item.factor,
                    anchor,
                },
                Label::new(i, at),
            );
        }

        ctx.commit(&path, batch)?;
        Ok(json!({"document": path, "created": created, "changed": total}))
    }
}

/// The wrapper node a group or frame is made from: named, placed at the bounds of its children when they have fixed
/// positions, and sized to them when it is a frame. Children that follow a layout get a wrapper that does too.
fn wrapper_of(
    container: Container,
    name: Option<String>,
    layers: &[&Node],
    at: &str,
    item: usize,
) -> Result<Value, ToolError> {
    let mut wrapper = json!({"type": match container {
        Container::Group => "group",
        Container::Frame => "frame",
    }});
    if let Some(name) = name {
        wrapper["name"] = json!(name);
    }
    let mut fixed: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut in_layout = 0;
    for layer in layers {
        match fixed_box(layer) {
            Ok(Some(place)) => fixed.push(place),
            Ok(None) => in_layout += 1,
            Err(reason) => {
                return Err(ToolError::invalid(
                    format!("{at}.ids"),
                    format!("{reason} Give it a fixed width and height first, then group it."),
                )
                .at_op(item));
            }
        }
    }
    if in_layout > 0 && !fixed.is_empty() {
        return Err(ToolError::invalid(
            format!("{at}.ids"),
            "Some of these layers follow a layout and some sit at fixed positions. Group one kind at a time.",
        )
        .at_op(item));
    }
    if fixed.is_empty() {
        wrapper["width"] = json!("hug");
        wrapper["height"] = json!("hug");
        return Ok(wrapper);
    }
    let left = fixed.iter().map(|b| b.0).fold(f64::INFINITY, f64::min);
    let top = fixed.iter().map(|b| b.1).fold(f64::INFINITY, f64::min);
    let right = fixed
        .iter()
        .map(|b| b.0 + b.2)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = fixed
        .iter()
        .map(|b| b.1 + b.3)
        .fold(f64::NEG_INFINITY, f64::max);
    wrapper["position"] = json!({"type": "absolute", "x": left, "y": top});
    // A group takes its size from its children; a frame is sized to them, as a fixed box.
    match container {
        Container::Group => {
            wrapper["width"] = json!("hug");
            wrapper["height"] = json!("hug");
        }
        Container::Frame => {
            wrapper["width"] = json!(right - left);
            wrapper["height"] = json!(bottom - top);
        }
    }
    Ok(wrapper)
}

/// Where a layer sits, as `(x, y, width, height)`, when it has a fixed position; `None` when it follows a layout.
/// An error when it sits at a fixed position but has no fixed size to measure.
fn fixed_box(layer: &Node) -> Result<Option<(f64, f64, f64, f64)>, String> {
    let json = serde_json::to_value(layer).unwrap_or(Value::Null);
    if json["position"]["type"] != "absolute" {
        return Ok(None);
    }
    match (
        json["position"]["x"].as_f64(),
        json["position"]["y"].as_f64(),
        json["width"].as_f64(),
        json["height"].as_f64(),
    ) {
        (Some(x), Some(y), Some(width), Some(height)) => Ok(Some((x, y, width, height))),
        _ => Err(format!(
            "{:?} has no fixed width and height, so its bounds cannot be measured.",
            layer.id().as_str()
        )),
    }
}

/// A layer id the agent gave, checked, at input path `at`.
fn node_id(id: &str, at: &str) -> Result<NodeId, ToolError> {
    NodeId::new(id).map_err(|e| ToolError::invalid(at, format!("{id:?} is not a layer id: {e}.")))
}

/// A list of layer ids, checked, at `at.ids`.
fn node_ids(ids: &[String], at: &str) -> Result<Vec<NodeId>, ToolError> {
    check_items(ids, &format!("{at}.ids"))?;
    ids.iter()
        .enumerate()
        .map(|(k, id)| node_id(id, &format!("{at}.ids[{k}]")))
        .collect()
}
