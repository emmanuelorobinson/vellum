//! Older formats stay readable forever (Invariant 5). A migration turns the JSON of one version into the JSON
//! of the next; the registry runs them in order, and a file with a newer version is a typed error. Every
//! migration ships a before and an after fixture.
//!
//! The real registry has no steps yet (FORMAT_VERSION is 1). The machinery is exercised with a made-up chain,
//! `example-migrations/`, whose versions 1 to 3 never existed.

use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};
use vellum_codec_json::{
    CodecError, JsonCodec, Migration, MigrationError, Migrations, RegistryError,
};
use vellum_types::FORMAT_VERSION;

/// Renames `meta.title` to `meta.name`.
struct TitleBecomesName;

impl Migration for TitleBecomesName {
    fn source_version(&self) -> u32 {
        1
    }

    fn migrate(&self, document: &mut Json) -> Result<(), MigrationError> {
        let meta = document["meta"]
            .as_object_mut()
            .ok_or_else(|| MigrationError::new("there is no meta block"))?;
        let title = meta
            .remove("title")
            .ok_or_else(|| MigrationError::new("a version 1 file has a title"))?;
        meta.insert("name".to_owned(), title);
        Ok(())
    }
}

/// Renames each page's `label` to `name`.
struct LabelBecomesName;

impl Migration for LabelBecomesName {
    fn source_version(&self) -> u32 {
        2
    }

    fn migrate(&self, document: &mut Json) -> Result<(), MigrationError> {
        for page in document["pages"].as_array_mut().into_iter().flatten() {
            if let Some(label) = page.as_object_mut().and_then(|page| page.remove("label")) {
                page["name"] = label;
            }
        }
        Ok(())
    }
}

/// Does what it should and forgets to bump the version, which the registry owns.
struct Forgetful;

impl Migration for Forgetful {
    fn source_version(&self) -> u32 {
        1
    }

    fn migrate(&self, _: &mut Json) -> Result<(), MigrationError> {
        Ok(())
    }
}

struct Failing;

impl Migration for Failing {
    fn source_version(&self) -> u32 {
        1
    }

    fn migrate(&self, _: &mut Json) -> Result<(), MigrationError> {
        Err(MigrationError::new("the page order is missing"))
    }
}

fn example_chain() -> Migrations {
    Migrations::new(
        3,
        vec![Box::new(TitleBecomesName), Box::new(LabelBecomesName)],
    )
    .expect("steps 1 and 2 reach version 3")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn json_of(path: &Path) -> Json {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The contract every migration meets: `before.vellum` at its version, `after.vellum` at the next, and the
/// step turns one into the other.
fn check_fixtures(migrations: &Migrations, root: &Path) {
    for step in migrations.steps() {
        let from = step.source_version();
        let dir = root.join(format!("v{from}-to-v{}", from + 1));
        let mut before = json_of(&dir.join("before.vellum"));
        let after = json_of(&dir.join("after.vellum"));
        assert_eq!(
            before["meta"]["formatVersion"],
            json!(from),
            "{}",
            dir.display()
        );
        assert_eq!(
            after["meta"]["formatVersion"],
            json!(from + 1),
            "{}",
            dir.display()
        );
        Migrations::apply_step(step.as_ref(), &mut before)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        assert_eq!(
            before,
            after,
            "{} does not turn before into after",
            dir.display()
        );
    }
}

fn repo_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/migrations")
}

// --- the registry -------------------------------------------------------------------------------

#[test]
fn the_standard_registry_reaches_the_version_this_build_writes() {
    let migrations = Migrations::standard();
    assert_eq!(migrations.current(), FORMAT_VERSION);
    assert_eq!(migrations.steps().len() as u32, FORMAT_VERSION - 1);
    for (index, step) in migrations.steps().iter().enumerate() {
        assert_eq!(
            step.source_version(),
            index as u32 + 1,
            "steps run 1→2, 2→3, and so on"
        );
    }
}

#[test]
fn every_migration_in_the_standard_registry_ships_a_before_and_an_after_fixture() {
    check_fixtures(&Migrations::standard(), &repo_fixtures());
}

#[test]
fn the_example_chain_ships_fixtures_and_each_step_turns_before_into_after() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/example-migrations");
    check_fixtures(&example_chain(), &root);
}

