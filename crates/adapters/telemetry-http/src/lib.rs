//! Explicit consent and fixed payloads; default path performs no IO.
use serde::{Deserialize, Serialize};
use std::time::Duration;
#[derive(Default, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    #[serde(default)]
    pub enabled: bool,
    pub origin: Option<String>,
}
impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.enabled && self.origin.is_none() {
            return Ok(());
        }
        let origin = self
            .origin
            .as_ref()
            .ok_or("Telemetry needs a configured HTTPS origin")?;
        let u = reqwest::Url::parse(origin).map_err(|_| "Invalid telemetry origin")?;
        if u.scheme() != "https"
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
            || u.query().is_some()
            || u.fragment().is_some()
            || u.path() != "/"
        {
            return Err("Telemetry requires an HTTPS origin without credentials, path or query");
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub enum Event {
    CliStarted,
    DaemonStarted,
}
fn payload(event: Event, version: &str, os: &str) -> serde_json::Value {
    serde_json::json!({"event":match event{Event::CliStarted=>"cli_started",Event::DaemonStarted=>"daemon_started"},"version":version,"os":os,"count":1})
}
/// No account or credential is involved. Errors are deliberately content-free and never logged.
/// This bounded, synchronous boundary is called at most once on process start, only after opt-in.
pub fn send(settings: &Settings, event: Event) -> Result<(), &'static str> {
    if !settings.enabled {
        return Ok(());
    }
    settings.validate()?;
    let origin = settings.origin.as_ref().ok_or("Telemetry origin missing")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Telemetry unavailable")?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .https_only(true)
            .timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Telemetry unavailable")?;
        let os = match std::env::consts::OS {
            "macos" => "macos",
            "windows" => "windows",
            "linux" => "linux",
            _ => "unknown",
        };
        let response = client
            .post(format!("{}/v1/telemetry", origin.trim_end_matches('/')))
            .json(&payload(event, env!("CARGO_PKG_VERSION"), os))
            .send()
            .await
            .map_err(|_| "Telemetry unavailable")?;
        if !response.status().is_success() {
            return Err("Telemetry unavailable");
        }
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consent_and_payload_are_bounded() {
        assert!(!Settings::default().enabled);
        assert_eq!(send(&Settings::default(), Event::CliStarted), Ok(()));
        assert!(
            Settings {
                enabled: true,
                origin: None
            }
            .validate()
            .is_err()
        );
        for origin in [
            "http://example.test",
            "https://key@example.test",
            "https://example.test/private?key=secret",
            "https://example.test/path",
        ] {
            assert!(
                Settings {
                    enabled: true,
                    origin: Some(origin.into())
                }
                .validate()
                .is_err()
            );
        }
        assert_eq!(
            payload(Event::CliStarted, "0.0.0", "linux"),
            serde_json::json!({"event":"cli_started","version":"0.0.0","os":"linux","count":1})
        );
    }
    #[test]
    fn corrupt_or_extra_settings_cannot_opt_in() {
        for raw in [
            r#"{"enabled":"true"}"#,
            r#"{"enabled":true,"token":"TEST DATA"}"#,
        ] {
            assert!(serde_json::from_str::<Settings>(raw).is_err());
        }
    }
}
