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
