//! `FontSource`.

use vellum_core::ports::FontSource;
use vellum_types::FontStyle;

/// Fonts resolve by family, the same request gives the same handle, and an unknown family is none.
///
/// `known` and `other_known` are two different families the source has; `unknown` is one it does not.
pub fn run(source: &dyn FontSource, known: &str, other_known: &str, unknown: &str) {
    let regular = source
        .resolve(known, 400, FontStyle::Normal)
        .unwrap_or_else(|| panic!("FontSource: {known:?} should resolve"));
    assert_eq!(
        Some(regular),
        source.resolve(known, 400, FontStyle::Normal),
        "FontSource: the same request gave two handles"
    );
    let other = source
        .resolve(other_known, 400, FontStyle::Normal)
        .unwrap_or_else(|| panic!("FontSource: {other_known:?} should resolve"));
    assert_ne!(regular, other, "FontSource: two families share a handle");

    for (weight, style) in [
        (100, FontStyle::Normal),
        (700, FontStyle::Normal),
        (900, FontStyle::Italic),
        (400, FontStyle::Italic),
    ] {
        assert!(
            source.resolve(known, weight, style).is_some(),
            "FontSource: {known:?} at weight {weight} {style:?} should resolve to the nearest it has"
        );
    }

    for weight in [100, 400, 900] {
        assert_eq!(
            None,
            source.resolve(unknown, weight, FontStyle::Normal),
            "FontSource: the unknown family {unknown:?} resolved at weight {weight}"
        );
    }
}
