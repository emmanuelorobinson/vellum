//! Non-secret native image settings. Credentials never enter this file or its CLI output.
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
use vellum_core::ports::{SecretStore, SecretString};
#[derive(Default, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AssetSettings {
    pub provider: Option<String>,
    pub endpoint: Option<String>,
    pub model: Option<String>,
    pub segmentation_model: Option<PathBuf>,
}
impl AssetSettings {
    pub fn validate(&self) -> io::Result<()> {
        if self.provider.is_some() != self.endpoint.is_some()
            || self.provider.is_some() != self.model.is_some()
        {
            return Err(io::Error::other(
                "Configure provider, endpoint and model together; choose a local ONNX model path.",
            ));
        }
        if let (Some(provider), Some(endpoint), Some(model)) =
            (&self.provider, &self.endpoint, &self.model)
        {
            if provider.is_empty()
                || provider.len() > 64
                || !provider
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || model.trim().is_empty()
                || model.len() > 128
            {
                return Err(io::Error::other("Invalid image provider or model."));
            }
            vellum_images_generate::ImageModel::new(provider,endpoint,"image_model",Arc::new(vellum_model_http::HttpFetcher::default()),Arc::new(vellum_secrets_keyring::Keychain::new(vellum_types::brand::KEYCHAIN_SERVICE))).map_err(|_|io::Error::other("Use an HTTPS image-generation endpoint without credentials, query or fragment."))?;
        }
        for path in [&self.segmentation_model].into_iter().flatten() {
            if !path.is_absolute() || !path.is_file() {
                return Err(io::Error::other(
                    "Choose existing absolute paths for the ONNX model.",
                ));
            }
        }
        Ok(())
    }
    pub fn load(state: &Path) -> io::Result<Self> {
        let path = state.join("image-settings.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path)?;
        if bytes.len() > 16384 {
            return Err(io::Error::other("Image settings exceed 16 KB."));
        }
        let value: Self = serde_json::from_slice(&bytes)
            .map_err(|_| io::Error::other("Image settings could not be read."))?;
        value.validate()?;
        Ok(value)
    }
    pub fn save(&self, state: &Path) -> io::Result<()> {
        self.validate()?;
        std::fs::create_dir_all(state)?;
        let fs = vellum_assets_fs::StdFs::new(state);
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|_| io::Error::other("Image settings could not be encoded."))?;
        vellum_core::files::atomic_write(&fs, "image-settings.json", &bytes)
            .map_err(|_| io::Error::other("Image settings could not be saved."))
    }
}
pub fn attach(docs: &mut crate::daemon::Documents, state: &Path) -> io::Result<()> {
    let settings = AssetSettings::load(state)?;
    let secrets: Arc<dyn SecretStore> = Arc::new(vellum_secrets_keyring::Keychain::new(
        vellum_types::brand::KEYCHAIN_SERVICE,
    ));
    let http = Arc::new(vellum_model_http::HttpFetcher::with_timeout(
        std::time::Duration::from_secs(180),
    ));
    docs.set_figma(Rc::new(vellum_import_figma_api::FigmaApi {
        http: http.clone(),
        secrets: secrets.clone(),
    }));
    if let (Some(provider), Some(endpoint)) = (&settings.provider, &settings.endpoint) {
        docs.set_image_generator(Rc::new(
            vellum_images_generate::ImageModel::new(
                provider,
                endpoint,
                "image_model",
                http,
                secrets,
            )
            .map_err(|_| io::Error::other("The image provider is invalid."))?,
        ));
    }
    if let Some(model) = settings.model {
        docs.set_image_model(model);
    }
    let model = match &settings.segmentation_model {
        Some(model) => Some(Box::new(
            vellum_image_ops::OnnxSegmentation::open(model).map_err(|_| {
                io::Error::other("The local segmentation model could not be loaded.")
            })?,
        ) as Box<dyn vellum_core::ports::SegmentationModel>),
        None => None,
    };
    docs.set_image_ops(Rc::new(vellum_image_ops::LocalImageOps::new(model)));
    Ok(())
}
pub fn save_key(secrets: &dyn SecretStore, kind: &str, input: &str) -> io::Result<()> {
    let key = match kind {
        "figma" => "figma_token",
        "image" => "image_model",
        _ => return Err(io::Error::other("Choose figma or image.")),
    };
    let value = input.trim();
    if value.is_empty() || value.len() > 4096 || value.contains(['\n', '\r', '\0']) {
        return Err(io::Error::other("The key is empty or invalid."));
    }
    secrets
        .set(key, SecretString::new(value))
        .map_err(|_| io::Error::other("The OS keychain could not save the key."))
}
