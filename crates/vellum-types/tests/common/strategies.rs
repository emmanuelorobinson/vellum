//! Generators for property tests: any node, paint, override or JSON value the types can hold. Shared by
//! `roundtrip.rs` here and the codec's tests, which include this file by path.
#![allow(dead_code)]

use std::collections::BTreeMap;

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use serde_json::Value as Json;
use vellum_types::*;

// --- leaves -------------------------------------------------------------------------------------

/// Finite numbers, with the defaults (0, 1, 360) over-represented: a field at its default is left out
/// of the file, and that path only runs when the generator lands on the default.
pub fn arb_f64() -> impl Strategy<Value = f64> {
    prop_oneof![
        3 => Just(0.0),
        3 => Just(1.0),
        1 => Just(360.0),
        6 => -1.0e6..1.0e6_f64,
        1 => prop::num::f64::NORMAL | prop::num::f64::SUBNORMAL | prop::num::f64::ZERO,
    ]
}

pub fn arb_text() -> impl Strategy<Value = String> {
    "\\PC{0,12}"
}

pub fn arb_id_text() -> impl Strategy<Value = String> {
    "[A-Za-z0-9_-]{1,24}"
}

pub fn arb_node_id() -> impl Strategy<Value = NodeId> {
    arb_id_text().prop_map(|s| NodeId::new(s).expect("valid"))
}

/// A reference names its variable by id (guide §9: ids never hold a slash).
pub fn arb_var() -> impl Strategy<Value = VarRef> {
    arb_id_text().prop_map(|s| VarRef::new(s).expect("valid id"))
}

pub fn arb_value<T: std::fmt::Debug + 'static>(
    literal: impl Strategy<Value = T> + 'static,
) -> impl Strategy<Value = Value<T>> {
    prop_oneof![3 => literal.prop_map(Value::Literal), 1 => arb_var().prop_map(Value::Var)]
}

pub fn arb_num() -> impl Strategy<Value = Value<f64>> {
    arb_value(arb_f64())
}

pub fn arb_color() -> impl Strategy<Value = Color> {
    (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
        .prop_map(|(r, g, b, a)| Color::rgba(r, g, b, a))
}

pub fn arb_color_value() -> impl Strategy<Value = Value<Color>> {
    arb_value(arb_color())
}

pub fn arb_point() -> impl Strategy<Value = Point> {
    (arb_f64(), arb_f64()).prop_map(|(x, y)| Point { x, y })
}

pub fn arb_transform() -> impl Strategy<Value = Transform> {
    prop_oneof![
        1 => Just(Transform::IDENTITY),
        3 => prop::array::uniform6(arb_f64()).prop_map(Transform),
    ]
}

pub fn arb_size() -> impl Strategy<Value = Size> {
    prop_oneof![
        arb_num().prop_map(Size::Fixed),
        Just(Size::Hug { fallback: None }),
        Just(Size::Fill { fallback: None }),
        arb_f64().prop_map(|f| Size::Hug { fallback: Some(f) }),
        arb_f64().prop_map(|f| Size::Fill { fallback: Some(f) }),
    ]
}

pub fn arb_position() -> impl Strategy<Value = Position> {
    prop_oneof![
        Just(Position::Auto),
        arb_point().prop_map(Position::Absolute)
    ]
}

pub fn arb_padding() -> impl Strategy<Value = Padding> {
    prop_oneof![
        arb_num().prop_map(Padding::All),
        prop::array::uniform2(arb_num()).prop_map(Padding::Axes),
        prop::array::uniform4(arb_num()).prop_map(Padding::Sides),
    ]
}

pub fn arb_radius() -> impl Strategy<Value = CornerRadius> {
    prop_oneof![
        arb_num().prop_map(CornerRadius::All),
        prop::array::uniform4(arb_num()).prop_map(CornerRadius::Corners),
    ]
}

pub fn arb_enum<T: Copy + std::fmt::Debug + 'static, const N: usize>(
    all: [T; N],
) -> impl Strategy<Value = T> {
    (0..N).prop_map(move |i| all[i])
}

pub fn arb_json() -> impl Strategy<Value = Json> {
    let leaf = prop_oneof![
        Just(Json::Null),
        any::<bool>().prop_map(Json::from),
        any::<i64>().prop_map(Json::from),
        (-1.0e6..1.0e6_f64).prop_map(Json::from),
        arb_text().prop_map(Json::from),
    ];
    leaf.prop_recursive(3, 16, 4, |inner| {
        prop_oneof![
            vec(inner.clone(), 0..4).prop_map(Json::Array),
            btree_map(arb_text(), inner, 0..4).prop_map(|m| Json::Object(m.into_iter().collect())),
        ]
    })
}

