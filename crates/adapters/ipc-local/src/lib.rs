//! Native boundary for private IPC and process lifecycle. No document mutation happens here.
mod client;
mod private;
mod server;
mod transport;

pub use client::{bridge, ensure_daemon, status, stop};
pub use private::RunDir;
pub use server::{DaemonConfig, run};

use serde_json::Value;
use std::{io, time::Duration};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

const MAX_FRAME: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(3);
const MAX_CLIENTS: usize = 64;

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

/// Transport scaffold only. Prompt 29 replaces this handler with the shared MCP registry.
fn rpc(bytes: &[u8]) -> Option<Value> {
    let request: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => {
            return Some(
                serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
            );
        }
    };
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !request.get("method").is_some_and(Value::is_string)
        || !request.is_object()
    {
        return Some(
            serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}}),
        );
    }
    let id = request.get("id")?;
    if !(id.is_string() || id.is_number() || id.is_null()) {
        return Some(
            serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request"}}),
        );
    }
    Some(
        serde_json::json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"MCP tool registry not implemented yet (Prompt 29)"}}),
    )
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
    fn scaffold_never_claims_tools_exist_or_answers_notifications() {
        assert!(rpc(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
        assert_eq!(rpc(b"garbage").unwrap()["error"]["code"], -32700);
        assert_eq!(rpc(b"[]").unwrap()["error"]["code"], -32600);
        let response = rpc(br#"{"jsonrpc":"2.0","id":"request","method":"tools/list"}"#).unwrap();
        assert_eq!(response["id"], "request");
        assert_eq!(response["error"]["code"], -32601);
        assert!(response.get("result").is_none());
    }
}
