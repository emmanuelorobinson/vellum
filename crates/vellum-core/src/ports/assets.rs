//! Assets (images, fonts) by content hash. Today: a local folder and S3-compatible storage.

use std::fmt;

/// The content hash of an asset: lowercase hexadecimal, 1 to 128 characters. The store computes it; the
/// algorithm is the store's business, but the same bytes always give the same hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetHash(String);

/// Why a string is not an asset hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetHashError;

impl fmt::Display for AssetHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an asset hash is 1 to 128 lowercase hexadecimal characters")
    }
}

impl std::error::Error for AssetHashError {}

impl AssetHash {
    /// Checks `hash` and wraps it.
    pub fn new(hash: impl Into<String>) -> Result<Self, AssetHashError> {
        let hash = hash.into();
        let valid = !hash.is_empty()
            && hash.len() <= 128
            && hash
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if valid {
            Ok(Self(hash))
        } else {
            Err(AssetHashError)
        }
    }

    /// The hash as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Why an asset could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    /// Nothing is stored under this hash.
    NotFound(AssetHash),
    /// The media type is not `type/subtype`.
    InvalidMime(String),
    /// The store could not do it (disk full, offline, permission).
    Unavailable(String),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(hash) => write!(f, "No asset is stored under {hash}."),
            Self::InvalidMime(mime) => write!(f, "{mime:?} is not a media type such as image/png."),
            Self::Unavailable(why) => write!(f, "The asset store is not available: {why}"),
        }
    }
}

impl std::error::Error for AssetError {}

/// Assets by content hash. Putting the same bytes twice gives the same hash and stores them once.
pub trait AssetStore {
    /// The bytes stored under `hash`.
    fn get(&self, hash: &AssetHash) -> Result<Vec<u8>, AssetError>;
    /// Stores `bytes` and returns their hash. `mime` is `type/subtype`.
    fn put(&self, bytes: &[u8], mime: &str) -> Result<AssetHash, AssetError>;
}

/// Checks that `mime` is `type/subtype`: two non-empty parts, no whitespace. Every store applies the same rule.
pub fn check_mime(mime: &str) -> Result<(), AssetError> {
    let mut parts = mime.split('/');
    let valid = match (parts.next(), parts.next(), parts.next()) {
        (Some(kind), Some(subtype), None) => [kind, subtype]
            .iter()
            .all(|p| !p.is_empty() && !p.chars().any(char::is_whitespace)),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(AssetError::InvalidMime(mime.to_owned()))
    }
}

/// A store that can say what it holds and drop what a document no longer uses (guide Prompt 24).
///
/// Separate from [`AssetStore`] because a store behind a network (S3) may offer neither. Nothing calls `remove`
/// but [`crate::files::collect_on_save`], and that only on an explicit save.
pub trait AssetCollect: AssetStore {
    /// Every hash in the store, sorted.
    fn list(&self) -> Result<Vec<AssetHash>, AssetError>;
    /// Removes one asset. A hash that is not there is `NotFound`.
    fn remove(&self, hash: &AssetHash) -> Result<(), AssetError>;
}
