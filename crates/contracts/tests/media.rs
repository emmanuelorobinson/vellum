//! TEST DATA. Shared image/Figma contracts over deterministic fakes.
use vellum_contracts::{fakes::*, suites::media::*};
#[test]
fn fakes_meet_media_contracts() {
    image_ops(&MemoryImageOps, b"TEST DATA");
    segmentation(&MemorySegmentation);
    generator(&MemoryGenerator(vellum_core::ports::ImageBytes {
        mime: "image/png".into(),
        bytes: b"TEST DATA".to_vec(),
    }));
    figma(&MemoryFigma);
}

#[test]
fn binary_http_fake_bounds_responses_and_blocks_plain_http() {
    use vellum_core::ports::*;
    let h = MemoryMediaHttp(FetchResponse {
        status: 200,
        content_type: "image/png".into(),
        body: b"TEST DATA".to_vec(),
    });
    let mut r = MediaRequest {
        url: "https://example.com".into(),
        method: MediaMethod::Get,
        body: vec![],
        credential: None,
        max_bytes: 100,
    };
    assert!(h.exchange(&r).is_ok());
    r.max_bytes = 1;
    assert_eq!(h.exchange(&r), Err(FetchError::TooLarge));
    r.url = "http://example.com".into();
    assert_eq!(h.exchange(&r), Err(FetchError::Blocked));
}
