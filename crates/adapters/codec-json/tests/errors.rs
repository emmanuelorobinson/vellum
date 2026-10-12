//! A file that cannot be read (DESIGN S05) says where: the path to the field, the line and the column, in
//! plain words. A file from a newer version (S04) is a typed error of its own, never a parse failure.

use vellum_codec_json::{CodecError, Position, decode};
use vellum_types::FORMAT_VERSION;

const CANONICAL: &str = r#"{
  "meta": {
    "formatVersion": 1,
    "name": "TEST DATA",
    "createdWith": "Vellum 0.1.0"
  },
  "pages": [
    {
      "id": "page000001",
      "name": "Canvas",
      "children": [
        {
          "type": "rect",
          "id": "r0000001",
          "name": "Back",
          "width": 10,
          "height": 10
        }
      ]
    }
  ]
}
"#;

/// Where `needle` first starts in `text`: 1-based line and column, the column counted in characters.
fn position_of(text: &str, needle: &str) -> Position {
    let offset = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} is not in {text}"));
    position_at(text, offset)
}

fn position_at(text: &str, offset: usize) -> Position {
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |at| at + 1);
    Position {
        line,
        column: text[line_start..offset].chars().count() + 1,
    }
}

fn failure(text: &str) -> CodecError {
    decode(text.as_bytes()).expect_err("the file should be refused")
}

/// `CANONICAL` with `line` added after the rect's `height`.
fn with_extra_in_rect(line: &str) -> String {
    CANONICAL.replace(
        "\"height\": 10\n",
        &format!("\"height\": 10,\n          {line}\n"),
    )
}

// --- unknown fields -----------------------------------------------------------------------------

#[test]
fn an_unknown_field_in_a_node_is_found_by_path_line_and_column() {
    let text = with_extra_in_rect("\"colour\": \"#FF0000\"");
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "\"colour\"")));
    assert!(
        error
            .path()
            .is_some_and(|p| p.starts_with("pages[0].children[0]")),
        "{error:?}"
    );
    assert!(error.to_string().contains("colour"), "{error}");
}

#[test]
fn an_unknown_field_at_the_top_of_the_document_is_found() {
    let text = CANONICAL.replacen("  \"pages\"", "  \"layers\": [],\n  \"pages\"", 1);
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "\"layers\"")));
    assert_eq!(error.path(), Some(""));
}

#[test]
fn an_unknown_field_in_the_meta_block_is_found() {
    let text = CANONICAL.replacen(
        "\"createdWith\": \"Vellum 0.1.0\"",
        "\"createdWith\": \"Vellum 0.1.0\",\n    \"author\": \"x\"",
        1,
    );
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "\"author\"")));
    assert_eq!(error.path(), Some("meta"));
}

#[test]
fn an_unknown_field_in_a_nested_child_is_found() {
    let text = CANONICAL.replacen(
        "\"height\": 10\n        }",
        "\"height\": 10,\n          \"children\": [\n            {\"type\": \"rect\", \"id\": \"r0000002\", \"name\": \"Inner\", \"width\": 1, \"height\": 1, \"bogus\": 1}\n          ]\n        }",
        1,
    );
    // A rect has no children of its own, so make the parent a frame.
    let text = text.replacen(
        "\"type\": \"rect\",\n          \"id\": \"r0000001\"",
        "\"type\": \"frame\",\n          \"id\": \"r0000001\"",
        1,
    );
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "\"bogus\"")));
}

#[test]
fn an_unknown_field_inside_a_paint_inside_a_node_is_found() {
    let text = with_extra_in_rect(
        "\"fills\": [{\"type\": \"solid\", \"color\": \"#FF0000\", \"colur\": 1}]",
    );
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "\"colur\"")));
}

#[test]
fn an_unknown_field_in_a_gradient_stop_is_found_and_a_valid_sibling_is_not_blamed() {
    let text = with_extra_in_rect(
        "\"fills\": [{\"type\": \"gradient\", \"kind\": \"linear\", \"stops\": [{\"color\": \"#000000\", \"position\": 0, \"stray\": 1}]}]",
    );
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "\"stray\"")));

    // `position` is valid on a stop; an unknown `position` on a paint is the one to blame.
    let text = with_extra_in_rect(
        "\"fills\": [{\"type\": \"gradient\", \"kind\": \"linear\", \"stops\": [{\"color\": \"#000000\", \"position\": 0}], \"position\": 3}]",
    );
    let error = failure(&text);
    assert_eq!(
        error.position(),
        Some(position_of(&text, "\"position\": 3"))
    );
}

#[test]
fn an_unknown_field_in_a_component_is_found_by_its_path() {
    let text = CANONICAL.replacen(
        "  \"pages\"",
        "  \"components\": {\"comp000001\": {\"name\": \"Card\", \"nmae\": 1, \"root\": {\"type\": \"frame\", \"id\": \"card00001\", \"name\": \"Card\", \"width\": 1, \"height\": 1}}},\n  \"pages\"",
        1,
    );
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "\"nmae\"")));
    assert_eq!(error.path(), Some("components.comp000001"));
}

