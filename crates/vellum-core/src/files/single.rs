//! The single-file format as a `DocumentFiles`: one `Codec`, one file, one atomic write.

use vellum_types::Document;

use super::atomic_write;
use crate::ports::{Codec, DocumentFiles, FileError, FileSystem, Loaded, Saved};

/// A document in one file, written by `codec`.
pub struct SingleFile<'a> {
    codec: &'a dyn Codec,
}

impl<'a> SingleFile<'a> {
    /// The single-file format over `codec`.
    pub fn new(codec: &'a dyn Codec) -> Self {
        Self { codec }
    }
}

impl DocumentFiles for SingleFile<'_> {
    fn id(&self) -> &'static str {
        self.codec.id()
    }

    fn read(&self, fs: &dyn FileSystem, path: &str) -> Result<Loaded, FileError> {
        let document = self.codec.decode(&fs.read(path)?)?;
        Ok(Loaded {
            document,
            orphans: Vec::new(),
            interrupted: false,
        })
    }

    fn write(&self, fs: &dyn FileSystem, path: &str, doc: &Document) -> Result<Saved, FileError> {
        let bytes = self.codec.encode(doc)?;
        if fs.read(path).is_ok_and(|current| current == bytes) {
            return Ok(Saved {
                unchanged: 1,
                ..Saved::default()
            });
        }
        atomic_write(fs, path, &bytes)?;
        Ok(Saved {
            written: vec![path.to_owned()],
            ..Saved::default()
        })
    }
}
