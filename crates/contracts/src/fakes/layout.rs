use std::collections::BTreeMap;

use vellum_core::ports::{
    Dimension, DirtySet, Extent, LayoutContent, LayoutEngine, LayoutNode, LayoutResult, Rect,
    TextEngine,
};
use vellum_types::{Align, Justify, Layout, NodeId};

/// Lays out rows and columns with fixed, hug and fill sizes, gap, padding, justify and align, by the rules of
/// ADR-002 that the contract suite checks. Always lays out everything and ignores the dirty set. A grid is laid
/// out as a column. Costs more than it should (a child may be placed twice), which is fine for small trees.
#[derive(Debug, Default, Clone, Copy)]
pub struct StackLayout;

impl StackLayout {
    /// The engine.
    pub fn new() -> Self {
        Self
    }
}

type Boxes = BTreeMap<NodeId, Rect>;

impl LayoutEngine for StackLayout {
    fn layout(
        &mut self,
        tree: &vellum_core::ports::LayoutTree,
        _dirty: &DirtySet,
        text: &dyn TextEngine,
    ) -> LayoutResult {
        let mut boxes = Boxes::new();
        let root = &tree.root;
        let (w, h) = place(root, None, None, text, &mut boxes);
        boxes.insert(root.id.clone(), Rect::new(root.style.x, root.style.y, w, h));
        LayoutResult { boxes }
    }
}

/// The size a dimension asks for when it does not depend on the content.
fn explicit(dim: Dimension, forced: Option<f64>) -> Option<f64> {
    match dim {
        Dimension::Fixed(v) => Some(v),
        Dimension::Fill { fallback } => forced.or(fallback),
        Dimension::Hug { .. } => None,
    }
}

fn hug_fallback(dim: Dimension) -> f64 {
    match dim {
        Dimension::Hug { fallback } | Dimension::Fill { fallback } => fallback.unwrap_or(0.0),
        Dimension::Fixed(v) => v,
    }
}

/// Sizes `node` (given sizes its parent forces on fill dimensions), records the boxes of everything under it
/// relative to their parents, and returns the node's own size.
fn place(
    node: &LayoutNode,
    forced_w: Option<f64>,
    forced_h: Option<f64>,
    text: &dyn TextEngine,
    out: &mut Boxes,
) -> (f64, f64) {
    let style = &node.style;
    let ow = explicit(style.width, forced_w);
    let oh = explicit(style.height, forced_h);
    if let LayoutContent::Text(rich) = &node.content {
        let w = ow.unwrap_or_else(|| text.measure(rich, Extent::UNBOUNDED).width);
        let h = oh.unwrap_or_else(|| text.measure(rich, Extent::with_width(w)).height);
        return (w, h);
    }
    match style.mode {
        Layout::None => place_free(node, ow, oh, text, out),
        Layout::Row => place_stack(node, true, ow, oh, text, out),
        Layout::Column | Layout::Grid => place_stack(node, false, ow, oh, text, out),
    }
}

fn place_free(
    node: &LayoutNode,
    ow: Option<f64>,
    oh: Option<f64>,
    text: &dyn TextEngine,
    out: &mut Boxes,
) -> (f64, f64) {
    let (mut right, mut bottom) = (0.0f64, 0.0f64);
    for child in &node.children {
        // Nothing to fill here (ADR-002 row 7): a fill child uses its fallback.
        let (w, h) = place(child, None, None, text, out);
        out.insert(
            child.id.clone(),
            Rect::new(child.style.x, child.style.y, w, h),
        );
        right = right.max(child.style.x + w);
        bottom = bottom.max(child.style.y + h);
    }
    let empty = node.children.is_empty();
    (
        ow.unwrap_or(if empty {
            hug_fallback(node.style.width)
        } else {
            right
        }),
        oh.unwrap_or(if empty {
            hug_fallback(node.style.height)
        } else {
            bottom
        }),
    )
}