#[test]
fn the_column_counts_characters_not_bytes() {
    let text = concat!(
        r#"{"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"x"},"#,
        r#""pages":[{"id":"page000001","name":"日本語日本語","bogus":1}]}"#
    );
    let error = failure(text);
    assert_eq!(error.position(), Some(position_of(text, "\"bogus\"")));
}

// --- wrong shapes -------------------------------------------------------------------------------

#[test]
fn a_wrong_type_points_at_the_value() {
    let text = CANONICAL.replacen("\"width\": 10", "\"width\": \"wide\"", 1);
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "\"wide\"")));
    assert_eq!(error.path(), Some("pages[0].children[0].width"));
}

#[test]
fn a_wrong_type_in_a_plain_struct_points_at_the_value() {
    let text = CANONICAL.replacen("\"name\": \"Canvas\"", "\"name\": 7", 1);
    let error = failure(&text);
    assert_eq!(error.position(), Some(position_of(&text, "7,")));
    assert_eq!(error.path(), Some("pages[0].name"));
}

#[test]
fn a_missing_field_points_at_the_object_that_lacks_it() {
    let text = CANONICAL.replacen("          \"name\": \"Back\",\n", "", 1);
    let error = failure(&text);
    assert!(error.to_string().contains("name"), "{error}");
    assert_eq!(error.path(), Some("pages[0].children[0]"));
    assert_eq!(
        error.position(),
        Some(position_of(&text, "{\n          \"type\": \"rect\""))
    );
}

#[test]
fn an_unknown_node_type_points_at_the_type() {
    let text = CANONICAL.replacen("\"rect\"", "\"retc\"", 1);
    let error = failure(&text);
    assert!(error.to_string().contains("retc"), "{error}");
    assert_eq!(error.position(), Some(position_of(&text, "\"retc\"")));
}

#[test]
fn a_bad_id_says_why_and_where() {
    let text = CANONICAL.replacen("\"r0000001\"", "\"a/b\"", 1);
    let error = failure(&text);
    assert!(error.to_string().contains('/'), "{error}");
    assert_eq!(error.position(), Some(position_of(&text, "\"a/b\"")));
}

#[test]
fn a_props_override_with_a_property_no_node_has_says_which() {
    let text = CANONICAL.replacen(
        "  \"pages\"",
        "  \"components\": {\"comp000001\": {\"name\": \"Card\", \"root\": {\"type\": \"frame\", \"id\": \"card00001\", \"name\": \"Card\", \"width\": 1, \"height\": 1}, \"variants\": [{\"properties\": {\"State\": \"hover\"}, \"overrides\": {\"card00001\": {\"props\": {\"colour\": 1}}}}]}},\n  \"pages\"",
        1,
    );
    let error = failure(&text);
    assert!(error.to_string().contains("colour"), "{error}");
    assert!(error.position().is_some());
}

#[test]
fn the_message_is_plain_words_with_the_line_and_the_column() {
    let text = with_extra_in_rect("\"colour\": 1");
    let position = position_of(&text, "\"colour\"");
    let message = failure(&text).to_string();
    assert!(
        message.contains(&format!("line {}", position.line))
            && message.contains(&format!("column {}", position.column)),
        "{message}"
    );
    assert!(message.contains("pages[0].children[0]"), "{message}");
}

// --- syntax -------------------------------------------------------------------------------------