#[test]
fn a_registry_with_a_gap_or_a_missing_step_is_refused_when_it_is_built() {
    let skipped = Migrations::new(
        4,
        vec![Box::new(TitleBecomesName), Box::new(LabelBecomesName)],
    );
    assert!(
        matches!(skipped, Err(RegistryError::MissingStep { from: 3 })),
        "{skipped:?}"
    );

    let out_of_order = Migrations::new(
        3,
        vec![Box::new(LabelBecomesName), Box::new(TitleBecomesName)],
    );
    assert!(
        matches!(
            out_of_order,
            Err(RegistryError::OutOfOrder {
                expected: 1,
                found: 2
            })
        ),
        "{out_of_order:?}"
    );

    let twice = Migrations::new(3, vec![Box::new(TitleBecomesName), Box::new(Forgetful)]);
    assert!(
        matches!(twice, Err(RegistryError::OutOfOrder { .. })),
        "{twice:?}"
    );

    let extra = Migrations::new(
        2,
        vec![Box::new(TitleBecomesName), Box::new(LabelBecomesName)],
    );
    assert!(
        matches!(extra, Err(RegistryError::PastCurrent { .. })),
        "{extra:?}"
    );

    assert!(Migrations::new(0, vec![]).is_err(), "versions start at 1");
}

// --- reading an older file ----------------------------------------------------------------------

#[test]
fn an_old_file_is_migrated_step_by_step_and_decoded_at_the_current_version() {
    let codec = JsonCodec::with_migrations(example_chain());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/example-migrations");
    let doc = codec
        .decode(read(&root.join("v1-to-v2/before.vellum")).as_bytes())
        .expect("a version 1 file reads");
    assert_eq!(doc.meta.format_version, 3);
    assert_eq!(doc.meta.name, "TEST DATA migration");
    assert_eq!(doc.pages[0].name, "Canvas");

    // The same document as written at version 3 decodes to the same thing.
    let current = codec
        .decode(read(&root.join("v2-to-v3/after.vellum")).as_bytes())
        .expect("a version 3 file reads");
    assert_eq!(doc, current);
}

#[test]
fn a_file_from_the_middle_of_the_chain_runs_only_the_steps_after_it() {
    let codec = JsonCodec::with_migrations(example_chain());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/example-migrations");
    let doc = codec
        .decode(read(&root.join("v2-to-v3/before.vellum")).as_bytes())
        .expect("a version 2 file reads");
    assert_eq!(doc.pages[0].name, "Canvas");
}

#[test]
fn the_codec_writes_the_registrys_current_version_and_nothing_older() {
    let codec = JsonCodec::with_migrations(example_chain());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/example-migrations");
    let doc = codec
        .decode(read(&root.join("v1-to-v2/before.vellum")).as_bytes())
        .expect("reads");
    let written = codec.encode(&doc).expect("writes");
    assert_eq!(
        written,
        read(&root.join("v2-to-v3/after.vellum")).as_bytes()
    );
    assert!(
        matches!(
            JsonCodec::new().encode(&doc),
            Err(CodecError::WrongVersion { found: 3, .. })
        ),
        "this build writes version {FORMAT_VERSION}"
    );
}

#[test]
fn the_registry_sets_the_version_after_each_step_so_a_step_cannot_forget_to() {
    let migrations = Migrations::new(2, vec![Box::new(Forgetful)]).expect("one step");
    let mut document =
        json!({"meta": {"formatVersion": 1, "name": "x", "createdWith": "y"}, "pages": []});
    migrations.upgrade(&mut document).expect("upgrades");
    assert_eq!(document["meta"]["formatVersion"], json!(2));
}

#[test]
fn a_step_that_fails_is_a_typed_error_naming_the_version_it_was_leaving() {
    let migrations = Migrations::new(2, vec![Box::new(Failing)]).expect("one step");
    let codec = JsonCodec::with_migrations(migrations);
    let text = r#"{"meta": {"formatVersion": 1, "name": "x", "createdWith": "y"}, "pages": []}"#;
    match codec.decode(text.as_bytes()) {
        Err(CodecError::Migration { from, message }) => {
            assert_eq!(from, 1);
            assert!(message.contains("page order"), "{message}");
        }
        other => panic!("{other:?}"),
    }
    let message = codec
        .decode(text.as_bytes())
        .expect_err("fails")
        .to_string();
    assert!(message.contains("format 1"), "{message}");
}

#[test]
fn a_migrated_file_that_is_still_wrong_is_reported_by_path_without_a_position() {
    // After the migration the JSON is not the user's text any more, so there is no line to point at.
    let codec = JsonCodec::with_migrations(example_chain());
    let text = r#"{"meta": {"formatVersion": 1, "title": "x", "createdWith": "y"},
        "pages": [{"id": "page000001", "label": "Canvas", "bogus": 1}]}"#;
    match codec.decode(text.as_bytes()) {
        Err(error @ CodecError::Invalid { .. }) => {
            assert_eq!(error.path(), Some("pages[0]"));
            assert_eq!(error.position(), None);
            assert!(error.to_string().contains("bogus"), "{error}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_file_newer_than_the_registry_is_a_newer_format_error_even_for_a_longer_chain() {
    let codec = JsonCodec::with_migrations(example_chain());
    let text = r#"{"meta": {"formatVersion": 4, "name": "x", "createdWith": "y"}, "pages": []}"#;
    assert!(matches!(
        codec.decode(text.as_bytes()),
        Err(CodecError::NewerFormat {
            found: 4,
            supported: 3
        })
    ));
}