fn place_stack(
    node: &LayoutNode,
    horizontal: bool,
    ow: Option<f64>,
    oh: Option<f64>,
    text: &dyn TextEngine,
    out: &mut Boxes,
) -> (f64, f64) {
    let style = &node.style;
    let pad = &style.padding;
    let (pad_main, pad_cross, pad_start_main, pad_start_cross) = if horizontal {
        (
            pad.left + pad.right,
            pad.top + pad.bottom,
            pad.left,
            pad.top,
        )
    } else {
        (
            pad.top + pad.bottom,
            pad.left + pad.right,
            pad.top,
            pad.left,
        )
    };
    let (own_main, own_cross) = if horizontal { (ow, oh) } else { (oh, ow) };
    let inner_main = own_main.map(|m| (m - pad_main).max(0.0));
    let inner_cross = own_cross.map(|c| (c - pad_cross).max(0.0));
    let n = node.children.len();
    let gaps = style.gap * n.saturating_sub(1) as f64;

    let dims = |c: &LayoutNode| {
        if horizontal {
            (c.style.width, c.style.height)
        } else {
            (c.style.height, c.style.width)
        }
    };
    let call =
        |c: &LayoutNode, main: Option<f64>, cross: Option<f64>, out: &mut Boxes| -> (f64, f64) {
            let (w, h) = if horizontal {
                place(c, main, cross, text, out)
            } else {
                place(c, cross, main, text, out)
            };
            if horizontal { (w, h) } else { (h, w) }
        };

    // Children that do not fill the main axis (or cannot, in a parent that hugs it) first.
    let fills_main =
        |c: &LayoutNode| inner_main.is_some() && matches!(dims(c).0, Dimension::Fill { .. });
    let mut sizes = vec![(0.0, 0.0); n];
    let mut forced_main: Vec<Option<f64>> = vec![None; n];
    for (i, c) in node.children.iter().enumerate() {
        if !fills_main(c) {
            let cross = matches!(dims(c).1, Dimension::Fill { .. })
                .then_some(inner_cross)
                .flatten();
            sizes[i] = call(c, None, cross, out);
        }
    }
    let fixed_total: f64 = node
        .children
        .iter()
        .enumerate()
        .filter(|(_, c)| !fills_main(c))
        .map(|(i, _)| sizes[i].0)
        .sum();
    let fill_count = node.children.iter().filter(|c| fills_main(c)).count();
    if let (Some(inner), true) = (inner_main, fill_count > 0) {
        let share = ((inner - gaps - fixed_total) / fill_count as f64).max(0.0);
        for (i, c) in node.children.iter().enumerate() {
            if fills_main(c) {
                let cross = matches!(dims(c).1, Dimension::Fill { .. })
                    .then_some(inner_cross)
                    .flatten();
                forced_main[i] = Some(share);
                sizes[i] = call(c, Some(share), cross, out);
            }
        }
    }

    // A parent that hugs the cross axis is as wide as its widest child; fill children stretch to that.
    let content_cross = sizes.iter().map(|s| s.1).fold(0.0, f64::max);
    let cross_total = own_cross.unwrap_or(if n == 0 {
        hug_fallback(if horizontal {
            style.height
        } else {
            style.width
        })
    } else {
        content_cross + pad_cross
    });
    if inner_cross.is_none() {
        for (i, c) in node.children.iter().enumerate() {
            if matches!(dims(c).1, Dimension::Fill { .. }) {
                sizes[i] = call(
                    c,
                    forced_main[i]
                        .or(Some(sizes[i].0))
                        .filter(|_| fills_main(c)),
                    Some(content_cross),
                    out,
                );
            }
        }
    }
    let main_content: f64 = sizes.iter().map(|s| s.0).sum::<f64>() + gaps;
    let main_total = own_main.unwrap_or(if n == 0 {
        hug_fallback(if horizontal {
            style.width
        } else {
            style.height
        })
    } else {
        main_content + pad_main
    });

    let inner_main_final = (main_total - pad_main).max(0.0);
    let inner_cross_final = (cross_total - pad_cross).max(0.0);
    let free = (inner_main_final - main_content).max(0.0);
    let (mut offset, extra) = match style.justify {
        Justify::Start => (0.0, 0.0),
        Justify::Center => (free / 2.0, 0.0),
        Justify::End => (free, 0.0),
        Justify::SpaceBetween if n > 1 => (0.0, free / (n - 1) as f64),
        Justify::SpaceAround if n > 0 => (free / n as f64 / 2.0, free / n as f64),
        Justify::SpaceBetween | Justify::SpaceAround => (0.0, 0.0),
    };
    offset += pad_start_main;
    for (c, (main, cross)) in node.children.iter().zip(&sizes) {
        let across = pad_start_cross
            + match style.align {
                Align::Start => 0.0,
                Align::Center => (inner_cross_final - cross) / 2.0,
                Align::End => inner_cross_final - cross,
            };
        let rect = if horizontal {
            Rect::new(offset, across, *main, *cross)
        } else {
            Rect::new(across, offset, *cross, *main)
        };
        out.insert(c.id.clone(), rect);
        offset += main + style.gap + extra;
    }

    if horizontal {
        (main_total, cross_total)
    } else {
        (cross_total, main_total)
    }
}