#[test]
fn broken_json_is_a_syntax_error_with_a_line_and_a_column() {
    let text = CANONICAL.replacen("\"height\": 10\n", "\"height\": 10,\n", 1);
    match failure(&text) {
        CodecError::Syntax { position, message } => {
            assert!(!message.is_empty());
            assert!(
                (16..=20).contains(&position.line),
                "the trailing comma is on line 17: {position:?}"
            );
            assert!(position.column >= 1);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_truncated_file_is_a_syntax_error() {
    let cut = &CANONICAL[..CANONICAL.len() / 2];
    assert!(matches!(failure(cut), CodecError::Syntax { .. }));
}

#[test]
fn an_empty_file_is_a_syntax_error_on_line_one() {
    for text in ["", "   "] {
        match failure(text) {
            CodecError::Syntax { position, .. } => assert_eq!(position.line, 1, "{text:?}"),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn text_after_the_document_is_a_syntax_error() {
    let text = format!("{CANONICAL}{{}}");
    assert!(matches!(failure(&text), CodecError::Syntax { .. }));
}

#[test]
fn bytes_that_are_not_utf_8_are_refused_not_guessed() {
    let mut bytes = CANONICAL.as_bytes().to_vec();
    let at = bytes
        .windows(4)
        .position(|w| w == b"Back")
        .expect("present");
    bytes[at] = 0xFF;
    assert!(decode(&bytes).is_err());
}

#[test]
fn a_git_conflict_marker_is_a_syntax_error_not_a_crash() {
    let text = CANONICAL.replacen(
        "\"width\": 10,",
        "<<<<<<< HEAD\n\"width\": 10,\n=======\n\"width\": 12,\n>>>>>>> branch",
        1,
    );
    assert!(matches!(failure(&text), CodecError::Syntax { .. }));
}

#[test]
fn a_document_nested_past_the_parser_limit_is_refused_not_a_stack_overflow() {
    let deep = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
    assert!(decode(deep.as_bytes()).is_err());
    let object = format!(
        "{}{}",
        "{\"a\":".repeat(5000),
        "1".to_owned() + &"}".repeat(5000)
    );
    assert!(decode(object.as_bytes()).is_err());
}

// --- shape of the file --------------------------------------------------------------------------

#[test]
fn a_document_that_is_not_an_object_is_refused() {
    for text in ["[]", "42", "\"text\"", "null", "true"] {
        assert!(
            matches!(failure(text), CodecError::Invalid { .. }),
            "{text}"
        );
    }
}

#[test]
fn a_file_without_a_meta_block_or_a_version_says_so() {
    let error = failure(r#"{"pages": []}"#);
    assert!(error.to_string().contains("meta"), "{error}");
    let error = failure(r#"{"meta": {"name": "x", "createdWith": "y"}, "pages": []}"#);
    assert!(error.to_string().contains("formatVersion"), "{error}");
    assert_eq!(error.path(), Some("meta"));
}

#[test]
fn a_version_that_is_not_a_whole_number_of_at_least_one_is_refused() {
    for version in [
        "0",
        "-1",
        "1.5",
        "\"1\"",
        "null",
        "true",
        "[]",
        "4294967296",
    ] {
        let text = format!(
            r#"{{"meta": {{"formatVersion": {version}, "name": "x", "createdWith": "y"}}, "pages": []}}"#
        );
        let error = failure(&text);
        assert!(
            matches!(error, CodecError::Invalid { .. }),
            "{version}: {error:?}"
        );
        assert_eq!(error.path(), Some("meta.formatVersion"), "{version}");
    }
}

// --- a newer file (S04) -------------------------------------------------------------------------

#[test]
fn a_newer_version_is_a_typed_error_and_not_a_parse_failure() {
    // Everything but the version could be unknown to this build.
    let text = r#"{
  "meta": {"formatVersion": 3, "name": "x", "createdWith": "Vellum 9", "sharding": true},
  "pages": [{"id": "page000001", "name": "Canvas", "layout3d": {"depth": 4}}],
  "futureSection": [1, 2, 3]
}"#;
    match failure(text) {
        CodecError::NewerFormat { found, supported } => {
            assert_eq!(found, 3);
            assert_eq!(supported, FORMAT_VERSION);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_newer_version_is_named_in_the_words_the_banner_uses() {
    let text = r#"{"meta": {"formatVersion": 3, "name": "x", "createdWith": "y"}, "pages": []}"#;
    let message = failure(text).to_string();
    assert!(message.contains("newer version of Vellum"), "{message}");
    assert!(message.contains("format 3"), "{message}");
}

// --- tolerated on the way in --------------------------------------------------------------------

#[test]
fn a_byte_order_mark_and_windows_line_endings_are_read_and_not_written_back() {
    let windows = format!("\u{FEFF}{}", CANONICAL.replace('\n', "\r\n"));
    let doc = decode(windows.as_bytes()).expect("a BOM and CRLF are only whitespace");
    let bytes = vellum_codec_json::encode(&doc).expect("encode");
    assert_eq!(
        bytes,
        CANONICAL.as_bytes(),
        "the canonical form has neither"
    );
}

#[test]
fn positions_in_a_file_with_a_byte_order_mark_count_from_the_first_character() {
    let text = with_extra_in_rect("\"colour\": 1");
    let expected = position_of(&text, "\"colour\"");
    let with_bom = format!("\u{FEFF}{text}");
    assert_eq!(failure(&with_bom).position(), Some(expected));
}

// --- keys that appear twice ---------------------------------------------------------------------

#[test]
fn a_key_that_appears_twice_in_a_map_is_refused_where_serde_would_keep_the_last() {
    // What a textual merge of two branches that both added the same variable can leave behind.
    let text = CANONICAL.replacen(
        "  \"pages\"",
        "  \"variables\": {\n    \"va000001\": {\"type\": \"boolean\", \"name\": \"a\", \"values\": []},\n    \"va000001\": {\"type\": \"boolean\", \"name\": \"b\", \"values\": []}\n  },\n  \"pages\"",
        1,
    );
    let error = failure(&text);
    let second = text.rfind("\"va000001\"").expect("present");
    assert_eq!(error.position(), Some(position_at(&text, second)));
    assert_eq!(error.path(), Some("variables"));
    assert!(
        error.to_string().contains("va000001") && error.to_string().contains("twice"),
        "{error}"
    );
}

#[test]
fn a_key_that_appears_twice_in_a_struct_is_refused_too() {
    let text = CANONICAL.replacen(
        "\"width\": 10,",
        "\"width\": 10,\n          \"width\": 12,",
        1,
    );
    assert!(matches!(failure(&text), CodecError::Invalid { .. }));
}
