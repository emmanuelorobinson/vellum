//! Native credential boundary. Stdio carries only typed protocol frames, never capabilities.
use super::*;
use std::{
    io::{BufRead, Read, Write},
    sync::mpsc,
};
use tungstenite::{Message, client::IntoClientRequest, stream::MaybeTlsStream};
pub fn bridge_stdio(endpoint: &str, room: &RoomId, key: &SecretString) -> Result<(), Error> {
    let origin = endpoint.trim_end_matches('/');
    let mut request = origin
        .into_client_request()
        .map_err(|_| Error::InvalidUpdate)?;
    let uri = request.uri();
    let secure = uri.scheme_str() == Some("wss");
    let local = uri.scheme_str() == Some("ws")
        && matches!(uri.host(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if !(secure || local)
        || uri.path() != "/"
        || uri.query().is_some()
        || uri.authority().is_none_or(|a| a.as_str().contains('@'))
        || endpoint.len() > 2048
    {
        return Err(Error::InvalidUpdate);
    }
    *request.uri_mut() = format!("{origin}/v1/rooms/{}", room.as_str())
        .parse()
        .map_err(|_| Error::InvalidUpdate)?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", key.expose())
            .parse()
            .map_err(|_| Error::Forbidden)?,
    );
    let config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(192 * 1024 * 1024))
        .max_frame_size(Some(192 * 1024 * 1024))
        .max_write_buffer_size(16 * 1024 * 1024);
    // Never forward the capability to a redirect target.
    let (mut socket, _) = tungstenite::client::connect_with_config(request, Some(config), 0)
        .map_err(|_| Error::Unavailable)?;
    let stream = match socket.get_mut() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => &mut s.sock,
        _ => return Err(Error::Unavailable),
    };
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| Error::Unavailable)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| Error::Unavailable)?;
    let (sender, receiver) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            let mut line = Vec::new();
            let read = (&mut input)
                .take(MAX_WIRE as u64 + 1)
                .read_until(b'\n', &mut line);
            if !matches!(read,Ok(n) if n>0&&n<=MAX_WIRE&&line.last()==Some(&b'\n')) {
                break;
            }
            let Ok(frame) = serde_json::from_slice::<ClientFrame>(&line) else {
                break;
            };
            if sender.send(frame).is_err() {
                break;
            }
        }
    });
    let mut output = std::io::stdout().lock();
    loop {
        match receiver.try_recv() {
            Ok(frame) => socket
                .send(Message::Text(
                    serde_json::to_string(&frame)
                        .map_err(|_| Error::InvalidUpdate)?
                        .into(),
                ))
                .map_err(|_| Error::Unavailable)?,
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                let frame: ServerFrame =
                    serde_json::from_str(&text).map_err(|_| Error::InvalidUpdate)?;
                serde_json::to_writer(&mut output, &frame).map_err(|_| Error::Unavailable)?;
                output
                    .write_all(b"\n")
                    .and_then(|_| output.flush())
                    .map_err(|_| Error::Unavailable)?;
            }
            Ok(Message::Ping(bytes)) => socket
                .send(Message::Pong(bytes))
                .map_err(|_| Error::Unavailable)?,
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Err(Error::Unavailable),
        }
    }
    let _ = socket.close(None);
    Ok(())
}
