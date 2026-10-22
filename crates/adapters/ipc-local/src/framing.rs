//! Bounded daemon framing, also compiled directly by the cargo-fuzz target.
use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// Bounded incremental framing: read_line would allocate without limit before it can be checked.
pub async fn frame<R: AsyncBufRead + Unpin>(
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
