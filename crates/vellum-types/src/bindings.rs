//! Where a node can hold a variable. Every property that takes a `Value<T>` is a place a `{"$var": …}` can
//! go, and each place has a type: opacity takes a number variable, a fill's colour takes a colour variable,
//! visibility takes a boolean one. [`Node::visit_bindings`] walks those places with the type of each, so the
//! op layer can refuse a variable of the wrong type when it is bound, and the resolver can replace a
//! reference with its value, without either of them knowing the list of properties.
//!
//! A walk covers one node's own properties and its styles' contents. It does not descend into children,
//! slot content, overrides or the versions a conflict node keeps for children; the caller walks the tree
//! (a conflict's `ours` and `theirs` are the node's own versions and are walked). Props overrides are plain
//! JSON, not typed values, so a walk does not see them: Prompt 17 checks them when it applies them.

use std::fmt;

use crate::color::Color;
use crate::geometry::{CornerRadius, Padding, Size, StrokeWidth};
use crate::node::{Node, on_each};
use crate::paint::Uniform;
use crate::style::Style;
use crate::value::{Scalar, Value};

#[derive(Clone, Copy)]
enum Segment<'a> {
    Root,
    Key(&'static str),
    Index(usize),
    Name(&'a str),
}

/// Where a binding is inside its node, in the file's spelling: `opacity`, `fills[0].color`,
/// `effects[1].uniforms.tint`. It is built on the stack as the walk goes down and costs nothing until it is
/// printed, so a walk that finds no problem allocates nothing for it.
#[derive(Clone, Copy)]
pub struct BindingPath<'a> {
    parent: Option<&'a BindingPath<'a>>,
    segment: Segment<'a>,
}

impl<'a> BindingPath<'a> {
    /// The node itself.
    pub const fn root() -> Self {
        Self {
            parent: None,
            segment: Segment::Root,
        }
    }

    /// The property `key` of this place.
    pub fn key<'b>(&'b self, key: &'static str) -> BindingPath<'b>
    where
        'a: 'b,
    {
        BindingPath {
            parent: Some(self),
            segment: Segment::Key(key),
        }
    }

    /// The `index`th entry of this place.
    pub fn index<'b>(&'b self, index: usize) -> BindingPath<'b>
    where
        'a: 'b,
    {
        BindingPath {
            parent: Some(self),
            segment: Segment::Index(index),
        }
    }

    /// The entry called `name` of this place (a shader uniform, a script input).
    pub fn name<'b>(&'b self, name: &'b str) -> BindingPath<'b>
    where
        'a: 'b,
    {
        BindingPath {
            parent: Some(self),
            segment: Segment::Name(name),
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self.segment, Segment::Root) && self.parent.is_none_or(Self::is_empty)
    }
}

impl fmt::Display for BindingPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dot = |f: &mut fmt::Formatter<'_>| {
            if self.parent.is_some_and(|p| !p.is_empty()) {
                f.write_str(".")
            } else {
                Ok(())
            }
        };
        if let Some(parent) = self.parent {
            parent.fmt(f)?;
        }
        match self.segment {
            Segment::Root => Ok(()),
            Segment::Key(key) => {
                dot(f)?;
                f.write_str(key)
            }
            Segment::Index(index) => write!(f, "[{index}]"),
            Segment::Name(name) => {
                dot(f)?;
                f.write_str(name)
            }
        }
    }
}

impl fmt::Debug for BindingPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BindingPath({self})")
    }
}

