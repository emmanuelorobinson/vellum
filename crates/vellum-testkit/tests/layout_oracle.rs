//! The layout oracle loader (guide Prompt 6): expected boxes written by `scripts/gen-layout-oracle.sh`
//! from headless Chrome load, save and load again unchanged, and comparisons use a 0.5 px tolerance
//! (Invariant 7).

use std::collections::BTreeMap;
use std::path::PathBuf;

use vellum_testkit::layout_oracle::{
    LayoutBox, OracleError, OracleFixture, TOLERANCE_PX, compare, load_dir,
};

/// The shape the generator writes: keys sorted, integers without a fraction, Chrome's 1/64 px
/// fractions as they come.
const ROW_FILL_THIRDS: &str = r#"{
  "fixture": "row_fill_thirds",
  "root": {
    "box": { "height": 20, "width": 100, "x": 0, "y": 0 },
    "children": [
      {
        "box": { "height": 20, "width": 33.328125, "x": 0, "y": 0 },
        "children": [],
        "id": "0.0",
        "props": { "height": 20, "width": "fill" }
      },
      {
        "box": { "height": 20, "width": 33.34375, "x": 33.328125, "y": 0 },
        "children": [],
        "id": "0.1",
        "props": { "height": 20, "width": "fill" }
      },
      {
        "box": null,
        "children": [],
        "id": "0.2",
        "props": { "height": 20, "visible": false, "width": 10 }
      }
    ],
    "id": "0",
    "props": { "height": 20, "layout": "row", "width": 100 }
  },
  "title": "TEST DATA: three fill children split a fixed row"
}
"#;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vellum-layout-oracle-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn tolerance_is_half_a_pixel() {
    assert_eq!(TOLERANCE_PX, 0.5);
}

#[test]
fn parses_the_generator_format() {
    let fixture = OracleFixture::from_json(ROW_FILL_THIRDS).unwrap();
    assert_eq!(fixture.fixture, "row_fill_thirds");
    assert_eq!(fixture.root.id, "0");
    assert_eq!(fixture.root.props["layout"], "row");
    assert_eq!(fixture.root.children.len(), 3);
    let second = &fixture.root.children[1];
    assert_eq!(
        second.layout_box,
        Some(LayoutBox {
            x: 33.328125,
            y: 0.0,
            width: 33.34375,
            height: 20.0
        })
    );
    assert_eq!(
        fixture.root.children[2].layout_box, None,
        "hidden nodes have no box"
    );
    let ids: Vec<_> = fixture.nodes().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["0", "0.0", "0.1", "0.2"]);
}

#[test]
fn round_trips_expected_boxes() {
    let fixture = OracleFixture::from_json(ROW_FILL_THIRDS).unwrap();
    let saved = fixture.to_json();
    let again = OracleFixture::from_json(&saved).unwrap();
    assert_eq!(again, fixture);
    assert_eq!(again.to_json(), saved, "saving is deterministic");
}

#[test]
fn rejects_unknown_fields_and_bad_boxes() {
    let extra = ROW_FILL_THIRDS.replacen("\"fixture\"", "\"surprise\": 1, \"fixture\"", 1);
    assert!(matches!(
        OracleFixture::from_json(&extra),
        Err(OracleError::Json(_))
    ));
    let negative = ROW_FILL_THIRDS.replacen("\"width\": 100, \"x\"", "\"width\": -1, \"x\"", 1);
    assert!(matches!(
        OracleFixture::from_json(&negative),
        Err(OracleError::Invalid(_))
    ));
    let dup = ROW_FILL_THIRDS.replacen("\"id\": \"0.1\"", "\"id\": \"0.0\"", 1);
    assert!(matches!(
        OracleFixture::from_json(&dup),
        Err(OracleError::Invalid(_))
    ));
}

#[test]
fn compare_passes_within_tolerance_and_reports_beyond_it() {
    let fixture = OracleFixture::from_json(ROW_FILL_THIRDS).unwrap();
    let third = 100.0 / 3.0;
    let mut actual = BTreeMap::from([
        ("0".to_string(), LayoutBox::new(0.0, 0.0, 100.0, 20.0)),
        ("0.0".to_string(), LayoutBox::new(0.0, 0.0, third, 20.0)),
        ("0.1".to_string(), LayoutBox::new(third, 0.0, third, 20.0)),
        // Hidden in the oracle: whatever the engine reports is not compared.
        ("0.2".to_string(), LayoutBox::new(0.0, 0.0, 0.0, 0.0)),
    ]);
    assert!(compare(&fixture, &actual, TOLERANCE_PX).is_empty());

    actual.insert(
        "0.1".to_string(),
        LayoutBox::new(third + 0.6, 0.0, third, 20.0),
    );
    actual.remove("0.0");
    let mismatches = compare(&fixture, &actual, TOLERANCE_PX);
    assert_eq!(mismatches.len(), 2, "{mismatches:?}");
    assert_eq!(mismatches[0].id, "0.0");
    assert_eq!(mismatches[0].actual, None, "a missing node is a mismatch");
    assert_eq!(mismatches[1].id, "0.1");
    assert!(mismatches[1].to_string().contains("x"));
    assert!((mismatches[1].max_delta - 0.6052).abs() < 1e-3);
}

#[test]
fn load_dir_reads_every_expected_file_sorted() {
    let dir = scratch("load-dir");
    let b = ROW_FILL_THIRDS.replace("row_fill_thirds", "b_fixture");
    let a = ROW_FILL_THIRDS.replace("row_fill_thirds", "a_fixture");
    std::fs::write(dir.join("b_fixture.json"), b).unwrap();
    std::fs::write(dir.join("a_fixture.json"), a).unwrap();
    std::fs::write(dir.join("notes.txt"), "ignored").unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        r#"{ "chrome": "155.0.8059.39", "fixtures": ["a_fixture", "b_fixture"], "tolerancePx": 0.5 }"#,
    )
    .unwrap();
    let fixtures = load_dir(&dir).unwrap();
    let names: Vec<_> = fixtures.iter().map(|f| f.fixture.as_str()).collect();
    assert_eq!(names, ["a_fixture", "b_fixture"]);

    std::fs::write(
        dir.join("c_fixture.json"),
        ROW_FILL_THIRDS.replace("row_fill_thirds", "not_c"),
    )
    .unwrap();
    assert!(
        matches!(load_dir(&dir), Err(OracleError::Invalid(m)) if m.contains("c_fixture")),
        "a file must be named after its fixture"
    );
}
