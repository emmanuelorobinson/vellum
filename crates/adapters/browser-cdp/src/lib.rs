//! Isolated, bounded Chromium captures. No document changes or arbitrary script interface.
use chromiumoxide::cdp::browser_protocol::{
    emulation::{SetDeviceMetricsOverrideParams, SetPageScaleFactorParams},
    fetch::{ContinueRequestParams, EventRequestPaused, FailRequestParams},
    network::ErrorReason,
    page::{CaptureScreenshotFormat, CaptureScreenshotParams, Viewport as Clip},
};
use chromiumoxide::{Browser, BrowserConfig};
use futures::StreamExt;
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::PathBuf, time::Duration};
use url::Url;
use vellum_core::ports::{BrowserCapture, BrowserError, BrowserPort, BrowserRequest};
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const LIMIT: Duration = Duration::from_secs(30);
fn web_url(value: &str) -> Result<Url, BrowserError> {
    let url = Url::parse(value).map_err(|_| BrowserError::InvalidRequest)?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
    {
        return Err(BrowserError::InvalidRequest);
    }
    Ok(url)
}
pub fn authorized_origins(request: &BrowserRequest) -> Result<BTreeSet<String>, BrowserError> {
    request.validate()?;
    let mut origins = BTreeSet::from([web_url(&request.url)?.origin().ascii_serialization()]);
    for raw in &request.origins {
        let url = web_url(raw)?;
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err(BrowserError::InvalidRequest);
        }
        origins.insert(url.origin().ascii_serialization());
    }
    Ok(origins)
}
pub fn permits_url(origins: &BTreeSet<String>, value: &str) -> bool {
    if value.starts_with("data:") {
        return true;
    }
    web_url(value).is_ok_and(|u| origins.contains(&u.origin().ascii_serialization()))
}
pub fn capture_script(request: &BrowserRequest) -> String {
    let shared: Value = serde_json::from_str(include_str!(
        "../../../../packages/host-api/src/browser-snapshot.json"
    ))
    .expect("bundled collector");
    format!(
        "({})({})",
        shared["script"].as_str().expect("collector function"),
        json!({"selector":request.selector,"scroll":request.scroll,"snapshot":request.snapshot})
    )
}
/// Executable is configured by the host; never supplied by a tool caller. Chrome's sandbox remains enabled.
#[derive(Default)]
pub struct ChromiumBrowser {
    pub executable: Option<PathBuf>,
}
impl BrowserPort for ChromiumBrowser {
    fn capture(&self, request: &BrowserRequest) -> Result<BrowserCapture, BrowserError> {
        let origins = authorized_origins(request)?;
        // Dedicated runtime: tools can be invoked from an existing async host without nested block_on.
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let rt = tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .enable_all()
                        .build()
                        .map_err(|_| BrowserError::Unavailable)?;
                    let result = rt.block_on(self.run(request, origins));
                    rt.shutdown_timeout(Duration::from_secs(2));
                    result
                })
                .join()
                .unwrap_or(Err(BrowserError::Capture))
        })
    }
}
impl ChromiumBrowser {
    async fn run(
        &self,
        request: &BrowserRequest,
        origins: BTreeSet<String>,
    ) -> Result<BrowserCapture, BrowserError> {
        let profile = tempfile::Builder::new()
            .prefix("vellum-browser-")
            .tempdir()
            .map_err(|_| BrowserError::Unavailable)?;
        let mut config = BrowserConfig::builder()
            .user_data_dir(profile.path())
            .new_headless_mode()
            .respect_https_errors()
            .enable_request_intercept()
            .incognito()
            .request_timeout(Duration::from_secs(10))
            .launch_timeout(Duration::from_secs(10))
            .arg("--disable-extensions")
            .arg("--disable-background-networking")
            .arg("--disable-component-update")
            .arg("--disable-sync")
            .arg("--no-first-run");
        if let Some(executable) = &self.executable {
            config = config.chrome_executable(executable);
        }
        let (mut browser, mut handler) =
            Browser::launch(config.build().map_err(|_| BrowserError::Unavailable)?)
                .await
                .map_err(|_| BrowserError::Unavailable)?;
        let pump = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if event.is_err() {
                    break;
                }
            }
        });
        let result = tokio::time::timeout(LIMIT, async {
            let page = browser
                .new_page("about:blank")
                .await
                .map_err(|_| BrowserError::Capture)?;
            let mut events = page
                .event_listener::<EventRequestPaused>()
                .await
                .map_err(|_| BrowserError::Capture)?;
            let guard_page = page.clone();
            let guard = tokio::spawn(async move {
                while let Some(event) = events.next().await {
                    if permits_url(&origins, &event.request.url) {
                        let _ = guard_page
                            .execute(ContinueRequestParams::new(event.request_id.clone()))
                            .await;
                    } else {
                        let _ = guard_page
                            .execute(FailRequestParams::new(
                                event.request_id.clone(),
                                ErrorReason::BlockedByClient,
                            ))
                            .await;
                    }
                }
            });
            let outcome = async {
                page.execute(SetDeviceMetricsOverrideParams::new(
                    request.width,
                    request.height,
                    1.,
                    false,
                ))
                .await
                .map_err(|_| BrowserError::Capture)?;
                page.goto(&request.url)
                    .await
                    .map_err(|_| BrowserError::Navigation)?;
                page.execute(SetPageScaleFactorParams::new(request.zoom))
                    .await
                    .map_err(|_| BrowserError::Capture)?;
                let data: Value = page
                    .evaluate(capture_script(request))
                    .await
                    .map_err(|_| BrowserError::Capture)?
                    .into_value()
                    .map_err(|_| BrowserError::Capture)?;
                if let Some(error) = data["error"].as_str() {
                    return Err(if error == "large" {
                        BrowserError::TooLarge
                    } else {
                        BrowserError::MissingElement
                    });
                }
                let clip: Clip = serde_json::from_value(data["clip"].clone())
                    .map_err(|_| BrowserError::Capture)?;
                let width = clip.width.ceil() as u32;
                let height = clip.height.ceil() as u32;
                let params = CaptureScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .clip(clip)
                    .capture_beyond_viewport(true)
                    .build();
                let png = page
                    .execute(params)
                    .await
                    .map_err(|_| BrowserError::Capture)?
                    .result
                    .data;
                let png = vellum_core::base64::decode(png.as_ref()).ok_or(BrowserError::Capture)?;
                if png.len() > 32 * 1024 * 1024 || data.to_string().len() > 32 * 1024 * 1024 {
                    return Err(BrowserError::TooLarge);
                }
                let dom = if request.snapshot {
                    Some(
                        serde_json::from_value(data["dom"].clone())
                            .map_err(|_| BrowserError::Capture)?,
                    )
                } else {
                    None
                };
                Ok(BrowserCapture {
                    png,
                    width,
                    height,
                    dom,
                })
            }
            .await;
            guard.abort();
            outcome
        })
        .await
        .unwrap_or(Err(BrowserError::Timeout));
        let _ = tokio::time::timeout(Duration::from_secs(2), browser.close()).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), browser.kill()).await;
        pump.abort();
        result
    }
}
