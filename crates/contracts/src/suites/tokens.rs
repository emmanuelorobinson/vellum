//! A token format writes the same text for the same set, reads back what it wrote, reports what it leaves out, and
//! does not invent tokens from text that holds none.
use vellum_core::ports::TokenFormat;
use vellum_core::tokens::{TokenOptions, TokenSet};

/// `set` is a set the format can hold in full: tokens of every type, an alias, and values under a non-default
/// theme value. `options` are the ones its `default_options` gives for the set's themes.
pub fn format(format: &dyn TokenFormat, set: &TokenSet, options: &TokenOptions) {
    assert!(
        !format.id().is_empty() && !format.label().is_empty() && !format.file_name().is_empty(),
        "TokenFormat: id, label and file name are not empty"
    );
    let first = format
        .export(set, options)
        .expect("TokenFormat: export of a set it can hold failed");
    let second = format
        .export(set, options)
        .expect("TokenFormat: export failed");
    assert_eq!(
        first.text, second.text,
        "TokenFormat: the same set gave different text"
    );
    assert!(
        first.warnings.is_empty(),
        "TokenFormat: a set it can hold in full gave warnings: {:?}",
        first.warnings
    );

    let read = format
        .import(&first.text, options)
        .expect("TokenFormat: it cannot read what it wrote");
    assert_eq!(
        read.set.tokens, set.tokens,
        "TokenFormat: reading what it wrote did not give the same tokens"
    );
    assert!(
        read.warnings.is_empty(),
        "TokenFormat: reading what it wrote gave warnings: {:?}",
        read.warnings
    );

    let empty = TokenSet::default();
    let text = format
        .export(&empty, options)
        .expect("TokenFormat: export of no tokens failed")
        .text;
    let read = format
        .import(&text, options)
        .expect("TokenFormat: it cannot read an empty file it wrote");
    assert!(
        read.set.tokens.is_empty(),
        "TokenFormat: tokens appeared from a file with none"
    );

    // Text that is not the format yields an error or nothing, never tokens it made up.
    for junk in ["", "\u{0}\u{1} not tokens {{{ ]]", "[1, 2, 3]", "{"] {
        if let Ok(read) = format.import(junk, options) {
            assert!(
                read.set.tokens.is_empty(),
                "TokenFormat: tokens made up from {junk:?}"
            );
        }
    }

    // A value it cannot write is reported and left out, not written as text that reads back as something else.
    let mut broken = set.clone();
    if let Some(token) = broken
        .tokens
        .iter_mut()
        .find(|t| matches!(t.value, vellum_core::tokens::TokenValue::Number(_)))
    {
        token.value = vellum_core::tokens::TokenValue::Number(f64::NAN);
        let name = token.name.clone();
        let out = format
            .export(&broken, options)
            .expect("TokenFormat: export with a number that is not finite failed outright");
        assert!(
            out.warnings
                .iter()
                .any(|w| w.token.as_deref() == Some(&name)),
            "TokenFormat: a number that is not finite was not reported"
        );
        assert!(
            !out.text.to_lowercase().contains("nan"),
            "TokenFormat: a number that is not finite was written"
        );
    }
}
