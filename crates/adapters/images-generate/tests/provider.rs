//! TEST DATA. The provider never exposes error response bodies or keys.
use std::sync::Arc;
use vellum_core::ports::*;
use vellum_images_generate::ImageModel;
struct Http;
impl MediaHttp for Http {
    fn exchange(&self, r: &MediaRequest) -> Result<FetchResponse, FetchError> {
        assert_eq!(r.method, MediaMethod::Post);
        assert_eq!(r.credential.as_ref().unwrap().0, "Authorization");
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(v["n"], 1);
        Ok(FetchResponse{status:200,content_type:"application/json".into(),body:serde_json::to_vec(&serde_json::json!({"data":[{"b64_json":vellum_core::base64::encode(b"TEST DATA png")}]})).unwrap()})
    }
}
#[test]
fn generation_uses_a_keychain_header_and_decodes_inline_result() {
    let secrets = Arc::new(vellum_contracts::fakes::MemorySecrets::default());
    secrets
        .set("image_model", SecretString::new("TEST_DATA_KEY"))
        .unwrap();
    let p = ImageModel::new(
        "configured",
        "https://images.example.com/v1/images/generations",
        "image_model",
        Arc::new(Http),
        secrets,
    )
    .unwrap();
    let r = p
        .generate(&GenerateImage {
            model: "TEST DATA model".into(),
            prompt: "TEST DATA illustration".into(),
        })
        .unwrap();
    assert_eq!(r.bytes, b"TEST DATA png");
    assert!(
        p.generate(&GenerateImage {
            model: "".into(),
            prompt: "".into()
        })
        .is_err()
    );
}

#[test]
fn real_adapter_meets_generator_contract_and_refuses_credential_urls() {
    let secrets = Arc::new(vellum_contracts::fakes::MemorySecrets::default());
    secrets
        .set("image_model", SecretString::new("TEST_DATA_KEY"))
        .unwrap();
    let p = ImageModel::new(
        "configured",
        "https://images.example.com/v1/images/generations",
        "image_model",
        Arc::new(Http),
        secrets.clone(),
    )
    .unwrap();
    vellum_contracts::suites::media::generator(&p);
    for url in [
        "http://example.com/images",
        "https://user:pass@example.com/images",
        "https://example.com/images?api_key=bad",
    ] {
        assert!(
            ImageModel::new(
                "configured",
                url,
                "image_model",
                Arc::new(Http),
                secrets.clone()
            )
            .is_err()
        )
    }
}

#[test]
fn provider_errors_and_echoed_credentials_never_escape() {
    struct Bad {
        status: u16,
        body: Vec<u8>,
    }
    impl MediaHttp for Bad {
        fn exchange(&self, _: &MediaRequest) -> Result<FetchResponse, FetchError> {
            Ok(FetchResponse {
                status: self.status,
                content_type: "application/json".into(),
                body: self.body.clone(),
            })
        }
    }
    for (status, body, expected) in [
        (
            401,
            b"TEST_DATA_KEY".to_vec(),
            ImageWorkError::Authentication,
        ),
        (429, b"TEST_DATA_KEY".to_vec(), ImageWorkError::RateLimited),
        (
            200,
            b"{\"error\":\"TEST_DATA_KEY\"}".to_vec(),
            ImageWorkError::InvalidOutput,
        ),
        (
            200,
            b"{\"data\":[{\"url\":\"https://example.com/image\"}]}".to_vec(),
            ImageWorkError::InvalidOutput,
        ),
    ] {
        let secrets = Arc::new(vellum_contracts::fakes::MemorySecrets::default());
        secrets
            .set("image_model", SecretString::new("TEST_DATA_KEY"))
            .unwrap();
        let model = ImageModel::new(
            "configured",
            "https://images.example.com/images",
            "image_model",
            Arc::new(Bad { status, body }),
            secrets,
        )
        .unwrap();
        let err = model
            .generate(&GenerateImage {
                prompt: "TEST DATA".into(),
                model: "fixture".into(),
            })
            .unwrap_err();
        assert_eq!(err, expected);
        assert!(!err.to_string().contains("TEST_DATA_KEY"));
    }
}
