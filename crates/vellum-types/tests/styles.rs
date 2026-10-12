//! Styles and fonts (ARCHITECTURE §4.1). A text style holds the settings of a text run without its
//! characters. An effect style holds effects in order. A font is a custom font file carried with the
//! document, and the file is keyed by the asset that holds it.

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value as Json, json};
use vellum_types::{FontFace, Style};

fn outcome<T: DeserializeOwned>(input: Json) -> Result<T, serde_json::Error> {
    serde_json::from_value(input)
}

fn canonical<T: Serialize + DeserializeOwned>(input: Json) -> T {
    let value: T = outcome(input.clone()).unwrap_or_else(|e| panic!("{input}: {e}"));
    let back = serde_json::to_value(&value).expect("serialise");
    assert_eq!(back, input, "did not print back as it was read");
    value
}

#[test]
fn a_text_style_has_the_settings_of_a_text_run_without_its_characters() {
    canonical::<Style>(json!({
        "type": "text",
        "name": "Heading",
        "font": "Geist",
        "weight": 600,
        "style": "italic",
        "size": {"$var": "va000001"},
        "lineHeight": 40.0,
        "letterSpacing": -0.5,
        "underline": true,
        "strikethrough": true
    }));
}

#[test]
fn a_text_style_needs_a_font_and_a_size() {
    assert!(outcome::<Style>(json!({"type": "text", "name": "Heading", "font": "Geist"})).is_err());
    assert!(outcome::<Style>(json!({"type": "text", "name": "Heading", "size": 14.0})).is_err());
}

#[test]
fn an_effect_style_holds_its_effects_in_order() {
    canonical::<Style>(json!({
        "type": "effect",
        "name": "Card",
        "effects": [
            {"type": "drop-shadow", "x": 0.0, "y": 4.0, "blur": 12.0, "color": "#00000033"},
            {"type": "layer-blur", "radius": 2.0}
        ]
    }));
}

#[test]
fn a_style_is_a_text_style_or_an_effect_style_and_nothing_else() {
    assert!(outcome::<Style>(json!({"type": "paint", "name": "x"})).is_err());
    assert!(
        outcome::<Style>(json!({"type": "effect", "name": "x", "effects": [], "extra": 1}))
            .is_err()
    );
}

#[test]
fn a_font_face_is_a_family_and_is_regular_roman_unless_said_otherwise() {
    canonical::<FontFace>(json!({"family": "Geist"}));
    canonical::<FontFace>(json!({"family": "Geist", "weight": 700, "style": "italic"}));
    assert!(outcome::<FontFace>(json!({"family": "Geist", "style": "oblique"})).is_err());
    assert!(
        outcome::<FontFace>(json!({"weight": 400})).is_err(),
        "a font has a family"
    );
}
