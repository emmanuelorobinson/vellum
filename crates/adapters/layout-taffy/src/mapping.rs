//! Each prepared Vellum property mapped to its Taffy equivalent. No document or variable resolution.
use taffy::{MaxTrackSizingFunction, MinTrackSizingFunction, prelude::*};
use vellum_core::layout::{PreparedNode, Sizing};
use vellum_types::{Align, Justify, Layout};
fn equal_tracks(count: u32) -> Vec<GridTemplateComponent<String>> {
    vec![repeat(
        count as u16,
        vec![minmax(
            MinTrackSizingFunction::length(0.0),
            MaxTrackSizingFunction::fr(1.0),
        )],
    )]
}
fn cell(align: Align) -> AlignItems {
    match align {
        Align::Start => AlignItems::START,
        Align::Center => AlignItems::CENTER,
        Align::End => AlignItems::END,
    }
}
pub fn style(n: &PreparedNode, parent: Option<&PreparedNode>) -> Style {
    let s = &n.style;
    let border = if s.layout_includes_stroke {
        s.stroke
    } else {
        Default::default()
    };
    let mut out = Style {
        box_sizing: BoxSizing::BorderBox,
        padding: Rect {
            top: length(s.padding.top as f32),
            right: length(s.padding.right as f32),
            bottom: length(s.padding.bottom as f32),
            left: length(s.padding.left as f32),
        },
        border: Rect {
            top: length(border.top as f32),
            right: length(border.right as f32),
            bottom: length(border.bottom as f32),
            left: length(border.left as f32),
        },
        ..Default::default()
    };
    out.display = if !s.visible {
        Display::None
    } else {
        match s.mode {
            Layout::None => Display::Block,
            Layout::Row | Layout::Column => Display::Flex,
            Layout::Grid => Display::Grid,
        }
    };
    if matches!(s.mode, Layout::Row | Layout::Column) {
        out.flex_direction = if s.mode == Layout::Row {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        };
        out.justify_content = Some(match s.justify {
            Justify::Start => JustifyContent::FLEX_START,
            Justify::Center => JustifyContent::CENTER,
            Justify::End => JustifyContent::FLEX_END,
            Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
            Justify::SpaceAround => JustifyContent::SPACE_AROUND,
        });
        out.align_items = Some(match s.align {
            Align::Start => AlignItems::FLEX_START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::FLEX_END,
        });
    } else if s.mode == Layout::Grid {
        out.grid_template_columns = equal_tracks(s.columns.expect("core validated columns"));
        if let Some(r) = s.rows {
            out.grid_template_rows = equal_tracks(r);
        } else {
            out.grid_auto_rows = vec![auto()];
        }
        out.align_items = Some(cell(s.align));
        out.justify_items = Some(match s.justify {
            Justify::Start => AlignItems::START,
            Justify::Center => AlignItems::CENTER,
            Justify::End => AlignItems::END,
            _ => unreachable!("core validates grid justification"),
        });
    }
    if s.mode != Layout::None {
        out.gap = Size {
            width: length(s.gap as f32),
            height: length(s.gap as f32),
        };
    }
    // Clipping is drawing only. The fill min sizes below already express overflow without changing layout.
    if n.positioned {
        let b = parent
            .filter(|p| p.style.layout_includes_stroke)
            .map(|p| p.style.stroke)
            .unwrap_or_default();
        out.position = Position::Absolute;
        out.inset = Rect {
            left: length((s.x - b.left) as f32),
            top: length((s.y - b.top) as f32),
            right: auto(),
            bottom: auto(),
        };
    }
    let flex = !n.positioned
        && parent.is_some_and(|p| matches!(p.style.mode, Layout::Row | Layout::Column));
    let grid = !n.positioned && parent.is_some_and(|p| p.style.mode == Layout::Grid);
    for (horizontal, dim) in [(true, n.width), (false, n.height)] {
        let main = parent.is_some_and(|p| (p.style.mode == Layout::Row) == horizontal);
        let size = match dim {
            Sizing::Fixed(n) => length(n as f32),
            _ => auto(),
        };
        if horizontal {
            out.size.width = size;
        } else {
            out.size.height = size;
        }
        if dim == Sizing::Fill {
            if (flex && main) || grid {
                if horizontal {
                    out.min_size.width = length(0.0);
                } else {
                    out.min_size.height = length(0.0);
                }
            }
            if flex && main {
                out.flex_grow = 1.0;
                out.flex_shrink = 1.0;
                out.flex_basis = length(0.0);
            } else if flex {
                out.align_self = Some(AlignSelf::STRETCH);
            } else if grid {
                if horizontal {
                    out.justify_self = Some(AlignSelf::STRETCH);
                } else {
                    out.align_self = Some(AlignSelf::STRETCH);
                }
            }
        } else if flex && main {
            out.flex_shrink = 0.0;
        }
    }
    out
}
