//! Native boundary for private IPC and process lifecycle. No document mutation happens here.
mod client;
mod interrupt;
mod private;
mod server;
mod transport;

pub use interrupt::{InterruptGuard, on_interrupt};

pub use client::{bridge, ensure_daemon, exchange, status, stop};
pub use private::RunDir;
pub use server::{DaemonConfig, run};

use serde_json::Value;
use std::{io, time::Duration};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

const MAX_FRAME: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(3);
const MAX_CLIENTS: usize = 64;
/// The largest reply a client reads: a screenshot is a few hundred kilobytes of base64.
const MAX_REPLY: usize = 16 * 1024 * 1024;
const IDLE: Duration = Duration::from_millis(250);

fn runtime() -> io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// Bounded incremental framing: read_line would allocate without limit before it can be checked.
async fn frame<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    limit: usize,
) -> io::Result<Option<Vec<u8>>> {
    let mut out = Vec::new();
    loop {
        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            return if out.is_empty() {
                Ok(None)
            } else {
                Err(invalid("truncated frame"))
            };
        }
        let end = buf.iter().position(|b| *b == b'\n').map(|n| n + 1);
        let count = end.unwrap_or(buf.len());
        if out.len() + count > limit {
            return Err(invalid("frame too large"));
        }
        out.extend_from_slice(&buf[..count]);
        reader.consume(count);
        if end.is_some() {
            return Ok(Some(out));
        }
    }
}
async fn send<W: AsyncWrite + Unpin>(
    writer: &mut W,
    value: &impl serde::Serialize,
) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(|_| invalid("cannot encode response"))?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await
}

/// Answers the JSON-RPC requests that arrive on the `mcp` channel and the opt-in HTTP listener.
///
/// It runs on one dedicated thread (see [`HandlerFactory`]), so a request never overlaps another and the
/// handler may hold state that is not `Send` (the daemon's open documents). This crate checks the JSON-RPC
/// envelope; what a method means is the handler's business.
pub trait RpcHandler {
    /// Connection `conn` opened. `cwd` is the directory its client was started in, when it said.
    fn opened(&mut self, _conn: u64, _cwd: Option<String>) {}
    /// A well-formed request or notification from connection `conn`. `None` for a notification, which gets no reply.
    fn request(&mut self, conn: u64, request: Value) -> Option<Value>;
    /// Called about four times a second while nothing is waiting: the handler's housekeeping (watching files,
    /// recovery copies).
    fn idle(&mut self) {}
    /// Connection `conn` ended. Called once, after its last request.
    fn closed(&mut self, _conn: u64) {}
}

/// Builds the handler on the thread that will run it, so the handler need not be `Send`.
pub type HandlerFactory = Box<dyn FnOnce() -> Box<dyn RpcHandler> + Send>;

/// A handler that has no tools: every method is unknown. For transport tests and a daemon built without a registry.
pub fn no_tools() -> HandlerFactory {
    struct NoTools;
    impl RpcHandler for NoTools {
        fn request(&mut self, _: u64, request: Value) -> Option<Value> {
            let id = request.get("id")?.clone();
            Some(
                serde_json::json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}}),
            )
        }
    }
    Box::new(|| Box::new(NoTools))
}

enum Job {
    Opened(u64, Option<String>),
    Call(u64, Value, tokio::sync::oneshot::Sender<Option<Value>>),
    Closed(u64),
}

/// The thread that owns the [`RpcHandler`]. Dropping the last sender ends it.
pub(crate) struct Worker {
    jobs: std::sync::mpsc::Sender<Job>,
    next: std::sync::atomic::AtomicU64,
}
impl Worker {
    pub(crate) fn start(factory: HandlerFactory) -> io::Result<Self> {
        let (jobs, inbox) = std::sync::mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("vellum-tools".into())
            .spawn(move || {
                let mut handler = factory();
                loop {
                    let job = match inbox.recv_timeout(IDLE) {
                        Ok(job) => job,
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                handler.idle()
                            }));
                            continue;
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    match job {
                        Job::Call(conn, request, reply) => {
                            let id = request.get("id").cloned();
                            // A tool that panics must not take the daemon down with it.
                            let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                handler.request(conn, request)
                            }))
                            .unwrap_or_else(|_| {
                                id.map(|id| serde_json::json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":"Internal error"}}))
                            });
                            let _ = reply.send(answer);
                        }
                        Job::Opened(conn, cwd) => {
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                handler.opened(conn, cwd)
                            }));
                        }
                        Job::Closed(conn) => {
                            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                handler.closed(conn)
                            }));
                        }
                    }
                }
            })?;
        Ok(Self {
            jobs,
            next: std::sync::atomic::AtomicU64::new(1),
        })
    }
    pub(crate) fn connection(&self) -> u64 {
        self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
    pub(crate) fn opened(&self, conn: u64, cwd: Option<String>) {
        let _ = self.jobs.send(Job::Opened(conn, cwd));
    }
    pub(crate) fn closed(&self, conn: u64) {
        let _ = self.jobs.send(Job::Closed(conn));
    }
    async fn call(&self, conn: u64, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let (reply, answer) = tokio::sync::oneshot::channel();
        let internal = |id: Option<Value>| {
            id.map(|id| serde_json::json!({"jsonrpc":"2.0","id":id,"error":{"code":-32603,"message":"Internal error"}}))
        };
        if self.jobs.send(Job::Call(conn, request, reply)).is_err() {
            return internal(id);
        }
        answer.await.unwrap_or_else(|_| internal(id))
    }
}

