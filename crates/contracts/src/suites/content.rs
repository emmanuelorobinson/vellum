//! Listing is stable and unique; every listed item reads, unknown items fail, no fabricated completeness.
use std::collections::BTreeSet;
use vellum_core::ports::{SkillSource, StyleLibrary};

pub fn skills(source: &dyn SkillSource) {
    let list = source.list().expect("SkillSource: list failed");
    assert_eq!(list, source.list().unwrap(), "SkillSource: unstable list");
    let mut paths = BTreeSet::new();
    for meta in list {
        assert!(
            paths.insert(meta.path.clone()),
            "SkillSource: duplicate path"
        );
        assert!(
            !meta.name.trim().is_empty() && !meta.description.trim().is_empty(),
            "SkillSource: metadata is empty"
        );
        let text = source
            .read(&meta.path)
            .expect("SkillSource: listed path cannot be read");
        assert!(!text.trim().is_empty(), "SkillSource: body is empty");
        assert_eq!(
            text,
            source.read(&meta.path).unwrap(),
            "SkillSource: unstable body"
        );
    }
    assert!(
        source.read("__unregistered_contract_test_path__").is_err(),
        "SkillSource: fabricated missing content"
    );
}

pub fn styles(source: &dyn StyleLibrary) {
    let list = source.list().expect("StyleLibrary: list failed");
    assert_eq!(list, source.list().unwrap(), "StyleLibrary: unstable list");
    let mut names = BTreeSet::new();
    for meta in list {
        assert!(
            names.insert(meta.name.clone()),
            "StyleLibrary: duplicate name"
        );
        let style = source
            .read(&meta.name)
            .expect("StyleLibrary: listed name cannot be read");
        assert_eq!(style.name, meta.name, "StyleLibrary: wrong name");
        assert_eq!(
            style.description, meta.description,
            "StyleLibrary: wrong description"
        );
        assert!(
            !style.name.is_empty()
                && !style.description.trim().is_empty()
                && !style.palette.is_empty(),
            "StyleLibrary: empty recipe"
        );
        assert!(
            style.radius.is_finite() && style.radius >= 0.0,
            "StyleLibrary: radius is invalid"
        );
        assert!(
            !style.typography.body.is_empty()
                && !style.typography.heading.is_empty()
                && !style.typography.mono.is_empty(),
            "StyleLibrary: missing font preference"
        );
        assert!(
            [
                style.typography.base_size,
                style.typography.scale,
                style.density.gap,
                style.density.padding,
                style.density.control_height
            ]
            .iter()
            .all(|n| n.is_finite() && *n > 0.0),
            "StyleLibrary: sizes must be positive and finite"
        );
        assert_eq!(
            style,
            source.read(&meta.name).unwrap(),
            "StyleLibrary: unstable recipe"
        );
    }
    assert!(
        source.read("__unregistered_contract_test_name__").is_err(),
        "StyleLibrary: fabricated missing style"
    );
}