pub fn arb_scalar() -> impl Strategy<Value = Scalar> {
    prop_oneof![
        any::<bool>().prop_map(Scalar::Bool),
        arb_f64().prop_map(Scalar::Number),
        arb_text().prop_map(Scalar::Text),
    ]
}

/// The property names a props override may set: every node property except the id and the children,
/// which have their own rules (ARCHITECTURE §4.1, [`NodePatch`]).
pub fn arb_patch_key() -> impl Strategy<Value = String> {
    let names: Vec<&'static str> = NODE_PROPERTIES
        .iter()
        .copied()
        .filter(|name| !matches!(*name, "id" | "children"))
        .collect();
    prop::sample::select(names).prop_map(str::to_owned)
}

/// One of the three ways an instance overrides a node: properties, a replacement node or new children.
pub fn arb_override(
    node: impl Strategy<Value = Node> + Clone + 'static,
) -> impl Strategy<Value = Override> {
    prop_oneof![
        btree_map(arb_patch_key(), arb_json(), 0..3)
            .prop_map(|map| Override::Props(NodePatch::new(map).expect("listed names"))),
        node.clone()
            .prop_map(|node| Override::Replace(Box::new(node))),
        vec(node, 0..3).prop_map(Override::Children),
    ]
}

pub fn arb_uniforms() -> impl Strategy<Value = BTreeMap<String, Value<Uniform>>> {
    let uniform = prop_oneof![
        any::<bool>().prop_map(Uniform::Bool),
        arb_f64().prop_map(Uniform::Number),
        arb_color().prop_map(Uniform::Color),
        prop::array::uniform2(arb_f64()).prop_map(Uniform::Vec2),
        prop::array::uniform3(arb_f64()).prop_map(Uniform::Vec3),
        prop::array::uniform4(arb_f64()).prop_map(Uniform::Vec4),
    ];
    btree_map("[a-z]{1,6}", arb_value(uniform), 0..3)
}

// --- paints, strokes, effects -------------------------------------------------------------------

pub fn arb_paint() -> impl Strategy<Value = Paint> {
    let solid = (arb_color_value(), any::<bool>())
        .prop_map(|(color, visible)| Paint::Solid(SolidPaint { color, visible }));
    let gradient = (
        arb_enum(GradientKind::ALL),
        vec((arb_color_value(), arb_f64()), 0..4),
        arb_transform(),
        any::<bool>(),
    )
        .prop_map(|(kind, stops, transform, visible)| {
            Paint::Gradient(GradientPaint {
                kind,
                stops: stops
                    .into_iter()
                    .map(|(color, position)| GradientStop { color, position })
                    .collect(),
                transform,
                visible,
            })
        });
    let handles = prop::option::of(
        (arb_point(), arb_point(), arb_point(), arb_point()).prop_map(|(left, right, up, down)| {
            MeshHandles {
                left,
                right,
                up,
                down,
            }
        }),
    );
    let mesh = (
        1..5_u32,
        1..5_u32,
        vec((arb_f64(), arb_f64(), arb_color_value(), handles), 0..5),
        any::<bool>(),
    )
        .prop_map(|(columns, rows, points, visible)| {
            Paint::Mesh(MeshPaint {
                columns,
                rows,
                points: points
                    .into_iter()
                    .map(|(x, y, color, handles)| MeshPoint {
                        x,
                        y,
                        color,
                        handles,
                    })
                    .collect(),
                visible,
            })
        });
    let image = (
        arb_id_text(),
        arb_enum(ImageFit::ALL),
        prop::option::of(arb_transform()),
        arb_num(),
        any::<bool>(),
    )
        .prop_map(|(asset, fit, crop, opacity, visible)| {
            Paint::Image(ImagePaint {
                asset: AssetId::new(asset).expect("valid"),
                fit,
                crop,
                opacity,
                visible,
            })
        });
    let shader =
        (arb_text(), arb_uniforms(), any::<bool>()).prop_map(|(src, uniforms, visible)| {
            Paint::Shader(ShaderPaint {
                src,
                uniforms,
                visible,
            })
        });
    prop_oneof![solid, gradient, mesh, image, shader]
}

