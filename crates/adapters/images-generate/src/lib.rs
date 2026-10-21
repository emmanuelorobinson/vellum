//! GPT Image-compatible inline image protocol. Credentials stay in SecretStore; no remote output URLs.
use serde_json::{Value, json};
use std::sync::Arc;
use vellum_core::ports::*;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub struct ImageModel {
    provider: String,
    endpoint: String,
    origin: String,
    key: String,
    http: Arc<dyn MediaHttp>,
    secrets: Arc<dyn SecretStore>,
}
impl ImageModel {
    pub fn new(
        provider: &str,
        endpoint: &str,
        key: &str,
        http: Arc<dyn MediaHttp>,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Self, ImageWorkError> {
        let u = url::Url::parse(endpoint).map_err(|_| ImageWorkError::InvalidInput)?;
        if u.scheme() != "https"
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.query().is_some()
            || u.fragment().is_some()
            || provider.is_empty()
            || provider.len() > 64
            || !provider
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(ImageWorkError::InvalidInput);
        }
        Ok(Self {
            provider: provider.into(),
            endpoint: endpoint.into(),
            origin: u.origin().ascii_serialization(),
            key: key.into(),
            http,
            secrets,
        })
    }
}
impl ImageGenerator for ImageModel {
    fn provider(&self) -> &str {
        &self.provider
    }
    fn origin(&self) -> &str {
        &self.origin
    }
    fn generate(&self, r: &GenerateImage) -> Result<ImageBytes, ImageWorkError> {
        if r.prompt.trim().is_empty()
            || r.prompt.len() > 32000
            || r.model.trim().is_empty()
            || r.model.len() > 128
        {
            return Err(ImageWorkError::InvalidInput);
        }
        let secret = self
            .secrets
            .get(&self.key)
            .map_err(|_| ImageWorkError::Unavailable)?
            .ok_or(ImageWorkError::NotConfigured)?;
        let body = serde_json::to_vec(
            &json!({"prompt":r.prompt,"model":r.model,"n":1,"output_format":"png"}),
        )
        .map_err(|_| ImageWorkError::InvalidInput)?;
        let response = self
            .http
            .exchange(&MediaRequest {
                url: self.endpoint.clone(),
                method: MediaMethod::Post,
                body,
                credential: Some((
                    "Authorization".into(),
                    SecretString::new(format!("Bearer {}", secret.expose())),
                )),
                max_bytes: 32 * 1024 * 1024,
            })
            .map_err(|_| ImageWorkError::Network)?;
        match response.status {
            200 => {}
            401 | 403 => return Err(ImageWorkError::Authentication),
            429 => return Err(ImageWorkError::RateLimited),
            _ => return Err(ImageWorkError::Network),
        }
        if response.body.len() > 32 * 1024 * 1024
            || (!secret.expose().is_empty()
                && response
                    .body
                    .windows(secret.expose().len())
                    .any(|b| b == secret.expose().as_bytes()))
        {
            return Err(ImageWorkError::InvalidOutput);
        }
        let v: Value =
            serde_json::from_slice(&response.body).map_err(|_| ImageWorkError::InvalidOutput)?;
        let data = v["data"]
            .as_array()
            .filter(|a| a.len() == 1)
            .ok_or(ImageWorkError::InvalidOutput)?;
        let bytes = data[0]["b64_json"]
            .as_str()
            .and_then(vellum_core::base64::decode)
            .filter(|b| !b.is_empty() && b.len() <= 20 * 1024 * 1024)
            .ok_or(ImageWorkError::InvalidOutput)?;
        Ok(ImageBytes {
            mime: "image/png".into(),
            bytes,
        })
    }
}
