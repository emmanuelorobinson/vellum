//! TEST DATA: documents and tokens made for these tests.
use std::collections::BTreeMap;

use serde_json::json;
use vellum_types::{Color, Document, Variable, VariableId, VariableType};

use super::*;

fn id(text: &str) -> VariableId {
    VariableId::new(text).unwrap()
}

fn variable(value: serde_json::Value) -> Variable {
    serde_json::from_value(value).unwrap()
}

fn themes() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([
        (
            "Mode".to_owned(),
            vec![
                "Light".to_owned(),
                "Dark".to_owned(),
                "High contrast".to_owned(),
            ],
        ),
        (
            "Device".to_owned(),
            vec!["Desktop".to_owned(), "Phone".to_owned()],
        ),
    ])
}

fn document() -> Document {
    let mut doc = Document::new("TEST DATA");
    doc.themes = themes();
    let vars = [
        (
            "v-primary",
            json!({"type": "color", "name": "color/primary", "values": [
            {"value": "#2563EB"}, {"value": "#60A5FA", "when": {"Mode": "Dark"}}]}),
        ),
        (
            "v-accent",
            json!({"type": "color", "name": "color/accent", "values": [
            {"$alias": "v-primary"}, {"value": "#FFFFFF", "when": {"Mode": "High contrast"}}]}),
        ),
        (
            "v-space",
            json!({"type": "number", "name": "space/4", "values": [
            {"value": 16.0}, {"value": 12.0, "when": {"Device": "Phone"}}]}),
        ),
        (
            "v-font",
            json!({"type": "string", "name": "font/body", "values": [{"value": "Inter"}]}),
        ),
        (
            "v-flag",
            json!({"type": "boolean", "name": "flag/dense", "values": [{"value": false}]}),
        ),
    ];
    for (key, value) in vars {
        doc.variables.insert(id(key), variable(value));
    }
    doc
}

fn counter() -> impl FnMut() -> VariableId {
    let mut n = 0;
    move || {
        n += 1;
        id(&format!("new-{n}"))
    }
}

#[test]
fn collect_writes_the_default_and_each_single_axis_difference() {
    let (set, warnings) = collect(&document());
    assert_eq!(warnings, vec![]);
    assert_eq!(set.themes, themes());
    let names: Vec<_> = set.tokens.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "color/accent",
            "color/primary",
            "flag/dense",
            "font/body",
            "space/4"
        ]
    );
    let primary = set.get("color/primary").unwrap();
    assert_eq!(primary.kind, VariableType::Color);
    assert_eq!(
        primary.value,
        TokenValue::Color(Color::rgb(0x25, 0x63, 0xEB))
    );
    assert_eq!(
        primary.modes,
        vec![ModeValue {
            axis: "Mode".into(),
            mode: "Dark".into(),
            value: TokenValue::Color(Color::rgb(0x60, 0xA5, 0xFA)),
        }]
    );
    // An alias is written as the name of its target, in the default and in a mode that keeps it.
    let accent = set.get("color/accent").unwrap();
    assert_eq!(accent.value, TokenValue::Alias("color/primary".into()));
    assert_eq!(accent.modes.len(), 1);
    assert_eq!(accent.modes[0].mode, "High contrast");
    // Dark does not differ for the accent: it is still the alias, so it is not listed.
    let space = set.get("space/4").unwrap();
    assert_eq!(space.modes[0].axis, "Device");
    assert_eq!(space.modes[0].value, TokenValue::Number(12.0));
}

