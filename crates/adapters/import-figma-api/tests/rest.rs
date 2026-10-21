//! TEST DATA. REST JSON only, no proprietary file parsing.
use serde_json::json;
use vellum_core::ports::{ImportInput, Importer};
use vellum_import_figma_api::FigmaImporter;
#[test]
fn rest_fixture_preserves_geometry_and_reports_loss() {
    let bytes = serde_json::to_vec(&json!({"document":{"type":"DOCUMENT","children":[{"type":"CANVAS","children":[{"id":"1:1","name":"TEST DATA card","type":"FRAME","absoluteBoundingBox":{"x":20,"y":30,"width":320,"height":200},"layoutMode":"VERTICAL","children":[{"id":"1:2","type":"TEXT","characters":"TEST DATA title","absoluteBoundingBox":{"x":28,"y":38,"width":150,"height":24},"style":{"fontFamily":"Inter","fontSize":18,"fontWeight":500},"fills":[{"type":"SOLID","color":{"r":1,"g":0,"b":0,"a":1}}]},{"id":"1:3","type":"SLICE"}]}]}]}})).unwrap();
    let result = FigmaImporter
        .import(&ImportInput {
            bytes: &bytes,
            name: "TEST DATA",
        })
        .unwrap();
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0]["children"][0]["position"]["x"], 8.0);
    assert_eq!(
        result.nodes[0]["children"][0]["content"][0]["text"],
        "TEST DATA title"
    );
    assert!(result.notes.iter().any(|s| s.contains("SLICE")));
    assert!(result.notes.iter().any(|s| s.contains("layout")));
    assert!(
        FigmaImporter
            .import(&ImportInput {
                bytes: b"fig-kiwi",
                name: "a.fig"
            })
            .is_err()
    );
}

use std::sync::{Arc, Mutex};
use vellum_core::ports::*;
struct Http {
    calls: Mutex<Vec<(String, bool)>>,
    status: u16,
}
impl MediaHttp for Http {
    fn exchange(&self, r: &MediaRequest) -> Result<FetchResponse, FetchError> {
        self.calls
            .lock()
            .unwrap()
            .push((r.url.clone(), r.credential.is_some()));
        let body = if r.url.ends_with("/images") {
            serde_json::to_vec(
                &json!({"meta":{"images":{"pic":"https://s3-alpha.figma.com/image.png"}}}),
            )
            .unwrap()
        } else if r.url.contains("geometry=paths") {
            assert_eq!(r.credential.as_ref().unwrap().0, "X-Figma-Token");
            serde_json::to_vec(&json!({"document":{"type":"DOCUMENT","children":[{"id":"1:2","type":"RECTANGLE","absoluteBoundingBox":{"x":0,"y":0,"width":40,"height":50},"fills":[{"type":"IMAGE","imageRef":"pic","scaleMode":"FIT"}]}]}})).unwrap()
        } else {
            assert!(r.credential.is_none());
            b"TEST DATA image".to_vec()
        };
        Ok(FetchResponse {
            status: self.status,
            content_type: if r.credential.is_some() {
                "application/json"
            } else {
                "image/png"
            }
            .into(),
            body,
        })
    }
}
#[test]
fn api_credentials_never_reach_image_downloads_and_every_origin_is_authorized() {
    let http = Arc::new(Http {
        calls: Mutex::new(vec![]),
        status: 200,
    });
    let secrets = Arc::new(vellum_contracts::fakes::MemorySecrets::default());
    secrets
        .set(
            vellum_import_figma_api::KEY_NAME,
            SecretString::new("TEST_DATA_TOKEN_CANARY_66"),
        )
        .unwrap();
    let api = vellum_import_figma_api::FigmaApi {
        http: http.clone(),
        secrets,
    };
    assert_eq!(
        api.import_file("test", &|_| Err(ImageWorkError::PermissionDenied)),
        Err(ImageWorkError::PermissionDenied)
    );
    assert!(http.calls.lock().unwrap().is_empty());
    let allowed = Mutex::new(vec![]);
    let result = api
        .import_file("test", &|o| {
            allowed.lock().unwrap().push(o.to_string());
            Ok(())
        })
        .unwrap();
    assert_eq!(result.assets.len(), 1);
    assert_eq!(result.nodes[0]["fills"][0]["asset"], "import:0");
    assert_eq!(
        *allowed.lock().unwrap(),
        vec!["https://api.figma.com", "https://s3-alpha.figma.com"]
    );
    let calls = http.calls.lock().unwrap();
    assert_eq!(
        calls.iter().map(|(_, key)| *key).collect::<Vec<_>>(),
        vec![true, true, false]
    );
    assert!(!format!("{result:?}").contains("TOKEN_CANARY"));
}
#[test]
fn keys_and_authentication_failures_are_payload_free() {
    for key in [
        "a.fig",
        "https://evil.com/design/a/b",
        "https://user:pass@www.figma.com/design/a/b",
        "a/b",
        "../a",
    ] {
        assert!(vellum_import_figma_api::file_key(key).is_err())
    }
    let secrets = Arc::new(vellum_contracts::fakes::MemorySecrets::default());
    let http = Arc::new(Http {
        calls: Mutex::new(vec![]),
        status: 429,
    });
    let api = vellum_import_figma_api::FigmaApi {
        http,
        secrets: secrets.clone(),
    };
    assert_eq!(
        api.import_file("test", &|_| Ok(())),
        Err(ImageWorkError::NotConfigured)
    );
    secrets
        .set(
            vellum_import_figma_api::KEY_NAME,
            SecretString::new("TEST_DATA_TOKEN_CANARY_66"),
        )
        .unwrap();
    assert_eq!(
        api.import_file("test", &|_| Ok(())),
        Err(ImageWorkError::RateLimited)
    );
}
#[test]
fn importer_contract_and_output_schema() {
    let bytes=serde_json::to_vec(&json!({"document":{"type":"DOCUMENT","children":[{"type":"FRAME","absoluteBoundingBox":{"x":0,"y":0,"width":100,"height":100},"children":[{"type":"TEXT","absoluteBoundingBox":{"x":0,"y":0,"width":80,"height":20},"characters":"TEST DATA","style":{"fontFamily":"Inter","fontSize":14,"fontWeight":400}}]}]}})).unwrap();
    vellum_contracts::suites::import::importer(&FigmaImporter, &bytes);
    let mut n = FigmaImporter
        .import(&ImportInput {
            bytes: &bytes,
            name: "TEST DATA",
        })
        .unwrap()
        .nodes
        .remove(0);
    n["id"] = json!("frame00001");
    n["children"][0]["id"] = json!("text000001");
    let _: vellum_types::Node = serde_json::from_value(n).unwrap();
}
