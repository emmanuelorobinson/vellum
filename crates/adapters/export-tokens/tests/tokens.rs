//! TEST DATA. What each token format writes and reads: exact text, round trips, real-world input, and the
//! things it must say it left out.
mod tokens {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};
    use vellum_contracts::{samples, suites};
    use vellum_core::ports::TokenFormat;
    use vellum_core::tokens::{
        ModeSelector, ModeValue, ThemeMapping, Token, TokenOptions, TokenSet, TokenValue,
    };
    use vellum_export_tokens::{CssVars, DtcgJson, TailwindV4, format_by_id, formats};
    use vellum_types::{Color, VariableType};

    fn options(format: &dyn TokenFormat, set: &TokenSet) -> TokenOptions {
        format.default_options(&set.themes)
    }

    fn export(format: &dyn TokenFormat, set: &TokenSet) -> String {
        format.export(set, &options(format, set)).unwrap().text
    }

    fn import(format: &dyn TokenFormat, text: &str) -> vellum_core::ports::TokenImport {
        // The options of the sample set's themes: `Mode` and `Device`.
        format
            .import(text, &format.default_options(&samples::token_set().themes))
            .unwrap()
    }

    fn color(name: &str, value: &str) -> Token {
        Token {
            name: name.into(),
            kind: VariableType::Color,
            value: TokenValue::Color(value.parse().unwrap()),
            modes: vec![],
        }
    }

    #[test]
    fn every_format_passes_the_contract_suite() {
        let set = samples::token_set();
        for format in formats() {
            suites::tokens::format(&*format, &set, &options(&*format, &set));
        }
    }

    #[test]
    fn formats_are_found_by_their_ids() {
        let ids: Vec<_> = formats().iter().map(|f| f.id()).collect();
        assert_eq!(ids, ["css-vars", "tailwind-v4", "dtcg-json"]);
        assert_eq!(
            format_by_id("tailwind-v4").unwrap().file_name(),
            "tokens.css"
        );
        assert!(format_by_id("swift").is_none());
    }

    #[test]
    fn css_variables_are_written_per_selector() {
        let text = export(&CssVars, &samples::token_set());
        assert_eq!(
            text,
            r##":root {
  --color-accent: var(--color-brand-primary);
  --color-brand-primary: #2563EB; /* name: color/brand/primary */
  --flag-dense: false; /* name: flag/dense */
  --font-body: Inter, sans-serif;
  --label-hex: "#2563eb"; /* name: label/hex */
  --radius-md: 8px;
  --space-4: 16px;
}

[data-device="phone"] {
  --flag-dense: true;
  --space-4: 12.5px;
}

.dark {
  --color-brand-primary: #60A5FACC;
}
"##
        );
    }

    #[test]
    fn tailwind_puts_the_default_in_theme_and_renames_space_to_spacing() {
        let text = export(&TailwindV4, &samples::token_set());
        assert!(text.starts_with("@theme {\n  --color-accent: var(--color-brand-primary);\n"));
        assert!(text.contains("  --spacing-4: 16px;\n"), "{text}");
        assert!(!text.contains("--space-4"), "{text}");
        assert!(text.contains("\n.dark {\n  --color-brand-primary: #60A5FACC;\n}\n"));
        assert!(text.contains(
            "[data-device=\"phone\"] {\n  --flag-dense: true;\n  --spacing-4: 12.5px;\n}"
        ));
        // Read back, the renamed namespace is `space` again and nothing needs a hint.
        let read = import(&TailwindV4, &text);
        assert!(read.set.get("space/4").is_some(), "{:?}", read.set);
    }

    #[test]
    fn dtcg_nests_names_and_keeps_modes_in_an_extension() {
        let text = export(&DtcgJson, &samples::token_set());
        let value: Value = serde_json::from_str(&text).unwrap();
        let ext = vellum_types::brand::NAME.to_lowercase();
        assert_eq!(
            value,
            json!({
                "$extensions": {ext.clone(): {"themes": {
                    "Device": ["Desktop", "Phone"], "Mode": ["Light", "Dark"]}}},
                "color": {
                    "accent": {"$type": "color", "$value": "{color.brand.primary}"},
                    "brand": {"primary": {"$type": "color", "$value": "#2563EB",
                        "$extensions": {ext.clone(): {"modes": {"Mode": {"Dark": "#60A5FACC"}}}}}}
                },
                "flag": {"dense": {"$type": "boolean", "$value": false,
                    "$extensions": {ext.clone(): {"modes": {"Device": {"Phone": true}}}}}},
                "font": {"body": {"$type": "string", "$value": "Inter, sans-serif"}},
                "label": {"hex": {"$type": "string", "$value": "#2563eb"}},
                "radius": {"md": {"$type": "dimension", "$value": "8px"}},
                "space": {"4": {"$type": "dimension", "$value": "16px",
                    "$extensions": {ext: {"modes": {"Device": {"Phone": "12.5px"}}}}}},
            })
        );
        assert!(text.ends_with("}\n"));
    }

    #[test]
    fn every_format_reads_back_exactly_what_it_wrote() {
        let set = samples::token_set();
        for format in formats() {
            let read = import(&*format, &export(&*format, &set));
            assert_eq!(read.set.tokens, set.tokens, "{}", format.id());
            assert_eq!(read.warnings, vec![], "{}", format.id());
        }
    }

    #[test]
    fn the_themes_a_file_used_come_with_their_defaults() {
        let read = import(&CssVars, &export(&CssVars, &samples::token_set()));
        assert_eq!(
            read.set.themes["Mode"],
            ["Light", "Dark"],
            "a mode block names its axis and the axis's default"
        );
        assert_eq!(read.set.themes["Device"], ["Desktop", "Phone"]);
        let read = import(&DtcgJson, &export(&DtcgJson, &samples::token_set()));
        assert_eq!(read.set.themes, samples::token_set().themes);
    }

    #[test]
    fn names_the_file_cannot_spell_are_kept_by_a_hint_or_the_known_names() {
        let set = TokenSet {
            themes: BTreeMap::new(),
            tokens: vec![
                color("color/a/b/c", "#010203"),
                color("weird name/with space", "#040506"),
                color("x-y", "#070809"),
            ],
        };
        let text = export(&CssVars, &set);
        assert!(
            text.contains("/* name: color/a/b/c */")
                && text.contains("/* name: weird name/with space */")
        );
        assert_eq!(import(&CssVars, &text).set.tokens, set.tokens);
        // Without the hints, the document's own names still decide.
        let bare = ":root { --color-a-b-c: #010203; --color-d-e: #000000; }";
        let mut options = TokenOptions::for_themes(&BTreeMap::new());
        options
            .known
            .insert("color/a/b/c".into(), VariableType::Color);
        let names: Vec<_> = CssVars
            .import(bare, &options)
            .unwrap()
            .set
            .tokens
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, ["color/a/b/c", "color/d-e"]);
    }

    #[test]
    fn strings_that_look_like_other_types_are_quoted_and_survive() {
        let strings = [
            "16px",
            "true",
            "#fff",
            "rgb(1, 2, 3)",
            "var(--x)",
            "",
            " padded ",
            "a\"b",
            "a\\b",
            "semi;colon",
            "brace}",
            "line\nbreak",
            "/* c */",
            "Inter, sans-serif",
            "50%",
            "1em",
        ];
        let set = TokenSet {
            themes: BTreeMap::new(),
            tokens: strings
                .iter()
                .enumerate()
                .map(|(i, s)| Token {
                    name: format!("str/s{i:02}"),
                    kind: VariableType::String,
                    value: TokenValue::String((*s).into()),
                    modes: vec![],
                })
                .collect(),
        };
        for format in formats() {
            let read = import(&*format, &export(&*format, &set));
            assert_eq!(read.set.tokens, set.tokens, "{}", format.id());
        }
    }

    #[test]
    fn numbers_are_lengths_by_name_and_plain_otherwise() {
        let number = |name: &str, n: f64| Token {
            name: name.into(),
            kind: VariableType::Number,
            value: TokenValue::Number(n),
            modes: vec![],
        };
        let set = TokenSet {
            themes: BTreeMap::new(),
            tokens: vec![
                number("font/weight-bold", 700.0),
                number("opacity/muted", 0.5),
                number("radius/md", 8.0),
                number("space/neg", -4.0),
            ],
        };
        let text = export(&CssVars, &set);
        assert!(text.contains("--font-weight-bold: 700;"), "{text}");
        assert!(text.contains("--opacity-muted: 0.5;"), "{text}");
        assert!(text.contains("--radius-md: 8px;"), "{text}");
        assert!(text.contains("--space-neg: -4px;"), "{text}");
        assert_eq!(import(&CssVars, &text).set.tokens, set.tokens);
    }

    #[test]
    fn lengths_in_rem_are_read_at_sixteen_pixels_and_other_units_stay_text() {
        let read = import(
            &CssVars,
            ":root { --size-a: 1.5rem; --size-b: 10px; --size-c: .5; --size-d: 2em; --size-e: 50%; --size-f: 3s; }",
        );
        let value = |n: &str| read.set.get(n).unwrap().value.clone();
        assert_eq!(value("size/a"), TokenValue::Number(24.0));
        assert_eq!(value("size/b"), TokenValue::Number(10.0));
        assert_eq!(value("size/c"), TokenValue::Number(0.5));
        assert_eq!(value("size/d"), TokenValue::String("2em".into()));
        assert_eq!(value("size/e"), TokenValue::String("50%".into()));
        assert_eq!(value("size/f"), TokenValue::String("3s".into()));
    }

    #[test]
    fn a_shadcn_stylesheet_is_read() {
        let css = r#"
@import "tailwindcss";
@custom-variant dark (&:is(.dark *));

:root {
  --radius: 0.625rem;
  --background: oklch(1 0 0);
  --foreground: oklch(0.145 0 0);
  --primary: oklch(0.205 0 0); /* the main action */
  --border: rgb(0 0 0 / 10%);
  --ring: hsl(0 0% 50%);
}

.dark {
  --background: oklch(0.145 0 0);
  --foreground: oklch(0.985 0 0);
}

@theme inline {
  --color-background: var(--background);
  --radius-lg: var(--radius);
}

@media (prefers-color-scheme: dark) {
  :root { --ignored: 1; }
}
"#;
        let read = import(&CssVars, css);
        let get = |n: &str| {
            read.set
                .get(n)
                .unwrap_or_else(|| panic!("{n}: {:?}", read.set))
        };
        assert_eq!(
            get("background").value,
            TokenValue::Color(Color::rgb(255, 255, 255))
        );
        assert_eq!(
            get("background").modes,
            vec![ModeValue {
                axis: "Mode".into(),
                mode: "Dark".into(),
                value: TokenValue::Color(Color::rgb(10, 10, 10)),
            }]
        );
        assert_eq!(
            get("foreground").value,
            TokenValue::Color(Color::rgb(10, 10, 10))
        );
        assert_eq!(
            get("foreground").modes[0].value,
            TokenValue::Color(Color::rgb(250, 250, 250))
        );
        assert_eq!(
            get("primary").value,
            TokenValue::Color(Color::rgb(23, 23, 23))
        );
        assert_eq!(
            get("border").value,
            TokenValue::Color(Color::rgba(0, 0, 0, 26))
        );
        assert_eq!(
            get("ring").value,
            TokenValue::Color(Color::rgb(128, 128, 128))
        );
        assert_eq!(get("radius").value, TokenValue::Number(10.0));
        assert_eq!(
            get("color/background").value,
            TokenValue::Alias("background".into())
        );
        assert_eq!(get("radius/lg").value, TokenValue::Alias("radius".into()));
        assert_eq!(
            get("radius/lg").kind,
            VariableType::Number,
            "an alias takes its target's type"
        );
        assert!(read.set.get("ignored").is_none());
        let said: Vec<_> = read.warnings.iter().map(|w| w.message.as_str()).collect();
        assert!(said.iter().any(|m| m.contains("@media")), "{said:?}");
        assert_eq!(read.set.themes["Mode"], ["Light", "Dark"]);
    }

    #[test]
    fn what_cannot_be_read_is_a_warning_naming_the_token() {
        let css = r#"
:root { --a: #111111; --b: var(--missing); --c: #222222; --d: 4px; --loop1: var(--loop2); --loop2: var(--loop1); }
.dark { --c: 12px; --a: #333333; --only-dark: #444444; --d: var(--nope); }
.sepia { --a: #555555; }
:root, .also { --e: #666666; }
"#;
        let read = import(&CssVars, css);
        let said = |name: &str| {
            read.warnings
                .iter()
                .find(|w| w.token.as_deref() == Some(name))
                .map(|w| w.message.clone())
                .unwrap_or_else(|| panic!("{name}: {:?}", read.warnings))
        };
        assert!(said("b").contains("--missing"));
        assert!(
            said("c").contains("not a color"),
            "a mode value of another type"
        );
        assert!(said("only-dark").contains("no default"));
        assert!(said("d").contains("--nope"));
        assert!(said("loop1").contains("alias"));
        assert!(
            read.warnings
                .iter()
                .any(|w| w.token.is_none() && w.message.contains(".sepia"))
        );
        assert!(read.set.get("b").is_none() && read.set.get("only-dark").is_none());
        assert_eq!(
            read.set.get("a").unwrap().modes.len(),
            1,
            "the valid mode value is kept"
        );
        assert!(
            read.set.get("c").unwrap().modes.is_empty(),
            "the mismatched mode value is not"
        );
        assert!(read.set.get("e").is_some());
    }

    #[test]
    fn the_person_chooses_the_selectors() {
        let set = samples::token_set();
        let mut options = options(&CssVars, &set);
        options.mapping = options
            .mapping
            .with(&[ModeSelector::new("Mode", "Dark", "[data-theme='dark']")])
            .unwrap();
        let text = CssVars.export(&set, &options).unwrap().text;
        assert!(
            text.contains("[data-theme=\"dark\"] {\n  --color-brand-primary: #60A5FACC;\n}"),
            "{text}"
        );
        assert!(!text.contains(".dark"));
        assert_eq!(
            CssVars.import(&text, &options).unwrap().set.tokens,
            set.tokens
        );
        // The default mapping no longer knows that selector.
        let default = TokenOptions::for_themes(&set.themes);
        let read = CssVars.import(&text, &default).unwrap();
        assert!(
            read.warnings
                .iter()
                .any(|w| w.message.contains("data-theme"))
        );
        assert!(
            read.set
                .get("color/brand/primary")
                .unwrap()
                .modes
                .is_empty()
        );
    }

    #[test]
    fn a_stylesheet_that_does_not_end_is_an_error_on_its_line() {
        let error = CssVars
            .import(
                ":root {\n  --a: #111111;\n",
                &TokenOptions::for_themes(&BTreeMap::new()),
            )
            .unwrap_err();
        assert!(error.message.contains("never closed"), "{error}");
        assert_eq!(error.line, Some(1));
        let error = CssVars
            .import("a { b: \"open", &TokenOptions::for_themes(&BTreeMap::new()))
            .unwrap_err();
        assert!(error.message.contains("string"), "{error}");
    }

    #[test]
    fn names_a_stylesheet_cannot_carry_are_left_out_with_a_warning() {
        let mut set = samples::token_set();
        set.tokens.push(color("bad/*/name", "#000000"));
        // A comment cannot hold `*/`, so a stylesheet cannot carry the name.
        for format in [&CssVars as &dyn TokenFormat, &TailwindV4] {
            let out = format.export(&set, &options(format, &set)).unwrap();
            assert!(
                out.warnings
                    .iter()
                    .any(|w| w.token.as_deref() == Some("bad/*/name")),
                "{}: {:?}",
                format.id(),
                out.warnings
            );
            assert!(!out.text.contains("bad"), "{}", out.text);
        }
        // Two names that become one property are not both written.
        let set = TokenSet {
            themes: BTreeMap::new(),
            tokens: vec![color("a b", "#010101"), color("a_b", "#020202")],
        };
        let out = CssVars.export(&set, &options(&CssVars, &set)).unwrap();
        assert_eq!(out.warnings.len(), 1, "{:?}", out.warnings);
        assert_eq!(out.warnings[0].token.as_deref(), Some("a_b"));
    }

    #[test]
    fn dtcg_reads_the_spec_forms() {
        let file = json!({
            "brand": {
                "$type": "color",
                "primary": {"$value": {"colorSpace": "srgb", "components": [0.0, 0.5, 1.0], "alpha": 0.5}},
                "hex": {"$value": {"colorSpace": "display-p3", "components": [0, 0, 0], "hex": "#112233"}},
                "link": {"$value": "{brand.hex}", "$description": "an alias by path"},
                "dotted": {"a.b": {"$value": "#010101"}}
            },
            "size": {
                "$type": "dimension",
                "sm": {"$value": {"value": 0.5, "unit": "rem"}},
                "md": {"$value": "12px"},
                "weird": {"$value": "3vw"}
            },
            "type": {
                "family": {"$type": "fontFamily", "$value": ["Inter", "sans-serif"]},
                "weight": {"$type": "fontWeight", "$value": "bold"},
                "w2": {"$type": "fontWeight", "$value": 300},
                "speed": {"$type": "duration", "$value": {"value": 200, "unit": "ms"}},
                "n": {"$type": "number", "$value": 1.25},
                "untyped": {"$value": true},
                "shadow": {"$type": "shadow", "$value": {"color": "#000000"}}
            }
        });
        let read = import(&DtcgJson, &file.to_string());
        let get = |n: &str| {
            read.set
                .get(n)
                .unwrap_or_else(|| {
                    panic!(
                        "{n}: {:?}",
                        read.set.tokens.iter().map(|t| &t.name).collect::<Vec<_>>()
                    )
                })
                .value
                .clone()
        };
        assert_eq!(
            get("brand/primary"),
            TokenValue::Color(Color::rgba(0, 128, 255, 128))
        );
        assert_eq!(
            get("brand/hex"),
            TokenValue::Color(Color::rgb(0x11, 0x22, 0x33))
        );
        assert_eq!(get("brand/link"), TokenValue::Alias("brand/hex".into()));
        assert_eq!(get("size/sm"), TokenValue::Number(8.0));
        assert_eq!(get("size/md"), TokenValue::Number(12.0));
        assert_eq!(get("size/weird"), TokenValue::String("3vw".into()));
        assert_eq!(
            get("type/family"),
            TokenValue::String("Inter, sans-serif".into())
        );
        assert_eq!(get("type/weight"), TokenValue::Number(700.0));
        assert_eq!(get("type/w2"), TokenValue::Number(300.0));
        assert_eq!(get("type/speed"), TokenValue::String("200ms".into()));
        assert_eq!(get("type/n"), TokenValue::Number(1.25));
        assert_eq!(get("type/untyped"), TokenValue::Boolean(true));
        assert!(read.set.get("type/shadow").is_none());
        let said: Vec<_> = read.warnings.iter().map(|w| w.to_string()).collect();
        assert!(
            said.iter()
                .any(|m| m.contains("type/shadow") && m.contains("shadow")),
            "{said:?}"
        );
        assert!(
            said.iter().any(|m| m.contains("brand/dotted/a.b")),
            "a name with a dot is flagged: {said:?}"
        );
    }

    #[test]
    fn dtcg_names_with_characters_a_path_cannot_hold_survive() {
        let set = TokenSet {
            themes: BTreeMap::new(),
            tokens: vec![
                color("space/1.5", "#010203"),
                color("a/{b}", "#040506"),
                color("$x/y", "#070809"),
                Token {
                    name: "ref".into(),
                    kind: VariableType::Color,
                    value: TokenValue::Alias("space/1.5".into()),
                    modes: vec![],
                },
            ],
        };
        let text = export(&DtcgJson, &set);
        assert!(text.contains("{space.1_5}"), "{text}");
        let read = import(&DtcgJson, &text);
        let mut want = set.tokens.clone();
        want.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(read.set.tokens, want);
    }

    #[test]
    fn dtcg_cannot_hold_a_token_and_a_group_of_one_name() {
        let set = TokenSet {
            themes: BTreeMap::new(),
            tokens: vec![color("radius", "#010203"), color("radius/md", "#040506")],
        };
        let out = DtcgJson.export(&set, &options(&DtcgJson, &set)).unwrap();
        assert_eq!(out.warnings.len(), 1);
        assert_eq!(out.warnings[0].token.as_deref(), Some("radius/md"));
        let read = import(&DtcgJson, &out.text);
        assert_eq!(read.set.tokens.len(), 1);
    }

    #[test]
    fn dtcg_that_is_not_json_is_an_error_on_its_line() {
        let error = DtcgJson
            .import(
                "{\n  \"a\": \n",
                &TokenOptions::for_themes(&BTreeMap::new()),
            )
            .unwrap_err();
        assert!(error.line.is_some(), "{error}");
        let error = DtcgJson
            .import("[1]", &TokenOptions::for_themes(&BTreeMap::new()))
            .unwrap_err();
        assert!(error.message.contains("object"), "{error}");
    }

    #[test]
    fn a_default_mapping_is_the_formats_own() {
        let set = samples::token_set();
        assert_eq!(
            TailwindV4.default_options(&set.themes).namespaces,
            BTreeMap::from([("space".to_owned(), "spacing".to_owned())])
        );
        assert!(CssVars.default_options(&set.themes).namespaces.is_empty());
        assert_eq!(
            CssVars.default_options(&set.themes).mapping,
            ThemeMapping::derive(&set.themes)
        );
    }
}
