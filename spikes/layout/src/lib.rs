//! Spike C (guide Prompt 6): maps the layout oracle's Vellum props onto Taffy and computes the boxes.
//!
//! The mapping mirrors the CSS mapping in `scripts/lib/layout-oracle.mjs` (ADR-002) one property at a
//! time, including the semantic resolution of hug and fill (where they fall back), so a difference from
//! Chrome is Taffy's, not the mapping's. Spike quality: props are read from JSON, not `vellum-types`
//! (Prompt 7), and errors panic. Prompt 18 rewrites this as `crates/adapters/layout-taffy`.

use std::collections::BTreeMap;

use serde_json::Value;
use taffy::prelude::*;
use taffy::{MaxTrackSizingFunction, MinTrackSizingFunction, Overflow, Point};
use vellum_testkit::layout_oracle::{LayoutBox, OracleFixture, OracleNode};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Sizing {
    Fixed(f32),
    Hug,
    Fill,
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    Width,
    Height,
}

impl Axis {
    fn key(self) -> &'static str {
        match self {
            Axis::Width => "width",
            Axis::Height => "height",
        }
    }
}

/// What a child needs to know about its parent to resolve and map itself.
#[derive(Clone, Copy)]
struct ParentInfo<'a> {
    layout: &'a str,
    width: Sizing,
    height: Sizing,
}

fn str_prop<'a>(node: &'a OracleNode, key: &str) -> Option<&'a str> {
    node.props.get(key).and_then(Value::as_str)
}

fn num_prop(node: &OracleNode, key: &str) -> Option<f32> {
    node.props
        .get(key)
        .and_then(Value::as_f64)
        .map(|v| v as f32)
}

fn layout_of(node: &OracleNode) -> &str {
    str_prop(node, "layout").unwrap_or("none")
}

fn is_flex(layout: &str) -> bool {
    layout == "row" || layout == "column"
}

fn main_axis(layout: &str) -> Axis {
    if layout == "row" {
        Axis::Width
    } else {
        Axis::Height
    }
}

/// The same rules as `resolveTree` in scripts/lib/layout-oracle.mjs.
fn resolve(node: &OracleNode, parent: Option<ParentInfo>, positioned: bool, axis: Axis) -> Sizing {
    let fallback = num_prop(node, &format!("{}Fallback", axis.key()));
    let hug = || {
        if layout_of(node) != "none" {
            Sizing::Hug
        } else {
            Sizing::Fixed(fallback.expect("hug without auto-layout needs a fallback"))
        }
    };
    match node.props.get(axis.key()) {
        Some(Value::Number(n)) => Sizing::Fixed(n.as_f64().unwrap() as f32),
        Some(Value::String(s)) if s == "hug" => hug(),
        Some(Value::String(s)) if s == "fill" => match parent {
            None => Sizing::Fixed(fallback.expect("fill needs a fallback")),
            Some(_) if positioned => Sizing::Fixed(fallback.expect("fill needs a fallback")),
            Some(p) if p.layout == "none" => {
                Sizing::Fixed(fallback.expect("fill needs a fallback"))
            }
            Some(p) => {
                let parent_axis = if axis == Axis::Width {
                    p.width
                } else {
                    p.height
                };
                if is_flex(p.layout) && main_axis(p.layout) == axis && parent_axis == Sizing::Hug {
                    fallback.map(Sizing::Fixed).unwrap_or_else(hug)
                } else {
                    Sizing::Fill
                }
            }
        },
        other => panic!("bad {} {other:?}", axis.key()),
    }
}

fn padding(node: &OracleNode) -> Rect<LengthPercentage> {
    let v: Vec<f32> = node
        .props
        .get("padding")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|n| n.as_f64().unwrap() as f32).collect())
        .unwrap_or_else(|| vec![0.0]);
    let (t, r, b, l) = match v.as_slice() {
        [a] => (*a, *a, *a, *a),
        [y, x] => (*y, *x, *y, *x),
        [t, r, b, l] => (*t, *r, *b, *l),
        _ => panic!("padding takes 1, 2 or 4 values"),
    };
    Rect {
        left: length(l),
        right: length(r),
        top: length(t),
        bottom: length(b),
    }
}

fn equal_tracks(count: u16) -> Vec<GridTemplateComponent<String>> {
    vec![repeat(
        count,
        vec![minmax(
            MinTrackSizingFunction::length(0.0),
            MaxTrackSizingFunction::fr(1.0),
        )],
    )]
}

