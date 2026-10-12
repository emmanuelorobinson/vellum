use std::collections::BTreeMap;
use std::sync::Mutex;

use vellum_core::ports::{AssetError, AssetHash, AssetStore};

/// Assets in a map. The hash is two FNV-1a passes over the bytes: the same bytes always give the same hash and
/// different test bytes give different ones, but it is not collision resistant like a real content hash.
#[derive(Debug, Default)]
pub struct InMemoryAssets {
    stored: Mutex<BTreeMap<AssetHash, Vec<u8>>>,
}

impl InMemoryAssets {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

fn fnv(bytes: &[u8], offset: u64) -> u64 {
    bytes.iter().fold(offset, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn hash_of(bytes: &[u8]) -> AssetHash {
    let a = fnv(bytes, 0xcbf2_9ce4_8422_2325);
    let b = fnv(bytes, 0x8422_2325_cbf2_9ce4) ^ (bytes.len() as u64);
    AssetHash::new(format!("{a:016x}{b:016x}")).expect("hex is a valid hash")
}

fn valid_mime(mime: &str) -> bool {
    let mut parts = mime.split('/');
    let (Some(kind), Some(subtype), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    [kind, subtype]
        .iter()
        .all(|p| !p.is_empty() && !p.chars().any(char::is_whitespace))
}

impl AssetStore for InMemoryAssets {
    fn get(&self, hash: &AssetHash) -> Result<Vec<u8>, AssetError> {
        let stored = self
            .stored
            .lock()
            .map_err(|_| AssetError::Unavailable("poisoned".into()))?;
        stored
            .get(hash)
            .cloned()
            .ok_or_else(|| AssetError::NotFound(hash.clone()))
    }

    fn put(&self, bytes: &[u8], mime: &str) -> Result<AssetHash, AssetError> {
        if !valid_mime(mime) {
            return Err(AssetError::InvalidMime(mime.to_owned()));
        }
        let hash = hash_of(bytes);
        let mut stored = self
            .stored
            .lock()
            .map_err(|_| AssetError::Unavailable("poisoned".into()))?;
        stored.entry(hash.clone()).or_insert_with(|| bytes.to_vec());
        Ok(hash)
    }
}
