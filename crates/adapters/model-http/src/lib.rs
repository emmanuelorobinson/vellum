//! Cancellable HTTP/SSE only; composition roots inject this into protocol adapters.
use async_channel::{Receiver, Sender};
use std::{
    future::{Future, poll_fn},
    sync::OnceLock,
    task::Poll,
    time::Duration,
};
use vellum_core::ports::*;
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
const MAX_EVENT: usize = 1024 * 1024;
const MAX_STREAM: usize = 16 * 1024 * 1024;

pub struct HttpTransport {
    timeout: Duration,
}
impl Default for HttpTransport {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(180),
        }
    }
}
impl HttpTransport {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}
fn runtime() -> Result<&'static tokio::runtime::Runtime, ModelError> {
    static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, ()>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .map_err(|_| ())
        })
        .as_ref()
        .map_err(|_| ModelError::Unavailable)
}
struct Stream {
    receiver: Receiver<Result<WireEvent, ModelError>>,
    cancel: Cancellation,
    done: bool,
}
impl ModelWireStream for Stream {
    fn next(&mut self) -> ModelFuture<'_, Option<Result<WireEvent, ModelError>>> {
        Box::pin(async move {
            if self.done {
                return None;
            }
            let mut recv = Box::pin(self.receiver.recv());
            let result = poll_fn(|cx| {
                if self.cancel.poll_cancelled(cx).is_ready() {
                    return Poll::Ready(Some(Err(ModelError::Cancelled)));
                }
                recv.as_mut().poll(cx).map(|value| value.ok())
            })
            .await;
            self.done = result.is_none() || matches!(result, Some(Err(_)));
            result
        })
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.receiver.close();
    }
}
impl ModelTransport for HttpTransport {
    fn start(
        &self,
        request: ModelHttpRequest,
        cancel: Cancellation,
    ) -> ModelFuture<'_, Result<WireStream, ModelError>> {
        Box::pin(async move {
            validate_endpoint(&request.endpoint)?;
            if cancel.is_cancelled() {
                return Err(ModelError::Cancelled);
            }
            let (sender, receiver) = async_channel::bounded(32);
            let timeout = self.timeout;
            let worker_cancel = cancel.clone();
            runtime()?.spawn(async move {
                let result = tokio::select! {
                    biased;
                    _ = poll_fn(|cx| worker_cancel.poll_cancelled(cx)) => Err(ModelError::Cancelled),
                    _ = sender.closed() => return,
                    result = run(request, &sender, timeout) => result,
                };
                if let Err(error) = result { let _ = sender.send(Err(error)).await; }
            });
            Ok(Box::new(Stream {
                receiver,
                cancel,
                done: false,
            }) as WireStream)
        })
    }
}
fn validate_endpoint(endpoint: &str) -> Result<(), ModelError> {
    let url = url::Url::parse(endpoint).map_err(|_| ModelError::InvalidRequest)?;
    let local = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain("localhost")) => true,
        _ => false,
    };
    if (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url
            .query_pairs()
            .any(|(key, _)| !matches!(key.as_ref(), "alt" | "api-version"))
    {
        return Err(ModelError::InvalidRequest);
    }
    Ok(())
}
fn network_error(error: reqwest::Error) -> ModelError {
    if error.is_timeout() {
        ModelError::Timeout
    } else {
        ModelError::Network
    }
}
async fn run(
    request: ModelHttpRequest,
    sender: &Sender<Result<WireEvent, ModelError>>,
    timeout: Duration,
) -> Result<(), ModelError> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ModelError::Unavailable)?;
    let mut post = client
        .post(&request.endpoint)
        .header("Accept", "text/event-stream");
    for (name, value) in &request.headers {
        if name.eq_ignore_ascii_case("authorization") || name.to_ascii_lowercase().contains("key") {
            return Err(ModelError::InvalidRequest);
        }
        post = post.header(name, value);
    }
    if let Some((name, value)) = &request.credential {
        post = post.header(name, value.expose());
    }
    let mut response = post
        .json(&request.body)
        .send()
        .await
        .map_err(network_error)?;
    match response.status().as_u16() {
        200 => {}
        401 | 403 => return Err(ModelError::Authentication),
        429 => return Err(ModelError::RateLimited),
        500..=599 => return Err(ModelError::Unavailable),
        _ => return Err(ModelError::Protocol),
    }
    if !response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(';').next() == Some("text/event-stream"))
    {
        return Err(ModelError::Protocol);
    }
    let secret = request.credential.as_ref().map(|(_, value)| {
        value
            .expose()
            .strip_prefix("Bearer ")
            .unwrap_or(value.expose())
    });
    let mut parser = SseParser::default();
    while let Some(chunk) = response.chunk().await.map_err(network_error)? {
        for data in parser.feed(&chunk)? {
            let event = if data == "[DONE]" {
                WireEvent::Done
            } else {
                let mut value = serde_json::from_str(&data).map_err(|_| ModelError::Protocol)?;
                if let Some(secret) = secret {
                    redact(&mut value, secret);
                }
                WireEvent::Data(value)
            };
            sender
                .send(Ok(event))
                .await
                .map_err(|_| ModelError::Cancelled)?;
        }
    }
    parser.finish()
}
fn redact(value: &mut serde_json::Value, secret: &str) {
    match value {
        serde_json::Value::String(text) if !secret.is_empty() => {
            *text = text.replace(secret, "[redacted]")
        }
        serde_json::Value::Array(values) => values.iter_mut().for_each(|v| redact(v, secret)),
        serde_json::Value::Object(values) => values.values_mut().for_each(|v| redact(v, secret)),
        _ => {}
    }
}
/// Bytes can split inside UTF-8, CRLF, JSON or SSE fields. Nothing is decoded before a line ends.
#[derive(Default)]
struct SseParser {
    line: Vec<u8>,
    data: String,
    total: usize,
}
impl SseParser {
    fn feed(&mut self, bytes: &[u8]) -> Result<Vec<String>, ModelError> {
        self.total += bytes.len();
        if self.total > MAX_STREAM {
            return Err(ModelError::Limit);
        }
        let mut events = vec![];
        for byte in bytes {
            self.line.push(*byte);
            if self.line.len() > MAX_EVENT {
                return Err(ModelError::Limit);
            }
            if *byte != b'\n' {
                continue;
            }
            let line = std::str::from_utf8(&self.line)
                .map_err(|_| ModelError::Protocol)?
                .trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(self.data.trim_end_matches('\n').to_owned());
                    self.data.clear();
                }
            } else if let Some(value) = line.strip_prefix("data:") {
                self.data.push_str(value.strip_prefix(' ').unwrap_or(value));
                self.data.push('\n');
                if self.data.len() > MAX_EVENT {
                    return Err(ModelError::Limit);
                }
            }
            self.line.clear();
        }
        Ok(events)
    }
    fn finish(&self) -> Result<(), ModelError> {
        if self.line.is_empty() && self.data.is_empty() {
            Ok(())
        } else {
            Err(ModelError::Truncated)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sse_fragments_crlf_comments_and_multiline() {
        let mut parser = SseParser::default();
        let mut events = vec![];
        for byte in b": comment\r\ndata: {\r\ndata: \"x\": 1}\r\n\r\ndata: [DONE]\n\n" {
            events.extend(parser.feed(&[*byte]).unwrap());
        }
        parser.finish().unwrap();
        assert_eq!(events, ["{\n\"x\": 1}", "[DONE]"]);
    }
    #[test]
    fn invalid_endpoints_and_oversized_events_fail() {
        for endpoint in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com?key=secret",
            "file:///tmp/test",
        ] {
            assert!(validate_endpoint(endpoint).is_err());
        }
        assert!(validate_endpoint("http://127.0.0.1:1234/chat").is_ok());
        assert_eq!(
            SseParser::default().feed(&vec![b'x'; MAX_EVENT + 1]),
            Err(ModelError::Limit)
        );
    }
}
