use super::{Error, values::*};
use crate::{ports::LayoutResult, resolver::ResolvedNode, scene::geometry::Affine};
use vellum_types::{
    BoolOp, Node,
    display::{Shape, ShapeOperand},
};
pub(super) fn shape(r: &ResolvedNode, l: &LayoutResult) -> Result<Option<Shape>, Error> {
    let b = l
        .boxes
        .get(&r.address)
        .ok_or_else(|| Error::Scene(crate::scene::SceneError::MissingLayout(r.address.clone())))?;
    let size = [f(b.width)?, f(b.height)?];
    let n = &r.value.node;
    Ok(match n {
        Node::Frame(n) => Some(Shape::Rect {
            size,
            radii: radii(&n.corner_radius)?,
        }),
        Node::Rect(n) => Some(Shape::Rect {
            size,
            radii: radii(&n.corner_radius)?,
        }),
        Node::Ellipse(n) => Some(Shape::Ellipse {
            size,
            inner: f(n.inner_radius)?,
            start: f(n.start_angle)?,
            sweep: f(n.sweep_angle)?,
        }),
        Node::Polygon(n) => Some(Shape::Polygon {
            size,
            sides: n.sides,
            radius: number(&n.corner_radius)?,
        }),
        Node::Star(n) => Some(Shape::Star {
            size,
            points: n.points,
            inner: f(n.inner_ratio)?,
        }),
        Node::Line(_) => Some(Shape::Line { width: size[0] }),
        Node::Vector(n) => Some(Shape::Vector {
            size,
            data: n.path.clone(),
            view_box: [
                f(n.view_box.x)?,
                f(n.view_box.y)?,
                f(n.view_box.width)?,
                f(n.view_box.height)?,
            ],
            fill_rule: n.fill_rule,
        }),
        Node::Boolean(n) => Some(boolean(r, l, n.op)?),
        Node::Group(_) => Some(boolean(r, l, BoolOp::Union)?),
        Node::Image(_) | Node::Script(_) => Some(Shape::Rect {
            size,
            radii: [0.; 4],
        }),
        _ => None,
    })
}
fn boolean(r: &ResolvedNode, l: &LayoutResult, op: BoolOp) -> Result<Shape, Error> {
    let mut operands = vec![];
    for c in &r.children {
        if !c.value.visible {
            continue;
        }
        let Some(shape) = shape(c, l)? else {
            return Err(Error::UnsupportedGeometry(c.address.clone()));
        };
        let b = l.boxes[&c.address];
        let rotation = match c.value.node.rotation() {
            vellum_types::Value::Literal(v) => *v,
            _ => return Err(Error::UnresolvedValue),
        };
        let transform =
            matrix(Affine::local(b, rotation, c.value.node.flip_x(), c.value.node.flip_y()).0)?;
        operands.push(ShapeOperand { transform, shape });
    }
    Ok(Shape::Boolean { op, operands })
}
