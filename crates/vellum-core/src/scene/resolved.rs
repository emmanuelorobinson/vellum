use super::{SceneError, SceneNode};
use crate::{ports::LayoutResult, resolver::ResolvedNode};
use vellum_types::{CornerRadius, Node, NodeIdPath, Value};
/// Prepare a page's resolved roots using parent-relative layout boxes. Hidden subtrees need no layout;
/// a visible occurrence without a box returns an explicit error. No document or store is changed.
pub fn from_resolved(
    roots: &[std::sync::Arc<ResolvedNode>],
    layout: &LayoutResult,
) -> Result<Vec<SceneNode>, SceneError> {
    fn visit(
        r: &ResolvedNode,
        parent: Option<NodeIdPath>,
        layout: &LayoutResult,
        out: &mut Vec<SceneNode>,
    ) -> Result<(), SceneError> {
        if !r.value.visible {
            return Ok(());
        }
        let own = &r.value.node;
        let rotation = match own.rotation() {
            Value::Literal(n) => *n,
            Value::Var(_) => return Err(SceneError::UnresolvedValue(r.address.clone())),
        };
        let bounds = *layout
            .boxes
            .get(&r.address)
            .ok_or_else(|| SceneError::MissingLayout(r.address.clone()))?;
        let number = |v: &Value<f64>| match v {
            Value::Literal(n) => Ok(*n),
            Value::Var(_) => Err(SceneError::UnresolvedValue(r.address.clone())),
        };
        let clip_radii = if let Node::Frame(frame) = own {
            match &frame.corner_radius {
                CornerRadius::All(v) => [number(v)?; 4],
                CornerRadius::Corners(v) => [
                    number(&v[0])?,
                    number(&v[1])?,
                    number(&v[2])?,
                    number(&v[3])?,
                ],
            }
        } else {
            [0.; 4]
        };
        out.push(SceneNode {
            clip_radii,
            parent,
            bounds,
            rotation,
            flip_x: own.flip_x(),
            flip_y: own.flip_y(),
            locked: own.locked(),
            clip: match own {
                Node::Frame(n) => n.clip,
                Node::Script(n) => n.clip,
                _ => false,
            },
            ..SceneNode::default_for(r.address.clone())
        });
        for child in &r.children {
            visit(child, Some(r.address.clone()), layout, out)?;
        }
        Ok(())
    }
    let mut out = vec![];
    for root in roots {
        visit(root, None, layout, &mut out)?;
    }
    Ok(out)
}
