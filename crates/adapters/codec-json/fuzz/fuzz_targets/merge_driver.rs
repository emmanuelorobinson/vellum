//! Byte admission and decode/check/merge/encode path used by the Git driver; no filesystem writes.
#![no_main]
use vellum_codec_json::{decode, encode};
libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let Some((choice, data)) = data.split_first() else {
        return;
    };
    let fixture = include_bytes!("../../../../../fixtures/documents/minimal.vellum");
    let (a, b, c) = if choice & 1 == 0 {
        let at = data.len() / 3;
        (&data[..at], &data[at..2 * at], &data[2 * at..])
    } else {
        (fixture.as_slice(), data, fixture.as_slice())
    };
    let (Ok(base), Ok(ours), Ok(theirs)) = (decode(a), decode(b), decode(c)) else {
        return;
    };
    if [&base, &ours, &theirs]
        .iter()
        .any(|doc| !vellum_core::ops::check_document(doc).is_empty())
    {
        return;
    }
    if let Ok(merged) = vellum_core::merge::merge(&base, &ours, &theirs) {
        assert!(vellum_core::ops::check_document(&merged.document).is_empty());
        let bytes = encode(&merged.document).unwrap();
        assert_eq!(encode(&decode(&bytes).unwrap()).unwrap(), bytes);
    }
});
