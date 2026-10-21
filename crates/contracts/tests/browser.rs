//! TEST DATA.
use vellum_contracts::{fakes::MemoryBrowser, suites::browser::browser};
use vellum_core::ports::*;
#[test]
fn fake_passes_contract() {
    let r = BrowserRequest {
        url: "http://localhost/".into(),
        origins: vec![],
        width: 320,
        height: 640,
        selector: None,
        scroll: [0., 0.],
        zoom: 1.,
        snapshot: true,
    };
    let port=MemoryBrowser{requests:Default::default(),response:BrowserCapture{png:b"\x89PNG\r\n\x1a\n".to_vec(),width:320,height:640,dom:Some(serde_json::from_value(serde_json::json!({"version":1,"width":320,"height":640,"nodes":[{"kind":"frame","name":"body","x":0,"y":0,"width":320,"height":640}]})).unwrap())}};
    browser(&port, &r);
    assert_eq!(port.requests.borrow().len(), 1);
}
#[test]
#[should_panic(expected = "BrowserPort: device viewport")]
fn contract_rejects_wrong_device_size() {
    struct Broken;
    impl BrowserPort for Broken {
        fn capture(&self, _: &BrowserRequest) -> Result<BrowserCapture, BrowserError> {
            Ok(BrowserCapture {
                png: b"\x89PNG\r\n\x1a\n".to_vec(),
                width: 1,
                height: 1,
                dom: None,
            })
        }
    }
    browser(
        &Broken,
        &BrowserRequest {
            url: "http://localhost/".into(),
            origins: vec![],
            width: 320,
            height: 640,
            selector: None,
            scroll: [0., 0.],
            zoom: 1.,
            snapshot: false,
        },
    );
}
