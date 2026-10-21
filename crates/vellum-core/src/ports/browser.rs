//! Browser port: native code owns Chromium; the core consumes only data.
pub use vellum_types::browser::{
    BrowserCapture, BrowserError, BrowserRequest, DomNode, DomSnapshot,
};
/// The composition root authorizes every network origin before calling this port. Implementations use
/// isolated sessions and refuse navigation and subrequests outside those origins. No arbitrary CDP API.
pub trait BrowserPort {
    fn capture(&self, request: &BrowserRequest) -> Result<BrowserCapture, BrowserError>;
}
