//! Paints, strokes, effects and blend modes (ARCHITECTURE §4.1).

use serde_json::{Value as Json, json};
use vellum_types::{BlendMode, Effect, Paint, Stroke};

fn canonical<T>(input: Json) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let value: T = serde_json::from_value(input.clone()).unwrap_or_else(|e| panic!("{input}: {e}"));
    assert_eq!(serde_json::to_value(&value).expect("serialise"), input);
    value
}

fn rejected<T: serde::de::DeserializeOwned>(input: Json) {
    assert!(
        serde_json::from_value::<T>(input.clone()).is_err(),
        "{input} should be rejected"
    );
}

#[test]
fn there_are_eighteen_blend_modes_in_the_documented_order() {
    let names: Vec<&str> = BlendMode::ALL.iter().map(|m| m.as_str()).collect();
    assert_eq!(
        names,
        [
            "normal",
            "darken",
            "multiply",
            "color-burn",
            "linear-burn",
            "lighten",
            "screen",
            "color-dodge",
            "linear-dodge",
            "overlay",
            "soft-light",
            "hard-light",
            "difference",
            "exclusion",
            "hue",
            "saturation",
            "color",
            "luminosity"
        ]
    );
    assert_eq!(BlendMode::ALL.len(), 18);
    for mode in BlendMode::ALL {
        let json = serde_json::to_value(mode).expect("serialise");
        assert_eq!(json, json!(mode.as_str()));
        assert_eq!(
            serde_json::from_value::<BlendMode>(json).expect("deserialise"),
            mode
        );
    }
    rejected::<BlendMode>(json!("Multiply"));
    rejected::<BlendMode>(json!("plus-lighter"));
}

#[test]
fn a_solid_paint_is_a_colour_or_a_variable() {
    canonical::<Paint>(json!({"type": "solid", "color": "#336699"}));
    canonical::<Paint>(json!({"type": "solid", "color": "#33669980"}));
    canonical::<Paint>(json!({"type": "solid", "color": {"$var": "color/primary"}}));
}

#[test]
fn a_paint_can_be_hidden() {
    let paint = canonical::<Paint>(json!({"type": "solid", "color": "#336699", "visible": false}));
    assert!(!paint.visible());
    let shown = canonical::<Paint>(json!({"type": "solid", "color": "#336699"}));
    assert!(shown.visible());
}

#[test]
fn gradients_are_linear_radial_or_angular() {
    for kind in ["linear", "radial", "angular"] {
        canonical::<Paint>(json!({
            "type": "gradient",
            "kind": kind,
            "stops": [
                {"color": "#000000", "position": 0.0},
                {"color": {"$var": "color/accent"}, "position": 0.5},
                {"color": "#FFFFFF", "position": 1.0}
            ]
        }));
    }
    canonical::<Paint>(json!({
        "type": "gradient",
        "kind": "linear",
        "stops": [{"color": "#000000", "position": 0.0}, {"color": "#FFFFFF", "position": 1.0}],
        "transform": [0.0, 1.0, -1.0, 0.0, 0.5, 0.0],
        "visible": false
    }));
    rejected::<Paint>(json!({"type": "gradient", "kind": "conic", "stops": []}));
    rejected::<Paint>(json!({"type": "gradient", "kind": "linear"}));
    rejected::<Paint>(
        json!({"type": "gradient", "kind": "linear", "stops": [], "transform": [1.0, 0.0]}),
    );
}

#[test]
fn a_mesh_paint_has_a_grid_of_points_with_optional_handles() {
    canonical::<Paint>(json!({
        "type": "mesh",
        "columns": 2,
        "rows": 2,
        "points": [
            {"x": 0.0, "y": 0.0, "color": "#FF0000"},
            {"x": 1.0, "y": 0.0, "color": "#00FF00"},
            {"x": 0.0, "y": 1.0, "color": {"$var": "color/a"}},
            {
                "x": 1.0, "y": 1.0, "color": "#FFFFFF",
                "handles": {
                    "left": {"x": -0.1, "y": 0.0}, "right": {"x": 0.1, "y": 0.0},
                    "up": {"x": 0.0, "y": -0.1}, "down": {"x": 0.0, "y": 0.1}
                }
            }
        ]
    }));
}

#[test]
fn an_image_paint_has_fit_crop_and_opacity() {
    canonical::<Paint>(json!({"type": "image", "asset": "a1b2c3d4e5"}));
    canonical::<Paint>(json!({
        "type": "image", "asset": "a1b2c3d4e5", "fit": "stretch",
        "crop": [2.0, 0.0, 0.0, 2.0, 0.0, 0.0], "opacity": 0.4
    }));
    rejected::<Paint>(json!({"type": "image", "asset": "a1b2c3d4e5", "fit": "cover"}));
}

