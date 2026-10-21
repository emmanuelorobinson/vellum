//! TEST DATA. Browser without a process or the network.
use std::cell::RefCell;
use vellum_core::ports::{BrowserCapture, BrowserError, BrowserPort, BrowserRequest};
pub struct MemoryBrowser {
    pub response: BrowserCapture,
    pub requests: RefCell<Vec<BrowserRequest>>,
}
impl BrowserPort for MemoryBrowser {
    fn capture(&self, request: &BrowserRequest) -> Result<BrowserCapture, BrowserError> {
        request.validate()?;
        self.requests.borrow_mut().push(request.clone());
        Ok(self.response.clone())
    }
}