fn build(
    tree: &mut TaffyTree<()>,
    node: &OracleNode,
    parent: Option<ParentInfo>,
    ids: &mut Vec<(String, NodeId)>,
) -> NodeId {
    let layout = layout_of(node);
    let absolute = str_prop(node, "position") == Some("absolute");
    let positioned = absolute || parent.is_some_and(|p| p.layout == "none");
    let width = resolve(node, parent, positioned, Axis::Width);
    let height = resolve(node, parent, positioned, Axis::Height);

    let mut style = Style::<String> {
        box_sizing: BoxSizing::BorderBox,
        padding: padding(node),
        ..Style::default()
    };
    style.display = if node.props.get("visible") == Some(&Value::Bool(false)) {
        Display::None
    } else {
        match layout {
            "none" => Display::Block,
            "row" | "column" => Display::Flex,
            "grid" => Display::Grid,
            other => panic!("bad layout {other}"),
        }
    };
    let gap = num_prop(node, "gap").unwrap_or(0.0);
    if is_flex(layout) {
        style.flex_direction = if layout == "row" {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        };
        style.flex_wrap = FlexWrap::NoWrap;
        style.justify_content = Some(match str_prop(node, "justify").unwrap_or("start") {
            "start" => JustifyContent::FLEX_START,
            "center" => JustifyContent::CENTER,
            "end" => JustifyContent::FLEX_END,
            "space-between" => JustifyContent::SPACE_BETWEEN,
            "space-around" => JustifyContent::SPACE_AROUND,
            other => panic!("bad justify {other}"),
        });
        style.align_items = Some(match str_prop(node, "align").unwrap_or("start") {
            "start" => AlignItems::FLEX_START,
            "center" => AlignItems::CENTER,
            "end" => AlignItems::FLEX_END,
            other => panic!("bad align {other}"),
        });
        style.gap = Size {
            width: length(gap),
            height: length(gap),
        };
    } else if layout == "grid" {
        let columns = node.props["columns"].as_u64().unwrap() as u16;
        style.grid_template_columns = equal_tracks(columns);
        match node.props.get("rows").and_then(Value::as_u64) {
            Some(rows) => style.grid_template_rows = equal_tracks(rows as u16),
            None => style.grid_auto_rows = vec![auto()],
        }
        let cell = |key| match str_prop(node, key).unwrap_or("start") {
            "start" => AlignItems::START,
            "center" => AlignItems::CENTER,
            "end" => AlignItems::END,
            other => panic!("bad grid {key} {other}"),
        };
        style.justify_items = Some(cell("justify"));
        style.align_items = Some(cell("align"));
        style.gap = Size {
            width: length(gap),
            height: length(gap),
        };
    }
    if node.props.get("clip") == Some(&Value::Bool(true)) {
        style.overflow = Point {
            x: Overflow::Hidden,
            y: Overflow::Hidden,
        };
        style.scrollbar_width = 0.0;
    }
    if positioned {
        style.position = Position::Absolute;
        style.inset = Rect {
            left: length(num_prop(node, "x").unwrap_or(0.0)),
            top: length(num_prop(node, "y").unwrap_or(0.0)),
            right: auto(),
            bottom: auto(),
        };
    }

    let in_flex = !positioned && parent.is_some_and(|p| is_flex(p.layout));
    let in_grid = !positioned && parent.is_some_and(|p| p.layout == "grid");
    for (axis, sizing) in [(Axis::Width, width), (Axis::Height, height)] {
        let main = parent.is_some_and(|p| main_axis(p.layout) == axis);
        let dim = match sizing {
            Sizing::Fixed(v) => length(v),
            // Taffy has no fit-content size; auto is content-sized in flex and grid items that
            // do not stretch, and the alignment defaults above keep them from stretching.
            Sizing::Hug | Sizing::Fill => auto(),
        };
        let min: Option<LengthPercentageAuto> = match sizing {
            Sizing::Fill if (in_flex && main) || in_grid => Some(length(0.0)),
            _ => None,
        };
        match axis {
            Axis::Width => {
                style.size.width = dim;
                if let Some(m) = min {
                    style.min_size.width = m;
                }
            }
            Axis::Height => {
                style.size.height = dim;
                if let Some(m) = min {
                    style.min_size.height = m;
                }
            }
        }
        if sizing == Sizing::Fill {
            if in_flex && main {
                style.flex_grow = 1.0;
                style.flex_shrink = 1.0;
                style.flex_basis = length(0.0);
            } else if in_flex {
                style.align_self = Some(AlignSelf::STRETCH);
            } else if in_grid {
                match axis {
                    Axis::Width => style.justify_self = Some(AlignSelf::STRETCH),
                    Axis::Height => style.align_self = Some(AlignSelf::STRETCH),
                }
            }
        } else if in_flex && main {
            style.flex_shrink = 0.0;
        }
    }

    let info = ParentInfo {
        layout,
        width,
        height,
    };
    let children: Vec<NodeId> = node
        .children
        .iter()
        .map(|c| build(tree, c, Some(info), ids))
        .collect();
    let id = tree.new_with_children(style, &children).unwrap();
    ids.push((node.id.clone(), id));
    id
}

/// Lays the fixture out with Taffy (max-content space, as on the canvas, no rounding) and returns each
/// node's border box relative to its parent's, keyed by oracle node id.
pub fn compute(fixture: &OracleFixture) -> BTreeMap<String, LayoutBox> {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let mut ids = Vec::new();
    let root = build(&mut tree, &fixture.root, None, &mut ids);
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        },
    )
    .unwrap();
    ids.into_iter()
        .map(|(oracle_id, id)| {
            let l = tree.layout(id).unwrap();
            let b = LayoutBox::new(
                l.location.x as f64,
                l.location.y as f64,
                l.size.width as f64,
                l.size.height as f64,
            );
            (oracle_id, b)
        })
        .collect()
}