/// Checks the JSON-RPC envelope, then hands the request to the worker.
async fn rpc(bytes: &[u8], worker: &Worker, conn: u64) -> Option<Value> {
    let request: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => {
            return Some(
                serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
            );
        }
    };
    let invalid = || {
        Some(
            serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}}),
        )
    };
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !request.get("method").is_some_and(Value::is_string)
        || !request.is_object()
    {
        return invalid();
    }
    if let Some(id) = request.get("id")
        && !(id.is_string() || id.is_number() || id.is_null())
    {
        return invalid();
    }
    worker.call(conn, request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;
    #[test]
    fn bounded_frames_reject_truncation_and_oversize() {
        runtime().unwrap().block_on(async {
            assert!(
                frame(&mut BufReader::new(&b"unterminated"[..]), 32)
                    .await
                    .is_err()
            );
            assert!(
                frame(&mut BufReader::new(&b"12345\n"[..]), 5)
                    .await
                    .is_err()
            );
            let mut reader = BufReader::new(&b"1234\nnext\n"[..]);
            assert_eq!(frame(&mut reader, 5).await.unwrap().unwrap(), b"1234\n");
            assert_eq!(frame(&mut reader, 5).await.unwrap().unwrap(), b"next\n");
            assert!(frame(&mut reader, 5).await.unwrap().is_none());
        });
    }
    #[test]
    fn envelope_errors_never_reach_the_handler_and_notifications_get_no_reply() {
        runtime().unwrap().block_on(async {
            let worker = Worker::start(no_tools()).unwrap();
            let conn = worker.connection();
            assert!(
                rpc(
                    br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                    &worker,
                    conn
                )
                .await
                .is_none()
            );
            assert_eq!(
                rpc(b"garbage", &worker, conn).await.unwrap()["error"]["code"],
                -32700
            );
            assert_eq!(
                rpc(b"[]", &worker, conn).await.unwrap()["error"]["code"],
                -32600
            );
            assert_eq!(
                rpc(br#"{"jsonrpc":"2.0","id":{},"method":"x"}"#, &worker, conn)
                    .await
                    .unwrap()["error"]["code"],
                -32600
            );
            let response = rpc(
                br#"{"jsonrpc":"2.0","id":"request","method":"tools/list"}"#,
                &worker,
                conn,
            )
            .await
            .unwrap();
            assert_eq!(response["id"], "request");
            assert_eq!(response["error"]["code"], -32601);
            assert!(response.get("result").is_none());
        });
    }
    #[test]
    fn a_handler_that_panics_answers_internal_error_and_keeps_serving() {
        struct Boom(u32);
        impl RpcHandler for Boom {
            fn request(&mut self, _: u64, request: Value) -> Option<Value> {
                self.0 += 1;
                if self.0 == 1 {
                    panic!("tool bug");
                }
                Some(
                    serde_json::json!({"jsonrpc":"2.0","id":request["id"].clone(),"result":{"calls":self.0}}),
                )
            }
        }
        runtime().unwrap().block_on(async {
            let worker = Worker::start(Box::new(|| Box::new(Boom(0)))).unwrap();
            let line = br#"{"jsonrpc":"2.0","id":1,"method":"x"}"#;
            assert_eq!(
                rpc(line, &worker, 1).await.unwrap()["error"]["code"],
                -32603
            );
            assert_eq!(rpc(line, &worker, 1).await.unwrap()["result"]["calls"], 2);
        });
    }
}
