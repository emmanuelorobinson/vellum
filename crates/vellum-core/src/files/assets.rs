//! Which assets a document uses, and the one moment unused ones may go.

use std::collections::BTreeSet;

use serde_json::Value;
use vellum_types::{AssetId, Document};

use crate::ports::{AssetCollect, AssetError, AssetHash};

/// What the writer is doing. Only an explicit save may remove assets: an autosave runs while the person can
/// still undo, and an asset only the undo history uses must survive it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    /// Saved in the background.
    Autosave,
    /// The person asked to save (⌘S, the command, the CLI).
    Explicit,
}

/// What a save found in the asset store.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GcReport {
    /// Hashes removed because nothing uses them.
    pub removed: Vec<AssetHash>,
    /// How many assets are in the store after the save.
    pub kept: usize,
    /// Ids the document uses that the store does not hold, so an image or font is broken. Ids that are not
    /// content hashes (a fixture's `a0000001`) cannot be in a store and are left out.
    pub missing: Vec<AssetId>,
}

/// The asset a `src` path names, when it points into an `<name>.assets/` folder: `app.assets/<hash>`, with any
/// folders above. A shader or script path elsewhere belongs to no store.
pub fn asset_in_path(src: &str) -> Option<AssetId> {
    let mut parts = src.rsplit('/');
    let name = parts.next()?;
    let folder = parts.next()?;
    if !folder.ends_with(".assets") || folder.len() == ".assets".len() {
        return None;
    }
    let hash = AssetHash::new(name).ok()?;
    AssetId::new(hash.as_str()).ok()
}

/// Every asset the document uses: image nodes and image paints wherever they sit (pages, components, styles),
/// custom fonts, and `src` paths that point into an assets folder.
///
/// It reads the document as JSON and takes every `asset` field, so a place that gains an image later is covered
/// without a change here. Over-keeping is the safe error, and the only one this can make.
pub fn referenced_assets(doc: &Document) -> BTreeSet<AssetId> {
    let tree = serde_json::to_value(doc).expect("a document serializes");
    let mut found = BTreeSet::new();
    if let Some(fonts) = tree.get("fonts").and_then(Value::as_object) {
        found.extend(fonts.keys().filter_map(|k| AssetId::new(k.as_str()).ok()));
    }
    walk(&tree, &mut found);
    found
}

fn walk(value: &Value, found: &mut BTreeSet<AssetId>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                match (key.as_str(), inner) {
                    ("asset", Value::String(id)) => found.extend(AssetId::new(id.as_str()).ok()),
                    ("src", Value::String(path)) => found.extend(asset_in_path(path)),
                    _ => {}
                }
                walk(inner, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| walk(item, found)),
        _ => {}
    }
}

/// Run after a save. Reports what is missing always; removes what nothing uses only for [`SaveKind::Explicit`].
///
/// `also_keep` is for the caller's undo history: the assets of nodes a person could still bring back.
pub fn collect_on_save(
    kind: SaveKind,
    store: &dyn AssetCollect,
    doc: &Document,
    also_keep: &BTreeSet<AssetId>,
) -> Result<GcReport, AssetError> {
    let used = referenced_assets(doc);
    let held = store.list()?;
    let held_set: BTreeSet<&AssetHash> = held.iter().collect();
    let missing = used
        .iter()
        .filter(|id| AssetHash::new(id.as_str()).is_ok_and(|hash| !held_set.contains(&hash)))
        .cloned()
        .collect();

    let mut removed = Vec::new();
    if kind == SaveKind::Explicit {
        for hash in &held {
            let wanted = |id: &AssetId| id.as_str() == hash.as_str();
            if used.iter().any(wanted) || also_keep.iter().any(wanted) {
                continue;
            }
            match store.remove(hash) {
                Ok(()) | Err(AssetError::NotFound(_)) => removed.push(hash.clone()),
                Err(other) => return Err(other),
            }
        }
    }
    Ok(GcReport {
        kept: held.len() - removed.len(),
        removed,
        missing,
    })
}
