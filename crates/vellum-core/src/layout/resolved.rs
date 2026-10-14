use super::LayoutError;
use crate::{ports::*, resolver::ResolvedNode};
use vellum_types::{Node, Padding, Position, Size, StrokeAlign, StrokeWidth, Value};
/// Build one top-level layout tree, retaining every resolved occurrence address. Never changes a document.
pub fn from_resolved(root: &ResolvedNode) -> Result<LayoutTree, LayoutError> {
    Ok(LayoutTree { root: node(root)? })
}
fn node(r: &ResolvedNode) -> Result<LayoutNode, LayoutError> {
    // Resolver diagnostics unrelated to geometry (including ignored theme choices) stay on the
    // resolver output. Missing component structure is fatal here; numeric references are checked below.
    if r.problems
        .iter()
        .any(|problem| matches!(problem, crate::resolver::components::Problem::Component(_)))
    {
        return Err(LayoutError::UnresolvedValue {
            node: r.address.clone(),
        });
    }
    let own = &r.value.node;
    let number = |v: &Value<f64>| match v {
        Value::Literal(n) => Ok(*n),
        Value::Var(_) => Err(LayoutError::UnresolvedValue {
            node: r.address.clone(),
        }),
    };
    let dim = |s: &Size| {
        Ok(match s {
            Size::Fixed(v) => Dimension::Fixed(number(v)?),
            Size::Hug { fallback } => Dimension::Hug {
                fallback: *fallback,
            },
            Size::Fill { fallback } => Dimension::Fill {
                fallback: *fallback,
            },
        })
    };
    let sides = |v: [&Value<f64>; 4]| -> Result<Insets, LayoutError> {
        Ok(Insets {
            top: number(v[0])?,
            right: number(v[1])?,
            bottom: number(v[2])?,
            left: number(v[3])?,
        })
    };
    let mut style = LayoutStyle {
        width: dim(own.width())?,
        height: dim(own.height())?,
        visible: r.value.visible,
        ..Default::default()
    };
    if let Position::Absolute(p) = own.position() {
        style.absolute = true;
        style.x = p.x;
        style.y = p.y;
    }
    if let Node::Frame(f) = own {
        style.mode = f.layout;
        style.columns = f.columns;
        style.rows = f.rows;
        style.gap = number(&f.gap)?;
        style.padding = match &f.padding {
            Padding::All(v) => sides([v; 4])?,
            Padding::Axes([y, x]) => sides([y, x, y, x])?,
            Padding::Sides([t, r, b, l]) => sides([t, r, b, l])?,
        };
        style.justify = f.justify;
        style.align = f.align;
        style.clip = f.clip;
        style.layout_includes_stroke = f.layout_includes_stroke;
    }
    if let Some(s) = own.stroke() {
        let factor = match s.align {
            StrokeAlign::Inside => 1.0,
            StrokeAlign::Center => 0.5,
            StrokeAlign::Outside => 0.0,
        };
        let v = match &s.width {
            StrokeWidth::Uniform(v) => sides([v; 4])?,
            StrokeWidth::PerSide([t, r, b, l]) => sides([t, r, b, l])?,
        };
        style.stroke = Insets {
            top: v.top * factor,
            right: v.right * factor,
            bottom: v.bottom * factor,
            left: v.left * factor,
        };
    }
    let content = if let Node::Text(t) = own {
        LayoutContent::Text(RichText {
            align: t.align,
            runs: t
                .content
                .iter()
                .map(|run| {
                    Ok(StyledRun {
                        text: run.text.clone(),
                        family: run.font.clone(),
                        weight: run.weight,
                        style: run.style,
                        size: number(&run.size)?,
                        line_height: run.line_height.as_ref().map(number).transpose()?,
                        letter_spacing: number(&run.letter_spacing)?,
                    })
                })
                .collect::<Result<_, LayoutError>>()?,
        })
    } else {
        LayoutContent::Frame
    };
    Ok(LayoutNode {
        id: r.address.clone(),
        style,
        content,
        children: r
            .children
            .iter()
            .map(|c| node(c))
            .collect::<Result<_, _>>()?,
    })
}