pub fn arb_stroke() -> impl Strategy<Value = Stroke> {
    let width = prop_oneof![
        arb_num().prop_map(StrokeWidth::Uniform),
        prop::array::uniform4(arb_num()).prop_map(StrokeWidth::PerSide),
    ];
    (
        vec(arb_paint(), 0..3),
        width,
        arb_enum(StrokeAlign::ALL),
        arb_enum(StrokeCap::ALL),
        arb_enum(StrokeJoin::ALL),
        vec(arb_f64(), 0..4),
    )
        .prop_map(|(paints, width, align, cap, join, dash)| Stroke {
            paints,
            width,
            align,
            cap,
            join,
            dash,
        })
}

pub fn arb_effect() -> impl Strategy<Value = Effect> {
    let shadow = (
        arb_num(),
        arb_num(),
        arb_num(),
        arb_num(),
        arb_color_value(),
        arb_enum(BlendMode::ALL),
        any::<bool>(),
    )
        .prop_map(|(x, y, blur, spread, color, blend_mode, visible)| Shadow {
            x,
            y,
            blur,
            spread,
            color,
            blend_mode,
            visible,
        })
        .boxed();
    let blur = (arb_num(), any::<bool>())
        .prop_map(|(radius, visible)| Blur { radius, visible })
        .boxed();
    let shader =
        (arb_text(), arb_uniforms(), any::<bool>()).prop_map(|(src, uniforms, visible)| {
            ShaderEffect {
                src,
                uniforms,
                visible,
            }
        });
    prop_oneof![
        shadow.clone().prop_map(Effect::DropShadow),
        shadow.prop_map(Effect::InnerShadow),
        blur.clone().prop_map(Effect::LayerBlur),
        blur.prop_map(Effect::BackgroundBlur),
        shader.prop_map(Effect::Shader),
    ]
}

pub fn arb_run() -> impl Strategy<Value = TextRun> {
    (
        (
            arb_text(),
            arb_text(),
            1..=9_u16,
            arb_enum(FontStyle::ALL),
            arb_num(),
        ),
        (
            prop::option::of(arb_num()),
            arb_num(),
            any::<bool>(),
            any::<bool>(),
            prop::option::of(arb_text()),
        ),
    )
        .prop_map(
            |(
                (text, font, weight, style, size),
                (line_height, letter_spacing, underline, strikethrough, link),
            )| {
                TextRun {
                    text,
                    font,
                    weight: weight * 100,
                    style,
                    size,
                    line_height,
                    letter_spacing,
                    underline,
                    strikethrough,
                    link,
                }
            },
        )
}

pub fn arb_prototype() -> impl Strategy<Value = Prototype> {
    (arb_node_id(), arb_enum(TransitionKind::ALL), any::<u32>()).prop_map(
        |(target, kind, duration_ms)| Prototype {
            target,
            transition: Transition { kind, duration_ms },
        },
    )
}

// --- nodes --------------------------------------------------------------------------------------

/// The fields every node has, generated once and stamped onto whichever kind is built.
#[derive(Debug, Clone)]
pub struct Common {
    id: NodeId,
    name: String,
    visible: Value<bool>,
    locked: bool,
    opacity: Value<f64>,
    rotation: Value<f64>,
    flip_x: bool,
    flip_y: bool,
    position: Position,
    width: Size,
    height: Size,
    fills: Vec<Paint>,
    stroke: Option<Stroke>,
    effects: Vec<Effect>,
    blend_mode: BlendMode,
    agent_notes: Option<String>,
    prototype: Option<Prototype>,
    metadata: BTreeMap<String, Json>,
}

prop_compose! {
    pub fn arb_common()(
        id in arb_node_id(),
        name in arb_text(),
        visible in arb_value(any::<bool>()),
        locked in any::<bool>(),
        opacity in arb_num(),
        rotation in arb_num(),
        flip_x in any::<bool>(),
        flip_y in any::<bool>(),
        position in arb_position(),
    )(
        width in arb_size(),
        height in arb_size(),
        fills in vec(arb_paint(), 0..3),
        stroke in prop::option::of(arb_stroke()),
        effects in vec(arb_effect(), 0..3),
        blend_mode in arb_enum(BlendMode::ALL),
        agent_notes in prop::option::of(arb_text()),
        prototype in prop::option::of(arb_prototype()),
        metadata in btree_map(arb_text(), arb_json(), 0..3),
        id in Just(id), name in Just(name), visible in Just(visible), locked in Just(locked),
        opacity in Just(opacity), rotation in Just(rotation), flip_x in Just(flip_x),
        flip_y in Just(flip_y), position in Just(position),
    ) -> Common {
        Common {
            id, name, visible, locked, opacity, rotation, flip_x, flip_y, position, width, height,
            fills, stroke, effects, blend_mode, agent_notes, prototype, metadata,
        }
    }
}

