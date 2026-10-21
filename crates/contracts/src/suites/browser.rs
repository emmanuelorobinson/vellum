//! Shared by the fake and Chrome lane. Call with a reachable fixture URL and exact device dimensions.
use vellum_core::ports::{BrowserError, BrowserPort, BrowserRequest};
pub fn browser(port: &dyn BrowserPort, request: &BrowserRequest) {
    let capture = port.capture(request).expect("BrowserPort: capture");
    assert!(
        capture.png.starts_with(b"\x89PNG\r\n\x1a\n"),
        "BrowserPort: PNG bytes"
    );
    assert!(
        capture.width > 0 && capture.height > 0,
        "BrowserPort: nonempty pixels"
    );
    if request.selector.is_none() {
        assert_eq!(
            (capture.width, capture.height),
            (request.width, request.height),
            "BrowserPort: device viewport"
        );
    }
    if request.snapshot {
        let dom = capture.dom.expect("BrowserPort: requested DOM");
        assert_eq!(dom.version, 1, "BrowserPort: schema revision");
        assert!(!dom.nodes.is_empty(), "BrowserPort: computed nodes");
    }
    let mut invalid = request.clone();
    invalid.width = 0;
    assert_eq!(
        port.capture(&invalid).err(),
        Some(BrowserError::InvalidRequest),
        "BrowserPort: reject invalid size before launching"
    );
}
