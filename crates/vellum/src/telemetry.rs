//! Native consent is separate from document/session state. Corrupt/missing settings fail closed.
use std::{io, path::Path};
pub fn load(state: &Path) -> vellum_telemetry_http::Settings {
    let settings = std::fs::read(state.join("telemetry.json"))
        .ok()
        .filter(|b| b.len() <= 4096)
        .and_then(|b| serde_json::from_slice::<vellum_telemetry_http::Settings>(&b).ok());
    settings
        .filter(|s| s.validate().is_ok())
        .unwrap_or_default()
}
pub fn save(state: &Path, settings: &vellum_telemetry_http::Settings) -> io::Result<()> {
    settings.validate().map_err(io::Error::other)?;
    std::fs::create_dir_all(state)?;
    vellum_core::files::atomic_write(
        &vellum_assets_fs::StdFs::new(state),
        "telemetry.json",
        &serde_json::to_vec(settings)?,
    )
    .map_err(|_| io::Error::other("Telemetry settings could not be saved"))
}
pub fn emit(state: &Path, event: vellum_telemetry_http::Event) {
    let _ = vellum_telemetry_http::send(&load(state), event);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_is_off() {
        assert!(!load(Path::new("target/not-a-telemetry-config")).enabled);
    }
}
