//! `Value<T>` (a literal or a variable reference), `Color` and `Size`.

use serde_json::json;
use vellum_types::{Color, Size, Value, VarRef};

fn parse<T: serde::de::DeserializeOwned>(json: &str) -> T {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

#[test]
fn a_value_is_a_literal_or_a_variable_reference() {
    let literal: Value<f64> = parse("0.5");
    assert_eq!(literal, Value::Literal(0.5));

    let var: Value<f64> = parse(r#"{"$var":"va000001"}"#);
    assert_eq!(var, Value::Var(VarRef::new("va000001").expect("var ref")));
}

#[test]
fn a_value_prints_the_same_way_it_was_read() {
    assert_eq!(
        serde_json::to_value(Value::Literal(true)).expect("ser"),
        json!(true)
    );
    assert_eq!(
        serde_json::to_value(Value::<f64>::Var(VarRef::new("va000002").expect("var")))
            .expect("ser"),
        json!({"$var": "va000002"})
    );
}

#[test]
fn a_variable_reference_is_an_id_and_has_no_other_keys() {
    assert!(VarRef::new("").is_err());
    assert!(serde_json::from_str::<Value<f64>>(r#"{"$var":""}"#).is_err());
    assert!(serde_json::from_str::<Value<f64>>(r#"{"$var":"a","extra":1}"#).is_err());
    assert!(serde_json::from_str::<Value<f64>>(r#"{"var":"a"}"#).is_err());
}

#[test]
fn a_variable_reference_names_its_variable_by_id_not_by_path() {
    // Ids never hold a slash (guide §9), so a reference that does is a path and is refused.
    assert!(VarRef::new("color/primary").is_err());
    assert!(serde_json::from_str::<Value<f64>>(r#"{"$var":"color/primary"}"#).is_err());
}

#[test]
fn a_value_can_wrap_a_structured_literal() {
    let value: Value<Color> = parse(r##""#FF0000""##);
    assert_eq!(value, Value::Literal(Color::rgb(255, 0, 0)));
}

#[test]
fn a_colour_is_hex_with_or_without_alpha() {
    assert_eq!(
        parse::<Color>(r##""#1A2B3C""##),
        Color::rgb(0x1A, 0x2B, 0x3C)
    );
    assert_eq!(
        parse::<Color>(r##""#1A2B3C80""##),
        Color::rgba(0x1A, 0x2B, 0x3C, 0x80)
    );
    // Case does not matter on the way in; it is upper case on the way out.
    assert_eq!(
        parse::<Color>(r##""#1a2b3c""##),
        Color::rgb(0x1A, 0x2B, 0x3C)
    );
}

#[test]
fn a_colour_prints_short_form_only_when_opaque() {
    assert_eq!(Color::rgb(0x1A, 0x2B, 0x3C).to_string(), "#1A2B3C");
    assert_eq!(Color::rgba(0x1A, 0x2B, 0x3C, 0xFF).to_string(), "#1A2B3C");
    assert_eq!(Color::rgba(0x1A, 0x2B, 0x3C, 0x80).to_string(), "#1A2B3C80");
    assert_eq!(
        serde_json::to_string(&Color::rgba(0, 0, 0, 0)).expect("ser"),
        r##""#00000000""##
    );
}

#[test]
fn a_colour_rejects_anything_else() {
    for bad in [
        "",
        "#",
        "#FFF",
        "#FFFF",
        "#GGGGGG",
        "FFFFFF",
        "#FFFFFFF",
        "#FFFFFFFFF",
        "red",
        "#ffffff ",
    ] {
        assert!(bad.parse::<Color>().is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn sizes_have_four_forms() {
    assert_eq!(parse::<Size>("120"), Size::Fixed(Value::Literal(120.0)));
    assert_eq!(
        parse::<Size>(r#"{"$var":"va000003"}"#),
        Size::Fixed(Value::Var(VarRef::new("va000003").expect("var")))
    );
    assert_eq!(parse::<Size>(r#""hug""#), Size::Hug { fallback: None });
    assert_eq!(parse::<Size>(r#""fill""#), Size::Fill { fallback: None });
    assert_eq!(
        parse::<Size>(r#"{"hug":40}"#),
        Size::Hug {
            fallback: Some(40.0)
        }
    );
    assert_eq!(
        parse::<Size>(r#"{"fill":0}"#),
        Size::Fill {
            fallback: Some(0.0)
        }
    );
}

#[test]
fn each_size_has_exactly_one_canonical_form() {
    for (size, json) in [
        (Size::Fixed(Value::Literal(120.0)), json!(120.0)),
        (Size::Hug { fallback: None }, json!("hug")),
        (Size::Fill { fallback: None }, json!("fill")),
        (
            Size::Hug {
                fallback: Some(40.0),
            },
            json!({"hug": 40.0}),
        ),
        (
            Size::Fill {
                fallback: Some(8.5),
            },
            json!({"fill": 8.5}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&size).expect("ser"), json);
        assert_eq!(serde_json::from_value::<Size>(json).expect("de"), size);
    }
}

#[test]
fn a_bad_size_is_an_error_not_a_default() {
    for bad in [
        r#""auto""#,
        r#"{"hug":"x"}"#,
        r#"{"hug":1,"fill":2}"#,
        r#"{"hug":null}"#,
        "null",
        "[]",
        "true",
    ] {
        assert!(
            serde_json::from_str::<Size>(bad).is_err(),
            "{bad} should be rejected"
        );
    }
}
