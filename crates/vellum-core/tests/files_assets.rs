//! Which assets a document uses, and when unused ones may be removed (guide Prompt 24).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use vellum_core::files::{SaveKind, asset_in_path, collect_on_save, referenced_assets};
use vellum_core::ports::{AssetCollect, AssetError, AssetHash, AssetStore};
use vellum_types::{AssetId, Document, FontFace, FontStyle};

const CONTENT: &str = include_str!("../../../fixtures/documents/content.vellum");

fn hash(n: u8) -> AssetHash {
    AssetHash::new(format!("{n:02x}").repeat(32)).expect("hex")
}

fn id(n: u8) -> AssetId {
    AssetId::new(hash(n).as_str()).expect("an asset id")
}

/// A store that is only a list of hashes, enough for collection rules.
#[derive(Default)]
struct Listed {
    held: Mutex<BTreeMap<AssetHash, Vec<u8>>>,
    removals: Mutex<usize>,
}

impl Listed {
    fn with(hashes: &[AssetHash]) -> Self {
        let store = Self::default();
        for h in hashes {
            store.held.lock().unwrap().insert(h.clone(), vec![1]);
        }
        store
    }
    fn hashes(&self) -> Vec<AssetHash> {
        self.held.lock().unwrap().keys().cloned().collect()
    }
}

impl AssetStore for Listed {
    fn get(&self, hash: &AssetHash) -> Result<Vec<u8>, AssetError> {
        self.held
            .lock()
            .unwrap()
            .get(hash)
            .cloned()
            .ok_or_else(|| AssetError::NotFound(hash.clone()))
    }
    fn put(&self, _bytes: &[u8], _mime: &str) -> Result<AssetHash, AssetError> {
        unreachable!("collection never writes")
    }
}

impl AssetCollect for Listed {
    fn list(&self) -> Result<Vec<AssetHash>, AssetError> {
        Ok(self.hashes())
    }
    fn remove(&self, hash: &AssetHash) -> Result<(), AssetError> {
        *self.removals.lock().unwrap() += 1;
        self.held
            .lock()
            .unwrap()
            .remove(hash)
            .map(|_| ())
            .ok_or_else(|| AssetError::NotFound(hash.clone()))
    }
}

#[test]
fn an_image_node_and_an_image_fill_both_count() {
    let doc: Document = serde_json::from_str(CONTENT).expect("the fixture");
    let used = referenced_assets(&doc);
    assert_eq!(
        used.iter().map(|a| a.as_str()).collect::<Vec<_>>(),
        vec!["a0000001", "fnt000001"],
        "content.vellum uses one image, as a node and as a paint, and carries one font"
    );
}

#[test]
fn a_document_with_nothing_uses_nothing() {
    assert!(referenced_assets(&Document::new("TEST DATA: empty")).is_empty());
}

#[test]
fn custom_fonts_count_even_when_no_text_uses_them() {
    let mut doc = Document::new("TEST DATA: fonts");
    doc.fonts.insert(
        id(7),
        FontFace {
            family: "TEST DATA Sans".into(),
            weight: 400,
            style: FontStyle::Normal,
        },
    );
    assert_eq!(referenced_assets(&doc), BTreeSet::from([id(7)]));
}

#[test]
fn a_shader_counts_only_when_it_points_into_an_assets_folder() {
    let h = hash(9);
    assert_eq!(
        asset_in_path(&format!("app.assets/{h}")).map(|a| a.to_string()),
        Some(h.to_string())
    );
    assert_eq!(
        asset_in_path(&format!("../shared/app.assets/{h}")).map(|a| a.to_string()),
        Some(h.to_string())
    );
    for not_an_asset in [
        "shaders/glow.sksl",
        "scripts/chart.js",
        &format!("assets/{h}"),
        "app.assets/not-a-hash",
        "app.assets",
        "",
    ] {
        assert_eq!(
            asset_in_path(not_an_asset),
            None,
            "{not_an_asset:?} was taken for an asset"
        );
    }
}

#[test]
fn autosave_never_removes_anything() {
    let doc = Document::new("TEST DATA: unused");
    let store = Listed::with(&[hash(1), hash(2)]);
    let report = collect_on_save(SaveKind::Autosave, &store, &doc, &BTreeSet::new()).unwrap();
    assert!(report.removed.is_empty());
    assert_eq!(report.kept, 2);
    assert_eq!(*store.removals.lock().unwrap(), 0);
    assert_eq!(store.hashes().len(), 2, "GC only on explicit save");
}

#[test]
fn an_explicit_save_removes_what_nothing_uses_and_keeps_the_rest() {
    let mut doc = Document::new("TEST DATA: one font");
    doc.fonts.insert(
        id(1),
        FontFace {
            family: "TEST DATA Sans".into(),
            weight: 400,
            style: FontStyle::Normal,
        },
    );
    let store = Listed::with(&[hash(1), hash(2), hash(3)]);
    let report = collect_on_save(SaveKind::Explicit, &store, &doc, &BTreeSet::from([id(3)]))
        .expect("collect");
    assert_eq!(report.removed, vec![hash(2)]);
    assert_eq!(
        report.kept, 2,
        "the font and the one the caller asked to keep"
    );
    assert_eq!(store.hashes(), vec![hash(1), hash(3)]);
}

#[test]
fn a_used_asset_that_is_not_in_the_store_is_reported_not_invented() {
    let mut doc = Document::new("TEST DATA: missing");
    doc.fonts.insert(
        id(4),
        FontFace {
            family: "TEST DATA Sans".into(),
            weight: 400,
            style: FontStyle::Normal,
        },
    );
    let store = Listed::with(&[hash(5)]);
    for kind in [SaveKind::Autosave, SaveKind::Explicit] {
        let report = collect_on_save(kind, &store, &doc, &BTreeSet::new()).unwrap();
        assert_eq!(report.missing, vec![id(4)], "{kind:?}");
    }
}

#[test]
fn an_id_that_is_not_a_hash_is_never_reported_missing() {
    // content.vellum uses `a0000001` (valid hex, so it could be stored) and `fnt000001` (it could not).
    let doc: Document = serde_json::from_str(CONTENT).unwrap();
    let store = Listed::with(&[hash(1)]);
    let report = collect_on_save(SaveKind::Explicit, &store, &doc, &BTreeSet::new()).unwrap();
    assert_eq!(
        report.removed,
        vec![hash(1)],
        "nothing in the document uses it"
    );
    assert_eq!(
        report
            .missing
            .iter()
            .map(|a| a.as_str())
            .collect::<Vec<_>>(),
        vec!["a0000001"],
        "only an id that could be a stored hash can be missing from the store"
    );
}
