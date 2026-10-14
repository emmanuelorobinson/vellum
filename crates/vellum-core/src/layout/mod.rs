//! Shared layout preparation: resolved occurrences → our layout tree → ADR-002 sizing semantics.
//! No vendor types. HTML export and inspection can consume the same prepared dimensions.
mod resolved;
use crate::ports::{Dimension, LayoutContent, LayoutNode, LayoutStyle, LayoutTree};
pub use resolved::from_resolved;
use std::{collections::BTreeSet, fmt};
use vellum_types::{Justify, Layout, NodeIdPath};
/// A sizing axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Width,
    Height,
}
/// Why a layout input cannot be used. Errors never produce fabricated boxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    MissingFallback {
        node: NodeIdPath,
        axis: Axis,
    },
    InvalidNumber {
        node: NodeIdPath,
        property: &'static str,
    },
    InvalidGrid {
        node: NodeIdPath,
    },
    DuplicateAddress {
        node: NodeIdPath,
    },
    UnresolvedValue {
        node: NodeIdPath,
    },
    TextChildren {
        node: NodeIdPath,
    },
}
impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "layout input: {self:?}")
    }
}
impl std::error::Error for LayoutError {}
/// A diagnostic with occurrence addresses, suitable for inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutProblem {
    /// A main-axis fill would depend on its hugging parent; ADR-002 sizes it to its own content.
    HugCycleBroken {
        parent: NodeIdPath,
        child: NodeIdPath,
        axis: Axis,
    },
    Invalid(LayoutError),
}
/// A dimension after fallbacks and dependency cycles have been resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sizing {
    Fixed(f64),
    Hug,
    Fill,
}
/// Prepared own fields and dimensions, still wholly independent of a layout implementation.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedNode {
    pub id: NodeIdPath,
    pub style: LayoutStyle,
    pub width: Sizing,
    pub height: Sizing,
    pub positioned: bool,
    pub content: LayoutContent,
    pub children: Vec<PreparedNode>,
}
/// Prepared tree plus safe cycle-resolution diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTree {
    pub root: PreparedNode,
    pub problems: Vec<LayoutProblem>,
}
/// Resolve hug/fill in context before any vendor or CSS mapping. Hidden subtrees still validate.
pub fn prepare(tree: &LayoutTree) -> Result<PreparedTree, LayoutError> {
    let mut seen = BTreeSet::new();
    let mut problems = Vec::new();
    let root = visit(&tree.root, None, &mut seen, &mut problems)?;
    Ok(PreparedTree { root, problems })
}
fn visit(
    n: &LayoutNode,
    parent: Option<&PreparedNode>,
    seen: &mut BTreeSet<NodeIdPath>,
    problems: &mut Vec<LayoutProblem>,
) -> Result<PreparedNode, LayoutError> {
    if !seen.insert(n.id.clone()) {
        return Err(LayoutError::DuplicateAddress { node: n.id.clone() });
    }
    let s = &n.style;
    // f64 model values must also be representable by the current f32 layout engines.
    for (property, v, negative) in [
        ("x", s.x, true),
        ("y", s.y, true),
        ("gap", s.gap, false),
        ("padding", s.padding.top, false),
        ("padding", s.padding.right, false),
        ("padding", s.padding.bottom, false),
        ("padding", s.padding.left, false),
        ("stroke", s.stroke.top, false),
        ("stroke", s.stroke.right, false),
        ("stroke", s.stroke.bottom, false),
        ("stroke", s.stroke.left, false),
    ] {
        valid(n, property, v, negative)?;
    }
    if s.mode == Layout::Grid {
        if s.columns.is_none_or(|c| c == 0 || c > u16::MAX as u32)
            || s.rows.is_some_and(|r| r == 0 || r > u16::MAX as u32)
            || matches!(s.justify, Justify::SpaceBetween | Justify::SpaceAround)
        {
            return Err(LayoutError::InvalidGrid { node: n.id.clone() });
        }
    } else if s.columns.is_some() || s.rows.is_some() {
        return Err(LayoutError::InvalidGrid { node: n.id.clone() });
    }
    if let LayoutContent::Text(rich) = &n.content {
        if !n.children.is_empty() {
            return Err(LayoutError::TextChildren { node: n.id.clone() });
        }
        for run in &rich.runs {
            valid(n, "font size", run.size, false)?;
            valid(n, "letter spacing", run.letter_spacing, true)?;
            if let Some(height) = run.line_height {
                valid(n, "line height", height, false)?;
            }
        }
    }
    let positioned = s.absolute || parent.is_some_and(|p| p.style.mode == Layout::None);
    let mut result = PreparedNode {
        id: n.id.clone(),
        style: s.clone(),
        width: Sizing::Hug,
        height: Sizing::Hug,
        positioned,
        content: n.content.clone(),
        children: vec![],
    };
    result.width = dimension(n, parent, positioned, Axis::Width, s.width, problems)?;
    result.height = dimension(n, parent, positioned, Axis::Height, s.height, problems)?;
    result.children = n
        .children
        .iter()
        .map(|c| visit(c, Some(&result), seen, problems))
        .collect::<Result<_, _>>()?;
    Ok(result)
}
fn valid(
    n: &LayoutNode,
    property: &'static str,
    value: f64,
    negative: bool,
) -> Result<(), LayoutError> {
    if !value.is_finite() || !((value as f32).is_finite()) || (!negative && value < 0.0) {
        Err(LayoutError::InvalidNumber {
            node: n.id.clone(),
            property,
        })
    } else {
        Ok(())
    }
}
fn dimension(
    n: &LayoutNode,
    parent: Option<&PreparedNode>,
    positioned: bool,
    axis: Axis,
    dim: Dimension,
    problems: &mut Vec<LayoutProblem>,
) -> Result<Sizing, LayoutError> {
    let fallback = match dim {
        Dimension::Fixed(v) => {
            valid(n, "size", v, false)?;
            return Ok(Sizing::Fixed(v));
        }
        Dimension::Hug { fallback } | Dimension::Fill { fallback } => fallback,
    };
    if let Some(v) = fallback {
        valid(n, "fallback", v, false)?;
    }
    let fixed = || {
        fallback
            .map(Sizing::Fixed)
            .ok_or_else(|| LayoutError::MissingFallback {
                node: n.id.clone(),
                axis,
            })
    };
    let hug = || {
        if n.style.mode != Layout::None || matches!(n.content, LayoutContent::Text(_)) {
            let has_content = matches!(n.content, LayoutContent::Text(_))
                || n.children
                    .iter()
                    .any(|c| c.style.visible && !c.style.absolute);
            Ok(if !has_content {
                fallback.map(Sizing::Fixed).unwrap_or(Sizing::Hug)
            } else {
                Sizing::Hug
            })
        } else {
            fixed()
        }
    };
    match dim {
        Dimension::Hug { .. } => hug(),
        Dimension::Fill { .. } => match parent {
            None => fixed(),
            Some(_) if positioned => fixed(),
            Some(p) => {
                let main = matches!(
                    (p.style.mode, axis),
                    (Layout::Row, Axis::Width) | (Layout::Column, Axis::Height)
                );
                let ps = if axis == Axis::Width {
                    p.width
                } else {
                    p.height
                };
                if main && ps == Sizing::Hug {
                    if let Some(v) = fallback {
                        Ok(Sizing::Fixed(v))
                    } else {
                        problems.push(LayoutProblem::HugCycleBroken {
                            parent: p.id.clone(),
                            child: n.id.clone(),
                            axis,
                        });
                        hug()
                    }
                } else {
                    Ok(Sizing::Fill)
                }
            }
        },
        Dimension::Fixed(_) => unreachable!(),
    }
}
