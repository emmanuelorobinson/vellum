//! Codec: deterministic single-file `.vellum` JSON (guide Prompt 9, Invariant 5).
//!
//! The same document always becomes the same bytes: keys in the order the types declare them (maps sorted),
//! 2-space indent, LF, a trailing newline, and numbers rounded to a fixed step. Rounding happens here and
//! nowhere else, so live state keeps every digit it was given. Loading and saving a canonical file gives the
//! file back.
//!
//! Reading is more forgiving than writing: key order, whitespace, CRLF and a byte order mark are accepted and
//! normalised away. Unknown fields are not accepted (DESIGN S05); the error names the path, line and column.
//! A file from an older format is migrated step by step (`Migrations`); one from a newer format is
//! `CodecError::NewerFormat` (S04).

mod decode;
mod locate;
mod migrate;
mod print;
mod quantize;
mod tree;

use vellum_types::Document;

pub use migrate::{Migration, MigrationError, Migrations, RegistryError};
pub use quantize::{EXACT_NUMBER_KEYS, Quantum, quantize, quantum_of};
pub use vellum_core::ports::{Codec, CodecError, Position};

/// The Cargo package name, used by the smoke test and by diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The single-file JSON codec.
pub struct JsonCodec {
    migrations: Migrations,
}

impl Default for JsonCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonCodec {
    /// Reads every format up to [`FORMAT_VERSION`] and writes that one.
    pub fn new() -> Self {
        Self::with_migrations(Migrations::standard())
    }

    /// A codec that reads and writes the versions `migrations` reaches. For tests of the registry.
    pub fn with_migrations(migrations: Migrations) -> Self {
        Self { migrations }
    }

    /// The codec's name in the registry of codecs.
    pub fn id(&self) -> &'static str {
        "json"
    }

    /// The canonical bytes of `doc`.
    ///
    /// Fails if a number is not finite or the document is not in this codec's format version.
    pub fn encode(&self, doc: &Document) -> Result<Vec<u8>, CodecError> {
        if doc.meta.format_version != self.migrations.current() {
            return Err(CodecError::WrongVersion {
                found: doc.meta.format_version,
                expected: self.migrations.current(),
            });
        }
        let mut tree = tree::to_tree(doc)?;
        let changed = round(&mut tree)?;
        if changed {
            // A value that rounds to its type's default is left out by the type, and the type judged the
            // unrounded value. Read back what would be written and write that, so saving what was loaded is
            // saving the same bytes.
            let settled: Document =
                serde_json::from_slice(&print::print(&tree)).map_err(|e| CodecError::Invalid {
                    path: String::new(),
                    position: None,
                    message: e.to_string(),
                })?;
            tree = tree::to_tree(&settled)?;
            round(&mut tree)?;
        }
        Ok(print::print(&tree))
    }

    /// The document in `bytes`, migrated to the current version.
    pub fn decode(&self, bytes: &[u8]) -> Result<Document, CodecError> {
        decode::decode(&self.migrations, bytes)
    }
}

fn round(tree: &mut tree::Tree) -> Result<bool, CodecError> {
    quantize::round_tree(tree).map_err(|quantize::NotFinite(path)| CodecError::NotFinite { path })
}

/// [`JsonCodec::encode`] with the standard registry.
pub fn encode(doc: &Document) -> Result<Vec<u8>, CodecError> {
    JsonCodec::new().encode(doc)
}

/// [`JsonCodec::decode`] with the standard registry.
pub fn decode(bytes: &[u8]) -> Result<Document, CodecError> {
    JsonCodec::new().decode(bytes)
}

impl Codec for JsonCodec {
    fn id(&self) -> &'static str {
        JsonCodec::id(self)
    }

    fn decode(&self, bytes: &[u8]) -> Result<Document, CodecError> {
        JsonCodec::decode(self, bytes)
    }

    fn encode(&self, doc: &Document) -> Result<Vec<u8>, CodecError> {
        JsonCodec::encode(self, doc)
    }
}
