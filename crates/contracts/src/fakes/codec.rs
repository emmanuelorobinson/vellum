use vellum_core::ports::{Codec, CodecError, Position};
use vellum_types::Document;

/// The types' own serde JSON, pretty-printed: deterministic because every map in a document is sorted. It does
/// not round numbers, migrate or locate errors in the text as the real codec does.
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeJsonCodec;

impl Codec for FakeJsonCodec {
    fn id(&self) -> &'static str {
        "fake-json"
    }

    fn decode(&self, bytes: &[u8]) -> Result<Document, CodecError> {
        serde_json::from_slice(bytes).map_err(|e| {
            if e.is_syntax() || e.is_eof() {
                CodecError::Syntax {
                    position: Position {
                        line: e.line().max(1),
                        column: e.column().max(1),
                    },
                    message: e.to_string(),
                }
            } else {
                CodecError::Invalid {
                    path: String::new(),
                    position: None,
                    message: e.to_string(),
                }
            }
        })
    }

    fn encode(&self, doc: &Document) -> Result<Vec<u8>, CodecError> {
        serde_json::to_vec_pretty(doc).map_err(|e| CodecError::Invalid {
            path: String::new(),
            position: None,
            message: e.to_string(),
        })
    }
}
