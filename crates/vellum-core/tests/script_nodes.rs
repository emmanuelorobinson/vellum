//! TEST DATA. Header parser regression/property corpus; no runtime or I/O required.
use proptest::prelude::*;
use vellum_core::script_nodes::Header;
#[test]
fn script_nodes_header_grammar_refuses_invalid_or_ambiguous_declarations() {
    for source in [
        "// @schema 2",
        "// @schema 1\n// @schema 1",
        "// @input x: wat = 1",
        "// @input x: number(min=3,max=2) = 3",
        "// @input x: number(step=0) = 1",
        "// @input x: number = true",
        "// @input x: string = 2",
        "// @input x: boolean = 1",
        "// @input x: color = \"bad\"",
        "// @input x: enum(\"a\",\"a\") = \"a\"",
        "// @input x: enum(\"a\") = \"b\"",
        "// @input x: enum() = \"a\"",
        "// @input __proto__: string = \"bad\"",
        "// @input x: number = 1\n// @input x: number = 2",
        "// @input x: string = \"unclosed",
        "/* @schema 1",
        "// @input x: number(unknown=1) = 1",
    ] {
        assert!(Header::parse(source).is_err(), "{source}");
    }
}
#[test]
fn script_nodes_header_handles_quoted_delimiters_and_stops_at_code() {
    let header=Header::parse("/** @schema 1 */\n// @input label: string(multiline=true) = \"a, (b) = c\"\n// @input mode: enum(options=[\"x,y\",\"z\"]) = \"x,y\"\nreturn '// @input ignored: number = 2';").unwrap();
    assert_eq!(header.inputs.len(), 2);
    assert!(
        Header::parse("return 1;\n// @input ignored: invalid")
            .unwrap()
            .inputs
            .is_empty()
    );
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]
    #[test]
    fn script_nodes_header_fuzz_smoke(source in ".{0,2048}") { let _=Header::parse(&source); }
    #[test]
    fn script_nodes_header_declaration_fuzz_smoke(source in ".{0,2048}") { let _=Header::parse(&format!("/*\n * @input {source}\n */")); }
}

#[test]
fn script_nodes_line_comment_default_can_contain_block_terminator() {
    let header = Header::parse("// @input text: string = \"a*/b\"\nreturn [];").unwrap();
    assert_eq!(
        header.inputs[0].default,
        Some(vellum_types::Scalar::Text("a*/b".into()))
    );
}

#[test]
fn script_nodes_long_line_excerpt_includes_the_error_column() {
    let source = format!("{}throw new Error('TEST DATA');", " ".repeat(1000));
    let diag = vellum_core::ports::ScriptDiagnostic::new(
        vellum_core::ports::ScriptError::Timeout,
        &source,
        Some((1, 1001)),
    );
    assert!(diag.excerpt.contains("throw new Error"));
    assert!(diag.excerpt.len() < 600);
}