#[test]
fn collect_says_what_it_could_not_keep() {
    let mut doc = document();
    doc.variables.insert(
        id("v-both"),
        variable(json!({"type": "number", "name": "size/both", "values": [
            {"value": 1.0}, {"value": 2.0, "when": {"Mode": "Dark", "Device": "Phone"}}]})),
    );
    doc.variables.insert(
        id("v-none"),
        variable(json!({"type": "number", "name": "size/none", "values": [
            {"value": 3.0, "when": {"Mode": "Dark"}}]})),
    );
    doc.variables.insert(
        id("v-dangling"),
        variable(
            json!({"type": "number", "name": "size/dangling", "values": [{"$alias": "gone"}]}),
        ),
    );
    let (set, warnings) = collect(&doc);
    let about = |name: &str| warnings.iter().find(|w| w.token.as_deref() == Some(name));
    assert!(
        about("size/both")
            .unwrap()
            .message
            .contains("more than one theme axis")
    );
    assert!(
        set.get("size/both").is_some(),
        "the default value is still kept"
    );
    assert!(
        about("size/none")
            .unwrap()
            .message
            .contains("no value under the default theme")
    );
    assert!(set.get("size/none").is_none());
    assert!(about("size/dangling").unwrap().message.contains("gone"));
    assert!(set.get("size/dangling").is_none());
}

#[test]
fn the_default_mapping_follows_the_themes() {
    let mapping = ThemeMapping::derive(&themes());
    assert_eq!(mapping.selector("Mode", "Light"), Some(":root"));
    assert_eq!(mapping.selector("Mode", "Dark"), Some(".dark"));
    assert_eq!(
        mapping.selector("Mode", "High contrast"),
        Some("[data-theme=\"high-contrast\"]")
    );
    assert_eq!(mapping.selector("Device", "Desktop"), Some(":root"));
    assert_eq!(
        mapping.selector("Device", "Phone"),
        Some("[data-device=\"phone\"]")
    );
    assert_eq!(mapping.validate(), Ok(()));
}

#[test]
fn a_document_without_themes_gets_light_and_dark() {
    let mapping = ThemeMapping::derive(&BTreeMap::new());
    assert_eq!(mapping, ThemeMapping::standard());
    assert_eq!(
        mapping.themes(),
        BTreeMap::from([(
            "Mode".to_owned(),
            vec!["Light".to_owned(), "Dark".to_owned()]
        )])
    );
}

#[test]
fn a_selector_is_found_whatever_its_quotes_and_spaces() {
    let mapping = ThemeMapping::derive(&themes());
    assert_eq!(mapping.lookup(".dark"), Some(("Mode", "Dark")));
    assert_eq!(
        mapping.lookup("[ data-device = 'phone' ]"),
        Some(("Device", "Phone"))
    );
    assert_eq!(mapping.lookup(":root"), None, "the root is no theme value");
    assert_eq!(mapping.lookup(".light"), None);
    assert!(ThemeMapping::is_root("  :root "));
}

#[test]
fn the_person_edits_a_mapping_row_by_row() {
    let mapping = ThemeMapping::derive(&themes())
        .with(&[ModeSelector::new("Mode", "Dark", "[data-theme=\"dark\"]")])
        .unwrap();
    assert_eq!(
        mapping.selector("Mode", "Dark"),
        Some("[data-theme=\"dark\"]")
    );
    assert_eq!(mapping.selector("Mode", "Light"), Some(":root"));
    // A value the mapping lacks is added after the axis's others.
    let mapping = mapping
        .with(&[ModeSelector::new("Mode", "Sepia", ".sepia")])
        .unwrap();
    assert_eq!(
        mapping.themes()["Mode"],
        ["Light", "Dark", "High contrast", "Sepia"]
    );
}

#[test]
fn a_mapping_that_could_not_be_read_back_is_refused() {
    let base = ThemeMapping::derive(&themes());
    let refuse = |edit: ModeSelector, word: &str| {
        let error = base.clone().with(&[edit]).unwrap_err().to_string();
        assert!(error.contains(word), "{error}");
    };
    refuse(
        ModeSelector::new("Mode", "Light", ".light"),
        "default value",
    );
    refuse(
        ModeSelector::new("Mode", "Dark", ":root"),
        "default values go",
    );
    refuse(
        ModeSelector::new("Mode", "Dark", "[data-device=\"phone\"]"),
        "already the selector",
    );
    refuse(
        ModeSelector::new("Mode", "Dark", ".dark { color: red }"),
        "not a selector",
    );
    refuse(ModeSelector::new("Mode", "Dark", " "), "not a selector");
    refuse(
        ModeSelector::new("Mode", "Dark", ".a; .b"),
        "not a selector",
    );
}