/// What a walk calls for each place that can hold a variable, by the type that place takes. Every method
/// does nothing unless overridden.
pub trait Bindings {
    /// A place that takes a number variable.
    fn number(&mut self, _at: &BindingPath<'_>, _value: &Value<f64>) {}
    /// A place that takes a boolean variable.
    fn boolean(&mut self, _at: &BindingPath<'_>, _value: &Value<bool>) {}
    /// A place that takes a colour variable.
    fn color(&mut self, _at: &BindingPath<'_>, _value: &Value<Color>) {}
    /// A place that takes a string variable.
    fn string(&mut self, _at: &BindingPath<'_>, _value: &Value<String>) {}
    /// A place that takes a boolean, number or string variable (a script input).
    fn scalar(&mut self, _at: &BindingPath<'_>, _value: &Value<Scalar>) {}
    /// A place that takes a boolean, number or colour variable (a shader uniform).
    fn uniform(&mut self, _at: &BindingPath<'_>, _value: &Value<Uniform>) {}
}

/// As [`Bindings`], for a walk that changes the values.
pub trait BindingsMut {
    /// A place that takes a number variable.
    fn number(&mut self, _at: &BindingPath<'_>, _value: &mut Value<f64>) {}
    /// A place that takes a boolean variable.
    fn boolean(&mut self, _at: &BindingPath<'_>, _value: &mut Value<bool>) {}
    /// A place that takes a colour variable.
    fn color(&mut self, _at: &BindingPath<'_>, _value: &mut Value<Color>) {}
    /// A place that takes a string variable.
    fn string(&mut self, _at: &BindingPath<'_>, _value: &mut Value<String>) {}
    /// A place that takes a boolean, number or string variable (a script input).
    fn scalar(&mut self, _at: &BindingPath<'_>, _value: &mut Value<Scalar>) {}
    /// A place that takes a boolean, number or colour variable (a shader uniform).
    fn uniform(&mut self, _at: &BindingPath<'_>, _value: &mut Value<Uniform>) {}
}

/// Writes the walk once for both the reading and the writing trait: `$($m)*` is `mut` or nothing.
macro_rules! walkers {
    ($module:ident, $trait:ident, $($m:tt)*) => {
        mod $module {
            use super::*;
            use crate::effect::Effect;
            use crate::paint::Paint;
            use crate::stroke::Stroke;
            use crate::text::TextRun;

            fn numbers<V: $trait>(v: &mut V, at: &BindingPath<'_>, values: &$($m)* [Value<f64>]) {
                for (i, value) in values.into_iter().enumerate() {
                    v.number(&at.index(i), value);
                }
            }

            fn size<V: $trait>(v: &mut V, at: &BindingPath<'_>, size: &$($m)* Size) {
                if let Size::Fixed(value) = size {
                    v.number(at, value);
                }
            }

            fn padding<V: $trait>(v: &mut V, at: &BindingPath<'_>, padding: &$($m)* Padding) {
                match padding {
                    Padding::All(value) => v.number(at, value),
                    Padding::Axes(values) => numbers(v, at, &$($m)* values[..]),
                    Padding::Sides(values) => numbers(v, at, &$($m)* values[..]),
                }
            }

            fn corner_radius<V: $trait>(v: &mut V, at: &BindingPath<'_>, radius: &$($m)* CornerRadius) {
                match radius {
                    CornerRadius::All(value) => v.number(at, value),
                    CornerRadius::Corners(values) => numbers(v, at, &$($m)* values[..]),
                }
            }

            fn paints<V: $trait>(v: &mut V, at: &BindingPath<'_>, paints: &$($m)* [Paint]) {
                for (i, paint) in paints.into_iter().enumerate() {
                    self::paint(v, &at.index(i), paint);
                }
            }

            fn paint<V: $trait>(v: &mut V, at: &BindingPath<'_>, paint: &$($m)* Paint) {
                match paint {
                    Paint::Solid(p) => v.color(&at.key("color"), &$($m)* p.color),
                    Paint::Gradient(p) => {
                        for (i, stop) in (&$($m)* p.stops).into_iter().enumerate() {
                            v.color(&at.key("stops").index(i).key("color"), &$($m)* stop.color);
                        }
                    }
                    Paint::Mesh(p) => {
                        for (i, point) in (&$($m)* p.points).into_iter().enumerate() {
                            v.color(&at.key("points").index(i).key("color"), &$($m)* point.color);
                        }
                    }
                    Paint::Image(p) => v.number(&at.key("opacity"), &$($m)* p.opacity),
                    Paint::Shader(p) => uniforms(v, at, &$($m)* p.uniforms),
                }
            }

            fn uniforms<V: $trait>(
                v: &mut V,
                at: &BindingPath<'_>,
                uniforms: &$($m)* std::collections::BTreeMap<String, Value<Uniform>>,
            ) {
                let at = at.key("uniforms");
                for (name, value) in uniforms {
                    v.uniform(&at.name(name), value);
                }
            }

            fn stroke<V: $trait>(v: &mut V, at: &BindingPath<'_>, stroke: &$($m)* Stroke) {
                paints(v, &at.key("paints"), &$($m)* stroke.paints);
                let at = at.key("width");
                match &$($m)* stroke.width {
                    StrokeWidth::Uniform(value) => v.number(&at, value),
                    StrokeWidth::PerSide(values) => numbers(v, &at, &$($m)* values[..]),
                }
            }

            fn effects<V: $trait>(v: &mut V, at: &BindingPath<'_>, effects: &$($m)* [Effect]) {
                for (i, effect) in effects.into_iter().enumerate() {
                    let at = at.index(i);
                    match effect {
                        Effect::DropShadow(s) | Effect::InnerShadow(s) => {
                            v.number(&at.key("x"), &$($m)* s.x);
                            v.number(&at.key("y"), &$($m)* s.y);
                            v.number(&at.key("blur"), &$($m)* s.blur);
                            v.number(&at.key("spread"), &$($m)* s.spread);
                            v.color(&at.key("color"), &$($m)* s.color);
                        }
                        Effect::LayerBlur(b) | Effect::BackgroundBlur(b) => {
                            v.number(&at.key("radius"), &$($m)* b.radius);
                        }
                        Effect::Shader(s) => uniforms(v, &at, &$($m)* s.uniforms),
                    }
                }
            }

            fn font<V: $trait>(
                v: &mut V,
                at: &BindingPath<'_>,
                size: &$($m)* Value<f64>,
                line_height: &$($m)* Option<Value<f64>>,
                letter_spacing: &$($m)* Value<f64>,
            ) {
                v.number(&at.key("size"), size);
                if let Some(line_height) = line_height {
                    v.number(&at.key("lineHeight"), line_height);
                }
                v.number(&at.key("letterSpacing"), letter_spacing);
            }

            fn runs<V: $trait>(v: &mut V, at: &BindingPath<'_>, runs: &$($m)* [TextRun]) {
                for (i, run) in runs.into_iter().enumerate() {
                    font(
                        v,
                        &at.index(i),
                        &$($m)* run.size,
                        &$($m)* run.line_height,
                        &$($m)* run.letter_spacing,
                    );
                }
            }

            pub(super) fn node<V: $trait>(node: &$($m)* Node, v: &mut V, at: &BindingPath<'_>) {
                on_each!(&$($m)* *node, n => {
                    v.boolean(&at.key("visible"), &$($m)* n.visible);
                    v.number(&at.key("opacity"), &$($m)* n.opacity);
                    v.number(&at.key("rotation"), &$($m)* n.rotation);
                    size(v, &at.key("width"), &$($m)* n.width);
                    size(v, &at.key("height"), &$($m)* n.height);
                    paints(v, &at.key("fills"), &$($m)* n.fills);
                    if let Some(s) = &$($m)* n.stroke {
                        stroke(v, &at.key("stroke"), s);
                    }
                    effects(v, &at.key("effects"), &$($m)* n.effects);
                });
                match node {
                    Node::Frame(n) => {
                        v.number(&at.key("gap"), &$($m)* n.gap);
                        padding(v, &at.key("padding"), &$($m)* n.padding);
                        corner_radius(v, &at.key("cornerRadius"), &$($m)* n.corner_radius);
                    }
                    Node::Rect(n) => corner_radius(v, &at.key("cornerRadius"), &$($m)* n.corner_radius),
                    Node::Polygon(n) => v.number(&at.key("cornerRadius"), &$($m)* n.corner_radius),
                    Node::Text(n) => runs(v, &at.key("content"), &$($m)* n.content),
                    Node::Icon(n) => v.string(&at.key("icon"), &$($m)* n.icon),
                    Node::Script(n) => {
                        let at = at.key("inputs");
                        for (name, value) in &$($m)* n.inputs {
                            v.scalar(&at.name(name), value);
                        }
                    }
                    Node::Note(n) => {
                        if let Some(color) = &$($m)* n.color {
                            v.color(&at.key("color"), color);
                        }
                    }
                    Node::Embed(n) => v.number(&at.key("radius"), &$($m)* n.radius),
                    Node::Conflict(n) => {
                        if let Some(ours) = &$($m)* n.ours {
                            self::node(&$($m)* **ours, v, &at.key("ours"));
                        }
                        if let Some(theirs) = &$($m)* n.theirs {
                            self::node(&$($m)* **theirs, v, &at.key("theirs"));
                        }
                    }
                    Node::Group(_)
                    | Node::Ellipse(_)
                    | Node::Star(_)
                    | Node::Line(_)
                    | Node::Vector(_)
                    | Node::Boolean(_)
                    | Node::Image(_)
                    | Node::Instance(_)
                    | Node::Context(_)
                    | Node::Prompt(_) => {}
                }
            }

            pub(super) fn style<V: $trait>(style: &$($m)* Style, v: &mut V) {
                let at = BindingPath::root();
                match style {
                    Style::Text(t) => font(
                        v,
                        &at,
                        &$($m)* t.size,
                        &$($m)* t.line_height,
                        &$($m)* t.letter_spacing,
                    ),
                    Style::Effect(e) => effects(v, &at.key("effects"), &$($m)* e.effects),
                }
            }
        }
    };
}

walkers!(read, Bindings,);
walkers!(write, BindingsMut, mut);

impl Node {
    /// Calls `visitor` for every place on this node that can hold a variable. Children are not visited.
    pub fn visit_bindings(&self, visitor: &mut impl Bindings) {
        read::node(self, visitor, &BindingPath::root());
    }

    /// As [`Node::visit_bindings`], with the values open to change.
    pub fn visit_bindings_mut(&mut self, visitor: &mut impl BindingsMut) {
        write::node(self, visitor, &BindingPath::root());
    }
}

impl Style {
    /// Calls `visitor` for every place in this style that can hold a variable.
    pub fn visit_bindings(&self, visitor: &mut impl Bindings) {
        read::style(self, visitor);
    }

    /// As [`Style::visit_bindings`], with the values open to change.
    pub fn visit_bindings_mut(&mut self, visitor: &mut impl BindingsMut) {
        write::style(self, visitor);
    }
}
