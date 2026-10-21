//! TEST DATA. No Chrome needed: validate before any process or network operation.
use vellum_browser_cdp::{authorized_origins, permits_url};
use vellum_core::ports::BrowserRequest;
fn request(url: &str) -> BrowserRequest {
    BrowserRequest {
        url: url.into(),
        origins: vec![],
        width: 320,
        height: 640,
        selector: None,
        scroll: [0., 0.],
        zoom: 1.,
        snapshot: true,
    }
}
#[test]
fn localhost_supported_but_credentials_files_and_origins_with_paths_rejected() {
    let r = request("http://localhost:3000/card");
    let origins = authorized_origins(&r).unwrap();
    assert!(permits_url(&origins, "http://localhost:3000/a.png"));
    assert!(!permits_url(&origins, "http://localhost:4000/"));
    assert!(!permits_url(&origins, "https://other.invalid/"));
    assert!(!permits_url(&origins, "file:///etc/passwd"));
    assert!(!permits_url(&origins, "http://user:pass@localhost:3000"));
    for url in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "http://user:pass@localhost/",
        "invalid",
    ] {
        assert!(authorized_origins(&request(url)).is_err());
    }
    let mut bad = r;
    bad.origins = vec!["https://cdn.invalid/path".into()];
    assert!(authorized_origins(&bad).is_err());
}
#[test]
fn bounded_viewport_zoom_and_selector() {
    for size in [0, 4097, u32::MAX] {
        let mut r = request("http://localhost/");
        r.width = size;
        assert!(authorized_origins(&r).is_err());
    }
    let mut r = request("http://localhost/");
    r.zoom = f64::NAN;
    assert!(authorized_origins(&r).is_err());
}