fn incoming(tokens: Vec<Token>) -> TokenSet {
    TokenSet {
        themes: themes(),
        tokens,
    }
}

#[test]
fn a_file_identical_to_the_document_changes_nothing() {
    let doc = document();
    let (set, _) = collect(&doc);
    let plan = plan(&doc, &set, true, &mut counter()).unwrap();
    assert!(plan.diff.is_empty(), "{:?}", plan.diff);
    assert_eq!(plan.diff.unchanged.len(), 5);
    assert!(plan.set.is_empty() && plan.remove.is_empty() && plan.themes.is_none());
}

#[test]
fn the_diff_lists_added_changed_and_removed() {
    let doc = document();
    let (mut set, _) = collect(&doc);
    set.tokens.retain(|t| t.name != "flag/dense");
    let primary = set
        .tokens
        .iter_mut()
        .find(|t| t.name == "color/primary")
        .unwrap();
    primary.value = TokenValue::Color(Color::rgb(0, 0, 0));
    primary.modes.clear();
    set.tokens.push(Token {
        name: "radius/md".into(),
        kind: VariableType::Number,
        value: TokenValue::Number(8.0),
        modes: vec![],
    });
    let plan = plan(&doc, &set, false, &mut counter()).unwrap();
    assert_eq!(plan.diff.added, ["radius/md"]);
    assert_eq!(plan.diff.removed, ["flag/dense"]);
    assert_eq!(
        plan.diff.changed,
        vec![Change {
            name: "color/primary".into(),
            what: vec!["value".into(), "Mode: Dark".into()]
        }]
    );
    // Only added and changed variables are set, and removing needs asking for.
    assert_eq!(plan.set.len(), 2);
    assert!(plan.remove.is_empty());
    assert_eq!(
        plan.ids["color/primary"],
        id("v-primary"),
        "a token keeps its variable's id"
    );
    assert_eq!(plan.ids["radius/md"], id("new-1"));

    let plan = super::plan(&doc, &set, true, &mut counter()).unwrap();
    assert_eq!(plan.remove, [id("v-flag")].into());
}

#[test]
fn a_changed_token_is_a_whole_variable_in_normal_form() {
    let doc = document();
    let (mut set, _) = collect(&doc);
    let space = set.tokens.iter_mut().find(|t| t.name == "space/4").unwrap();
    space.value = TokenValue::Number(20.0);
    let plan = plan(&doc, &set, false, &mut counter()).unwrap();
    assert_eq!(
        plan.set[&id("v-space")],
        variable(json!({"type": "number", "name": "space/4", "values": [
            {"value": 20.0}, {"value": 12.0, "when": {"Device": "Phone"}}]}))
    );
}

#[test]
fn aliases_become_the_id_of_the_token_they_name() {
    let doc = Document::new("TEST DATA");
    let set = TokenSet {
        themes: BTreeMap::new(),
        tokens: vec![
            Token {
                name: "color/accent".into(),
                kind: VariableType::Color,
                value: TokenValue::Alias("color/primary".into()),
                modes: vec![],
            },
            Token {
                name: "color/primary".into(),
                kind: VariableType::Color,
                value: TokenValue::Color(Color::rgb(1, 2, 3)),
                modes: vec![],
            },
        ],
    };
    let plan = plan(&doc, &set, false, &mut counter()).unwrap();
    assert_eq!(
        plan.set[&plan.ids["color/accent"]],
        variable(json!({"type": "color", "name": "color/accent", "values": [
            {"$alias": plan.ids["color/primary"].as_str()}]}))
    );
}

