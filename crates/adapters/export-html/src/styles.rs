use vellum_core::{
    ports::{ExportError, Rect},
    resolver::ResolvedNode,
};
use vellum_types::{CornerRadius, Node, Paint, TextRun, Value};
type Styles = Vec<(String, String)>;
pub fn px(n: f64) -> String {
    format!("{n}px")
}
fn number(v: &Value<f64>) -> Result<f64, ExportError> {
    v.literal()
        .copied()
        .filter(|v| v.is_finite())
        .ok_or_else(|| ExportError::new("Unresolved HTML value"))
}
fn color(n: &Node) -> Result<String, ExportError> {
    match n.fills().iter().find(|p| p.visible()) {
        None => Ok("transparent".into()),
        Some(Paint::Solid(p)) => p
            .color
            .literal()
            .map(ToString::to_string)
            .ok_or_else(|| ExportError::new("Unresolved color")),
        _ => Err(ExportError::new("Appearance requires an SVG asset")),
    }
}
pub fn supported(r: &ResolvedNode) -> bool {
    if !r.value.visible {
        return true;
    }
    let n = &r.value.node;
    r.all_problems().is_empty()
        && matches!(
            n,
            Node::Frame(_) | Node::Group(_) | Node::Rect(_) | Node::Text(_) | Node::Ellipse(_)
        )
        && n.stroke().is_none()
        && n.effects().is_empty()
        && n.blend_mode() == vellum_types::BlendMode::Normal
        && n.fills().iter().filter(|p| p.visible()).count() <= 1
        && n.fills()
            .iter()
            .filter(|p| p.visible())
            .all(|p| matches!(p, Paint::Solid(_)))
        && match n {
            Node::Ellipse(e) => {
                e.inner_radius == 0. && e.start_angle == 0. && e.sweep_angle == 360.
            }
            Node::Text(t) => {
                t.text_style.is_none() && t.vertical_align == vellum_types::VerticalAlign::Top
            }
            _ => true,
        }
        && r.children.iter().all(|c| supported(c))
}
pub fn node_style(n: &Node, b: Rect) -> Result<Styles, ExportError> {
    let mut s = vec![
        ("box-sizing".into(), "border-box".into()),
        ("position".into(), "absolute".into()),
        ("left".into(), px(b.x)),
        ("top".into(), px(b.y)),
        ("width".into(), px(b.width)),
        ("height".into(), px(b.height)),
        ("opacity".into(), number(n.opacity())?.to_string()),
        (
            "transform".into(),
            format!(
                "rotate({}deg) scale({},{})",
                number(n.rotation())?,
                if n.flip_x() { -1 } else { 1 },
                if n.flip_y() { -1 } else { 1 }
            ),
        ),
        ("transform-origin".into(), "50% 50%".into()),
    ];
    let radii = match n {
        Node::Frame(f) => Some(&f.corner_radius),
        Node::Rect(r) => Some(&r.corner_radius),
        _ => None,
    };
    if let Some(radius) = radii {
        let values = match radius {
            CornerRadius::All(v) => vec![px(number(v)?)],
            CornerRadius::Corners(v) => v
                .iter()
                .map(|v| number(v).map(px))
                .collect::<Result<Vec<_>, _>>()?,
        };
        s.push(("border-radius".into(), values.join(" ")));
    }
    if matches!(n, Node::Ellipse(_)) {
        s.push(("border-radius".into(), "50%".into()));
    }
    if let Node::Frame(f) = n {
        s.push((
            "overflow".into(),
            if f.clip { "hidden" } else { "visible" }.into(),
        ));
    }
    if let Node::Text(t) = n {
        s.extend([
            ("color".into(), color(n)?),
            ("white-space".into(), "pre-wrap".into()),
            ("overflow-wrap".into(), "anywhere".into()),
            ("text-align".into(), t.align.as_str().into()),
        ]);
        if t.vertical_align != vellum_types::VerticalAlign::Top {
            return Err(ExportError::new(
                "Text vertical alignment requires an SVG asset",
            ));
        }
    } else {
        s.push(("background".into(), color(n)?));
    }
    Ok(s)
}
pub fn css_string(s: &str) -> String {
    format!(
        "\"{}\"",
        s.chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == ' ' || c == '-' {
                c.to_string()
            } else {
                format!("\\{:x} ", c as u32)
            })
            .collect::<String>()
    )
}
pub fn run_style(r: &TextRun) -> Result<Styles, ExportError> {
    let mut s = vec![
        ("font-family".into(), css_string(&r.font)),
        ("font-optical-sizing".into(), "none".into()),
        ("text-rendering".into(), "geometricPrecision".into()),
        ("-webkit-font-smoothing".into(), "antialiased".into()),
        ("font-weight".into(), r.weight.to_string()),
        ("font-style".into(), r.style.as_str().into()),
        ("font-size".into(), px(number(&r.size)?)),
        ("letter-spacing".into(), px(number(&r.letter_spacing)?)),
    ];
    if let Some(h) = &r.line_height {
        s.push(("line-height".into(), px(number(h)?)));
    }
    let mut decoration = vec![];
    if r.underline {
        decoration.push("underline");
    }
    if r.strikethrough {
        decoration.push("line-through");
    }
    if !decoration.is_empty() {
        s.push(("text-decoration".into(), decoration.join(" ")));
    }
    Ok(s)
}
