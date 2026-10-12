//! `Codec`.

use vellum_core::ports::Codec;
use vellum_types::Document;

/// A codec writes the same bytes for the same document, reads them back, and refuses what is not a document.
///
/// `docs` are documents whose numbers already sit on the codec's grid, so reading what was written gives the
/// document back. Include one with every section filled in if the codec has to carry them all.
pub fn run(codec: &dyn Codec, docs: &[Document]) {
    assert!(!codec.id().is_empty(), "Codec: the id is empty");
    assert_eq!(
        codec.id(),
        codec.id(),
        "Codec: the id changes between calls"
    );
    assert!(
        !docs.is_empty(),
        "Codec: the suite needs at least one document"
    );

    for (n, doc) in docs.iter().enumerate() {
        let bytes = codec
            .encode(doc)
            .unwrap_or_else(|e| panic!("Codec: document {n} could not be written: {e}"));
        assert!(!bytes.is_empty(), "Codec: document {n} became no bytes");
        let again = codec.encode(doc).expect("the second write");
        assert_eq!(
            bytes, again,
            "Codec: document {n} gave different bytes twice"
        );

        let back = codec
            .decode(&bytes)
            .unwrap_or_else(|e| panic!("Codec: the bytes of document {n} could not be read: {e}"));
        assert_eq!(&back, doc, "Codec: document {n} changed in a round trip");

        let rewritten = codec.encode(&back).expect("the third write");
        assert_eq!(
            bytes, rewritten,
            "Codec: load then save changed the bytes of document {n} (Invariant 5)"
        );

        assert!(
            codec.decode(&bytes[..bytes.len() / 2]).is_err(),
            "Codec: half of document {n} was accepted as a whole"
        );
    }

    for (what, bytes) in [
        ("no bytes", &b""[..]),
        (
            "text that is not a document",
            &b"this is not a document"[..],
        ),
        ("an empty object", &b"{}"[..]),
        ("random bytes", &[0xff, 0xfe, 0x00, 0x9c, 0x01][..]),
    ] {
        assert!(codec.decode(bytes).is_err(), "Codec: {what} was accepted");
    }
}