#[test]
fn a_mode_the_document_lacks_is_added_to_its_themes() {
    let doc = Document::new("TEST DATA");
    let set = TokenSet {
        themes: ThemeMapping::standard().themes(),
        tokens: vec![Token {
            name: "color/bg".into(),
            kind: VariableType::Color,
            value: TokenValue::Color(Color::rgb(255, 255, 255)),
            modes: vec![ModeValue {
                axis: "Mode".into(),
                mode: "Dark".into(),
                value: TokenValue::Color(Color::rgb(0, 0, 0)),
            }],
        }],
    };
    let plan = plan(&doc, &set, false, &mut counter()).unwrap();
    assert_eq!(
        plan.themes,
        Some(BTreeMap::from([(
            "Mode".to_owned(),
            vec!["Light".to_owned(), "Dark".to_owned()]
        )]))
    );
    // A value added to an axis the document has goes after the others.
    let mut doc = Document::new("TEST DATA");
    doc.themes = ThemeMapping::standard().themes();
    let mut set = set;
    set.tokens[0].modes[0].mode = "Sepia".into();
    let plan = super::plan(&doc, &set, false, &mut counter()).unwrap();
    assert_eq!(plan.themes.unwrap()["Mode"], ["Light", "Dark", "Sepia"]);
}

#[test]
fn a_set_that_cannot_be_variables_is_refused_with_the_token_at_fault() {
    let doc = document();
    let token = |name: &str, kind, value| Token {
        name: name.into(),
        kind,
        value,
        modes: vec![],
    };
    let refuse = |tokens: Vec<Token>, word: &str| {
        let error = plan(&doc, &incoming(tokens), false, &mut counter()).unwrap_err();
        assert!(error.to_string().contains(word), "{error}");
        error
    };
    let error = refuse(
        vec![token(
            "a",
            VariableType::Color,
            TokenValue::Alias("missing".into()),
        )],
        "not a token in the file",
    );
    assert_eq!(error.token.as_deref(), Some("a"));
    refuse(
        vec![token("a", VariableType::Color, TokenValue::Number(1.0))],
        "not a color",
    );
    refuse(
        vec![token(
            "a",
            VariableType::Number,
            TokenValue::Alias("color/primary".into()),
        )],
        "a color token",
    );
    refuse(
        vec![
            token("a", VariableType::Color, TokenValue::Alias("b".into())),
            token("b", VariableType::Color, TokenValue::Alias("a".into())),
        ],
        "a → b → a",
    );
    refuse(
        vec![
            token("a", VariableType::Number, TokenValue::Number(1.0)),
            token("a", VariableType::Number, TokenValue::Number(2.0)),
        ],
        "twice",
    );
    refuse(
        vec![token(" a", VariableType::Number, TokenValue::Number(1.0))],
        "cannot be empty",
    );
}

#[test]
fn a_mode_on_an_axis_nobody_defined_is_refused() {
    let doc = Document::new("TEST DATA");
    let set = TokenSet {
        themes: BTreeMap::new(),
        tokens: vec![Token {
            name: "a".into(),
            kind: VariableType::Number,
            value: TokenValue::Number(1.0),
            modes: vec![ModeValue {
                axis: "Density".into(),
                mode: "Compact".into(),
                value: TokenValue::Number(2.0),
            }],
        }],
    };
    let error = plan(&doc, &set, false, &mut counter()).unwrap_err();
    assert!(error.to_string().contains("no theme axis"), "{error}");
}

#[test]
fn lengths_are_matched_by_prefix() {
    let options = TokenOptions::for_themes(&BTreeMap::new());
    assert!(options.is_length("radius/md"));
    assert!(options.is_length("space/4"));
    assert!(options.is_length("font/size-lg"));
    assert!(!options.is_length("font/weight-bold"));
    assert!(!options.is_length("spacer/1"), "a prefix is a whole word");
    assert!(!options.is_length("opacity/muted"));
}