/// `build!(Kind { field: value, .. }, common)`: a node struct with the common fields filled in.
macro_rules! build {
    ($kind:ident { $($field:ident : $value:expr),* $(,)? }, $c:expr) => {{
        let c: Common = $c;
        $kind {
            id: c.id, name: c.name, visible: c.visible, locked: c.locked, opacity: c.opacity,
            rotation: c.rotation, flip_x: c.flip_x, flip_y: c.flip_y, position: c.position,
            width: c.width, height: c.height, fills: c.fills, stroke: c.stroke, effects: c.effects,
            blend_mode: c.blend_mode, agent_notes: c.agent_notes, prototype: c.prototype,
            metadata: c.metadata,
            $($field: $value),*
        }
    }};
}

/// Every kind with no children.
pub fn arb_leaf() -> impl Strategy<Value = Node> {
    let shapes = prop_oneof![
        (arb_common(), arb_radius()).prop_map(|(c, corner_radius)| Node::Rect(build!(
            RectNode {
                corner_radius: corner_radius
            },
            c
        ))),
        (arb_common(), arb_f64(), arb_f64(), arb_f64()).prop_map(
            |(c, inner_radius, start_angle, sweep_angle)| {
                Node::Ellipse(build!(
                    EllipseNode {
                        inner_radius: inner_radius,
                        start_angle: start_angle,
                        sweep_angle: sweep_angle
                    },
                    c
                ))
            }
        ),
        (arb_common(), any::<u32>(), arb_num()).prop_map(|(c, sides, corner_radius)| {
            Node::Polygon(build!(
                PolygonNode {
                    sides: sides,
                    corner_radius: corner_radius
                },
                c
            ))
        }),
        (arb_common(), any::<u32>(), arb_f64()).prop_map(|(c, points, inner_ratio)| Node::Star(
            build!(
                StarNode {
                    points: points,
                    inner_ratio: inner_ratio
                },
                c
            )
        )),
        arb_common().prop_map(|c| Node::Line(build!(LineNode {}, c))),
        (
            arb_common(),
            arb_text(),
            (arb_f64(), arb_f64(), arb_f64(), arb_f64()),
            arb_enum(FillRule::ALL)
        )
            .prop_map(|(c, path, (x, y, width, height), fill_rule)| {
                Node::Vector(build!(
                    VectorNode {
                        path: path,
                        view_box: ViewBox {
                            x,
                            y,
                            width,
                            height
                        },
                        fill_rule: fill_rule
                    },
                    c
                ))
            }),
    ];
    let content = prop_oneof![
        (
            arb_common(),
            vec(arb_run(), 0..3),
            arb_enum(TextAlign::ALL),
            arb_enum(VerticalAlign::ALL),
            arb_enum(TextResize::ALL),
            prop::option::of(arb_id_text()),
        )
            .prop_map(|(c, content, align, vertical_align, resize, style)| {
                Node::Text(build!(
                    TextNode {
                        content: content,
                        align: align,
                        vertical_align: vertical_align,
                        resize: resize,
                        text_style: style.map(|s| StyleId::new(s).expect("valid")),
                    },
                    c
                ))
            }),
        (
            arb_common(),
            arb_id_text(),
            arb_enum(ImageFit::ALL),
            prop::option::of(arb_transform())
        )
            .prop_map(|(c, asset, fit, crop)| {
                Node::Image(build!(
                    ImageNode {
                        asset: AssetId::new(asset).expect("valid"),
                        fit: fit,
                        crop: crop
                    },
                    c
                ))
            }),
        (
            arb_common(),
            arb_text(),
            arb_value(arb_text()),
            any::<u16>(),
            any::<bool>()
        )
            .prop_map(|(c, library, icon, weight, fill)| Node::Icon(build!(
                IconNode {
                    library: library,
                    icon: icon,
                    weight: weight,
                    fill: fill
                },
                c
            ))),
        (
            arb_common(),
            arb_text(),
            btree_map(arb_text(), arb_value(arb_scalar()), 0..3),
            any::<bool>()
        )
            .prop_map(|(c, src, inputs, clip)| Node::Script(build!(
                ScriptNode {
                    src: src,
                    inputs: inputs,
                    clip: clip
                },
                c
            ))),
        (
            arb_common(),
            arb_text(),
            prop::option::of(arb_color_value())
        )
            .prop_map(|(c, content, color)| Node::Note(build!(
                NoteNode {
                    content: content,
                    color: color
                },
                c
            ))),
        (arb_common(), arb_text(), prop::option::of(arb_node_id())).prop_map(
            |(c, content, points_at)| Node::Context(build!(
                ContextNode {
                    content: content,
                    points_at: points_at
                },
                c
            ))
        ),
        (
            arb_common(),
            arb_text(),
            prop::option::of(arb_text()),
            prop::option::of(arb_text())
        )
            .prop_map(|(c, content, engine, model)| Node::Prompt(build!(
                PromptNode {
                    content: content,
                    engine: engine,
                    model: model
                },
                c
            ))),
        (
            arb_common(),
            arb_text(),
            prop::option::of(arb_text()),
            arb_f64(),
            arb_point(),
            arb_num()
        )
            .prop_map(|(c, url, device, zoom, scroll, radius)| {
                Node::Embed(build!(
                    EmbedNode {
                        url: url,
                        device: device,
                        zoom: zoom,
                        scroll: scroll,
                        radius: radius
                    },
                    c
                ))
            }),
    ];
    prop_oneof![shapes, content]
}

