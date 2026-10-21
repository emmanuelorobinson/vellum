//! Image work returns bytes/specs; only the caller commits an attributed Op transaction.
use super::{FetchError, FetchResponse, Imported, SecretString};
use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageWorkError {
    NotConfigured,
    Authentication,
    RateLimited,
    Network,
    PermissionDenied,
    InvalidInput,
    InvalidOutput,
    Unavailable,
}
impl fmt::Display for ImageWorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotConfigured => "Configure the provider or local model in Settings, Assets.",
            Self::Authentication => "The provider did not accept the key.",
            Self::RateLimited => "The provider is rate limiting requests. Try again later.",
            Self::PermissionDenied => "Network access was denied.",
            Self::Network => "The provider could not be reached.",
            Self::InvalidInput => "The image request is invalid.",
            Self::InvalidOutput => "The result could not be used.",
            Self::Unavailable => "The image service is unavailable.",
        })
    }
}
impl std::error::Error for ImageWorkError {}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageBytes {
    pub mime: String,
    pub bytes: Vec<u8>,
}
pub trait ImageOps {
    fn remove_background(&self, bytes: &[u8]) -> Result<ImageBytes, ImageWorkError>;
    fn trace(&self, bytes: &[u8]) -> Result<String, ImageWorkError>;
}
/// Local inference only. RGBA input, same-size finite alpha probabilities in [0,1].
pub trait SegmentationModel {
    fn mask(&self, rgba: &[u8], width: u32, height: u32) -> Result<Vec<f32>, ImageWorkError>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateImage {
    pub prompt: String,
    pub model: String,
}
pub trait ImageGenerator {
    fn provider(&self) -> &str;
    fn origin(&self) -> &str;
    fn generate(&self, request: &GenerateImage) -> Result<ImageBytes, ImageWorkError>;
}
pub trait FigmaSource {
    fn import_file(
        &self,
        key: &str,
        allow: &dyn Fn(&str) -> Result<(), ImageWorkError>,
    ) -> Result<Imported, ImageWorkError>;
}
/// Credential values are redacted by SecretString. Transports never echo response bodies in errors.
#[derive(Debug, Clone)]
pub struct MediaRequest {
    pub url: String,
    pub method: MediaMethod,
    pub body: Vec<u8>,
    pub credential: Option<(String, SecretString)>,
    pub max_bytes: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMethod {
    Get,
    Post,
}
pub trait MediaHttp: Send + Sync {
    fn exchange(&self, request: &MediaRequest) -> Result<FetchResponse, FetchError>;
}
