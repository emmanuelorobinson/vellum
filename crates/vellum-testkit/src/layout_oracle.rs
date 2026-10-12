//! The Chrome layout oracle (guide Prompt 6, D2, Invariant 7). `scripts/gen-layout-oracle.sh` lays
//! out the fixtures in `fixtures/layout-oracle/` in headless Chrome and writes one JSON file of
//! expected boxes per fixture to `fixtures/layout-oracle/expected/`. This module loads those files and
//! compares an engine's boxes against them with a [`TOLERANCE_PX`] tolerance.
//!
//! Boxes are border boxes in CSS pixels, relative to the parent's border box (the root's to the page).
//! `props` holds the node's Vellum sizing semantics as the fixture wrote them; a layout engine maps
//! them itself (ADR-002 has the mapping). Hidden nodes have no box and are never compared.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Largest allowed difference, in CSS pixels, on any edge of any box (Invariant 7).
pub const TOLERANCE_PX: f64 = 0.5;

/// A border box relative to the parent's border box.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
// Fields in key order, so the JSON matches the generator's sorted keys.
pub struct LayoutBox {
    pub height: f64,
    pub width: f64,
    pub x: f64,
    pub y: f64,
}

impl LayoutBox {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The largest absolute difference between the two boxes' x, y, width and height.
    pub fn max_delta(&self, other: &LayoutBox) -> f64 {
        [
            (self.x - other.x).abs(),
            (self.y - other.y).abs(),
            (self.width - other.width).abs(),
            (self.height - other.height).abs(),
        ]
        .into_iter()
        .fold(0.0, f64::max)
    }
}

/// One node of a fixture: its id (an index path such as `0.2.1`), its Vellum props and Chrome's box.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleNode {
    #[serde(rename = "box")]
    pub layout_box: Option<LayoutBox>,
    pub children: Vec<OracleNode>,
    pub id: String,
    pub props: BTreeMap<String, serde_json::Value>,
}

/// One expected-boxes file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleFixture {
    pub fixture: String,
    pub root: OracleNode,
    pub title: String,
}

#[derive(Debug)]
pub enum OracleError {
    Io(PathBuf, std::io::Error),
    Json(serde_json::Error),
    Invalid(String),
}

impl fmt::Display for OracleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OracleError::Io(path, e) => write!(f, "{}: {e}", path.display()),
            OracleError::Json(e) => write!(f, "invalid oracle JSON: {e}"),
            OracleError::Invalid(m) => write!(f, "invalid oracle fixture: {m}"),
        }
    }
}

impl std::error::Error for OracleError {}

impl OracleFixture {
    pub fn from_json(text: &str) -> Result<Self, OracleError> {
        let fixture: OracleFixture = serde_json::from_str(text).map_err(OracleError::Json)?;
        fixture.validate()?;
        Ok(fixture)
    }

    /// Pretty JSON with sorted keys and a trailing newline; the same fixture always gives the same bytes.
    pub fn to_json(&self) -> String {
        let mut text =
            serde_json::to_string_pretty(self).expect("oracle fixtures always serialize");
        text.push('\n');
        text
    }

    pub fn load(path: &Path) -> Result<Self, OracleError> {
        let text = fs::read_to_string(path).map_err(|e| OracleError::Io(path.to_path_buf(), e))?;
        Self::from_json(&text)
    }

    /// Every node, depth first, parents before children.
    pub fn nodes(&self) -> impl Iterator<Item = &OracleNode> {
        let mut stack = vec![&self.root];
        std::iter::from_fn(move || {
            let node = stack.pop()?;
            stack.extend(node.children.iter().rev());
            Some(node)
        })
    }

    fn validate(&self) -> Result<(), OracleError> {
        let mut seen = BTreeSet::new();
        for node in self.nodes() {
            if !seen.insert(node.id.as_str()) {
                return Err(OracleError::Invalid(format!(
                    "{}: duplicate node id {}",
                    self.fixture, node.id
                )));
            }
            if let Some(b) = node.layout_box {
                let finite = [b.x, b.y, b.width, b.height].iter().all(|v| v.is_finite());
                if !finite || b.width < 0.0 || b.height < 0.0 {
                    return Err(OracleError::Invalid(format!(
                        "{}: node {} has a bad box {b:?}",
                        self.fixture, node.id
                    )));
                }
            }
        }
        Ok(())
    }
}

/// The generator's run record (Chrome version, fixture list) in the expected directory; not a fixture.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Every `*.json` file in `dir` except [`MANIFEST_FILE`], sorted by name. Each file must be named
/// after its fixture.
pub fn load_dir(dir: &Path) -> Result<Vec<OracleFixture>, OracleError> {
    let entries = fs::read_dir(dir).map_err(|e| OracleError::Io(dir.to_path_buf(), e))?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| OracleError::Io(dir.to_path_buf(), e))?
            .path();
        if path.extension().is_some_and(|e| e == "json")
            && path.file_name().is_some_and(|n| n != MANIFEST_FILE)
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let fixture = OracleFixture::load(path)?;
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if stem != fixture.fixture {
                return Err(OracleError::Invalid(format!(
                    "{} holds fixture {:?}",
                    path.display(),
                    fixture.fixture
                )));
            }
            Ok(fixture)
        })
        .collect()
}

/// A node whose computed box is missing or further than the tolerance from Chrome's.
#[derive(Clone, Debug, PartialEq)]
pub struct Mismatch {
    pub id: String,
    pub expected: LayoutBox,
    pub actual: Option<LayoutBox>,
    pub max_delta: f64,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let e = self.expected;
        match self.actual {
            None => write!(f, "node {}: no box computed (expected {e:?})", self.id),
            Some(a) => {
                let mut parts = Vec::new();
                for (name, ev, av) in [
                    ("x", e.x, a.x),
                    ("y", e.y, a.y),
                    ("width", e.width, a.width),
                    ("height", e.height, a.height),
                ] {
                    if (ev - av).abs() > 1e-9 {
                        parts.push(format!("{name} {av} (Chrome {ev})"));
                    }
                }
                write!(f, "node {}: {}", self.id, parts.join(", "))
            }
        }
    }
}

/// Compares `actual` boxes (keyed by node id) with the oracle. Nodes without an oracle box (hidden)
/// are skipped; a visible node missing from `actual` is a mismatch. Results follow tree order.
pub fn compare(
    expected: &OracleFixture,
    actual: &BTreeMap<String, LayoutBox>,
    tolerance: f64,
) -> Vec<Mismatch> {
    expected
        .nodes()
        .filter_map(|node| {
            let want = node.layout_box?;
            match actual.get(&node.id) {
                None => Some(Mismatch {
                    id: node.id.clone(),
                    expected: want,
                    actual: None,
                    max_delta: f64::INFINITY,
                }),
                Some(got) => {
                    let delta = want.max_delta(got);
                    (delta > tolerance).then(|| Mismatch {
                        id: node.id.clone(),
                        expected: want,
                        actual: Some(*got),
                        max_delta: delta,
                    })
                }
            }
        })
        .collect()
}