pub fn arb_node() -> impl Strategy<Value = Node> {
    arb_leaf().prop_recursive(3, 24, 4, |inner| {
        let children = vec(inner.clone(), 0..4);
        prop_oneof![
            (
                arb_common(),
                (
                    arb_enum(Layout::ALL),
                    prop::option::of(1..6_u32),
                    prop::option::of(1..6_u32),
                    arb_num(),
                    arb_padding(),
                    arb_enum(Justify::ALL),
                    arb_enum(Align::ALL),
                ),
                (
                    any::<bool>(),
                    any::<bool>(),
                    arb_radius(),
                    btree_map("[A-Za-z]{1,6}", "[A-Za-z]{1,6}", 0..3),
                    prop::option::of(vec(arb_id_text(), 0..3)),
                    children.clone(),
                ),
            )
                .prop_map(
                    |(
                        c,
                        (layout, columns, rows, gap, padding, justify, align),
                        (layout_includes_stroke, clip, corner_radius, theme, slot, children),
                    )| {
                        Node::Frame(build!(
                            FrameNode {
                                layout: layout,
                                columns: columns,
                                rows: rows,
                                gap: gap,
                                padding: padding,
                                justify: justify,
                                align: align,
                                layout_includes_stroke: layout_includes_stroke,
                                clip: clip,
                                corner_radius: corner_radius,
                                theme: theme,
                                slot: slot.map(|ids| Slot {
                                    suggested: ids
                                        .into_iter()
                                        .map(|i| ComponentId::new(i).expect("valid"))
                                        .collect(),
                                }),
                                children: children,
                            },
                            c
                        ))
                    }
                ),
            (arb_common(), children.clone())
                .prop_map(|(c, children)| Node::Group(build!(GroupNode { children: children }, c))),
            (
                arb_common(),
                prop::option::of(inner.clone()),
                prop::option::of(inner.clone()),
                vec("[a-z]{1,8}", 0..3),
                children.clone(),
            )
                .prop_map(|(c, ours, theirs, properties, children)| {
                    Node::Conflict(build!(
                        ConflictNode {
                            ours: ours.map(Box::new),
                            theirs: theirs.map(Box::new),
                            properties: properties,
                            children: children
                        },
                        c
                    ))
                }),
            (arb_common(), arb_enum(BoolOp::ALL), children).prop_map(|(c, op, children)| {
                Node::Boolean(build!(
                    BooleanNode {
                        op: op,
                        children: children
                    },
                    c
                ))
            }),
            (
                arb_common(),
                arb_id_text(),
                btree_map("[A-Za-z]{1,6}", "[A-Za-z]{1,6}", 0..3),
                btree_map("[a-z]{1,6}", arb_scalar(), 0..3),
                btree_map(
                    vec(arb_node_id(), 1..4).prop_map(|ids| {
                        let mut it = ids.into_iter();
                        let first = NodeIdPath::from(it.next().expect("one id"));
                        it.fold(first, NodeIdPath::join)
                    }),
                    arb_override(inner.clone()),
                    0..3,
                ),
                btree_map(arb_node_id(), vec(inner, 0..3), 0..3),
            )
                .prop_map(|(c, component, variant, props, overrides, slot_content)| {
                    Node::Instance(build!(
                        InstanceNode {
                            component_id: ComponentId::new(component).expect("valid"),
                            library: None,
                            variant: variant,
                            props: props,
                            overrides: overrides,
                            slot_content: slot_content,
                        },
                        c
                    ))
                }),
        ]
    })
}