#[test]
fn a_shader_paint_binds_uniforms_of_every_kind() {
    canonical::<Paint>(json!({
        "type": "shader",
        "src": "shaders/noise.sksl",
        "uniforms": {
            "amount": 0.5,
            "enabled": true,
            "tint": "#FF8800",
            "offset": [1.0, 2.0],
            "light": [1.0, 2.0, 3.0],
            "rect": [1.0, 2.0, 3.0, 4.0],
            "scale": {"$var": "number/scale"}
        }
    }));
    canonical::<Paint>(json!({"type": "shader", "src": "shaders/noise.sksl"}));
    rejected::<Paint>(json!({"type": "shader", "src": "a.sksl", "uniforms": {"v": [1.0]}}));
    rejected::<Paint>(
        json!({"type": "shader", "src": "a.sksl", "uniforms": {"v": [1.0, 2.0, 3.0, 4.0, 5.0]}}),
    );
}

#[test]
fn an_unknown_paint_type_or_field_is_an_error() {
    rejected::<Paint>(json!({"type": "pattern"}));
    rejected::<Paint>(json!({"color": "#000000"}));
    rejected::<Paint>(json!({"type": "solid", "color": "#000000", "blend": "normal"}));
    rejected::<Paint>(json!({"type": "solid"}));
}

#[test]
fn a_stroke_has_paints_width_align_cap_join_and_dash() {
    let stroke = canonical::<Stroke>(json!({
        "paints": [{"type": "solid", "color": "#000000"}],
        "width": 2.0
    }));
    assert_eq!(stroke.align.as_str(), "center");
    assert_eq!(stroke.cap.as_str(), "butt");
    assert_eq!(stroke.join.as_str(), "miter");
    assert!(stroke.dash.is_empty());

    canonical::<Stroke>(json!({
        "paints": [
            {"type": "solid", "color": "#000000"},
            {"type": "gradient", "kind": "linear", "stops": [], "visible": false}
        ],
        "width": {"$var": "stroke/thin"},
        "align": "outside",
        "cap": "round",
        "join": "bevel",
        "dash": [4.0, 2.0]
    }));
}

#[test]
fn stroke_width_is_uniform_or_per_side() {
    canonical::<Stroke>(json!({"paints": [], "width": [1.0, 2.0, 3.0, 4.0]}));
    canonical::<Stroke>(json!({"paints": [], "width": [1.0, {"$var": "w"}, 3.0, 4.0]}));
    rejected::<Stroke>(json!({"paints": [], "width": [1.0, 2.0]}));
    rejected::<Stroke>(json!({"paints": []}));
    for (field, bad) in [("align", "middle"), ("cap", "flat"), ("join", "sharp")] {
        let mut stroke = json!({"paints": [], "width": 1.0});
        stroke[field] = json!(bad);
        rejected::<Stroke>(stroke);
    }
}

#[test]
fn shadows_have_offset_blur_spread_colour_and_blend_mode() {
    for kind in ["drop-shadow", "inner-shadow"] {
        canonical::<Effect>(json!({
            "type": kind, "x": 0.0, "y": 4.0, "blur": 8.0, "color": "#00000040"
        }));
        canonical::<Effect>(json!({
            "type": kind, "x": 1.0, "y": 2.0, "blur": {"$var": "blur/md"}, "spread": 3.0,
            "color": {"$var": "color/shadow"}, "blendMode": "multiply", "visible": false
        }));
    }
}

#[test]
fn blurs_and_shader_effects_round_trip() {
    canonical::<Effect>(json!({"type": "layer-blur", "radius": 4.0}));
    canonical::<Effect>(
        json!({"type": "background-blur", "radius": {"$var": "blur/glass"}, "visible": false}),
    );
    canonical::<Effect>(json!({
        "type": "shader", "src": "shaders/glow.sksl", "uniforms": {"strength": 0.8}
    }));
    canonical::<Effect>(json!({"type": "shader", "src": "shaders/glow.sksl"}));
}

#[test]
fn an_unknown_effect_or_missing_field_is_an_error() {
    rejected::<Effect>(json!({"type": "glow", "radius": 4.0}));
    rejected::<Effect>(json!({"type": "layer-blur"}));
    rejected::<Effect>(json!({"type": "drop-shadow", "x": 0.0, "y": 0.0}));
    rejected::<Effect>(json!({"type": "layer-blur", "radius": 4.0, "spread": 1.0}));
}
