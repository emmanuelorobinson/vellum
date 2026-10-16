//! TEST DATA: ordered wire transcripts over a loopback-only HTTP endpoint.
//! The endpoint never retains request headers or credentials. Each request consumes one reply;
//! exhaustion is HTTP 409, not a fabricated completion. No listener exists until start().
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

#[derive(Clone, Debug)]
pub struct CapturedRequest {
    pub path: String,
    pub body: Value,
    pub credential_header: Option<String>,
}
#[derive(Clone, Debug)]
pub struct StubReply {
    pub status: u16,
    pub content_type: String,
    pub chunks: Vec<Vec<u8>>,
    pub delay: Duration,
}
impl StubReply {
    pub fn sse(events: Vec<Value>, done: bool) -> Self {
        let mut data = String::new();
        for event in events {
            data.push_str(&format!("data: {event}\r\n\r\n"));
        }
        if done {
            data.push_str("data: [DONE]\r\n\r\n");
        }
        // Deliberately split UTF-8, lines and JSON across transport reads.
        Self {
            status: 200,
            content_type: "text/event-stream".into(),
            chunks: data.as_bytes().chunks(7).map(<[u8]>::to_vec).collect(),
            delay: Duration::ZERO,
        }
    }
    pub fn http(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "application/json".into(),
            chunks: vec![body.as_bytes().to_vec()],
            delay: Duration::ZERO,
        }
    }
}
pub struct StubEndpoint {
    address: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    scripts: Arc<Mutex<VecDeque<StubReply>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    closed: Arc<AtomicUsize>,
}
impl StubEndpoint {
    pub fn start(replies: Vec<StubReply>) -> std::io::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let requests = Arc::new(Mutex::new(Vec::new()));
        let scripts = Arc::new(Mutex::new(VecDeque::from(replies)));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, pending, stopping) = (requests.clone(), scripts.clone(), stop.clone());
        let closed = Arc::new(AtomicUsize::new(0));
        let closed_count = closed.clone();
        let worker = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else {
                    break;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                if let Ok(request) = read_request(&mut stream) {
                    seen.lock().unwrap().push(request);
                    let reply = pending
                        .lock()
                        .unwrap()
                        .pop_front()
                        .unwrap_or_else(|| StubReply::http(409, "script exhausted"));
                    let _ = write_reply(&mut stream, reply, &stopping);
                    closed_count.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        Ok(Self {
            address,
            requests,
            scripts,
            stop,
            worker: Some(worker),
            closed,
        })
    }
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }
    pub fn requests(&self) -> Vec<CapturedRequest> {
        self.requests.lock().unwrap().clone()
    }
    pub fn closed(&self) -> usize {
        self.closed.load(Ordering::SeqCst)
    }
    pub fn remaining(&self) -> usize {
        self.scripts.lock().unwrap().len()
    }
}
impl Drop for StubEndpoint {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn read_request(stream: &mut TcpStream) -> std::io::Result<CapturedRequest> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
    let mut length = 0;
    let mut chunked = false;
    let mut credential_header = None;
    let mut header_bytes = 0;
    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        header_bytes += read;
        if header_bytes > 32 * 1024 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        if read == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.to_ascii_lowercase();
            match name.as_str() {
                "content-length" => {
                    length = value
                        .trim()
                        .parse::<usize>()
                        .map_err(std::io::Error::other)?
                }
                "transfer-encoding" => chunked = value.trim() == "chunked",
                "authorization" | "x-api-key" | "api-key" | "x-goog-api-key" => {
                    credential_header = Some(name)
                }
                _ => {}
            }
        }
    }
    let mut body = vec![];
    if chunked {
        loop {
            line.clear();
            reader.read_line(&mut line)?;
            let size = usize::from_str_radix(line.trim(), 16).map_err(std::io::Error::other)?;
            if size == 0 {
                break;
            }
            if body.len() + size > 8 * 1024 * 1024 {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            let old = body.len();
            body.resize(old + size, 0);
            reader.read_exact(&mut body[old..])?;
            let mut crlf = [0; 2];
            reader.read_exact(&mut crlf)?;
        }
    } else {
        if length > 8 * 1024 * 1024 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        body.resize(length, 0);
        reader.read_exact(&mut body)?;
    }
    Ok(CapturedRequest {
        path,
        body: serde_json::from_slice(&body).map_err(std::io::Error::other)?,
        credential_header,
    })
}
fn write_reply(stream: &mut TcpStream, reply: StubReply, stop: &AtomicBool) -> std::io::Result<()> {
    let length: usize = reply.chunks.iter().map(Vec::len).sum();
    write!(
        stream,
        "HTTP/1.1 {} Stub\r\nContent-Type: {}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n",
        reply.status, reply.content_type
    )?;
    stream.flush()?;
    stream.set_read_timeout(Some(Duration::from_millis(1)))?;
    for (index, chunk) in reply.chunks.into_iter().enumerate() {
        let mut remaining = if index == 0 {
            Duration::ZERO
        } else {
            reply.delay
        };
        while remaining > Duration::ZERO {
            if stop.load(Ordering::SeqCst) || matches!(stream.peek(&mut [0]), Ok(0)) {
                return Ok(());
            }
            let step = remaining.min(Duration::from_millis(10));
            std::thread::sleep(step);
            remaining -= step;
        }
        stream.write_all(&chunk)?;
        stream.flush()?;
    }
    Ok(())
}

/// Canary scanner skeleton: call on all persisted artifacts/logs from a scripted session.
/// Reports paths only; neither the canary nor matching bytes are ever formatted.
pub fn scan_canary(
    roots: &[std::path::PathBuf],
    canary: &[u8],
) -> std::io::Result<Vec<std::path::PathBuf>> {
    if canary.is_empty() {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let mut hits = vec![];
    let mut pending = roots.to_vec();
    while let Some(path) = pending.pop() {
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            for entry in std::fs::read_dir(path)? {
                pending.push(entry?.path());
            }
        } else if meta.is_file() {
            let mut file = std::fs::File::open(&path)?;
            let mut tail = vec![];
            let mut chunk = [0; 8192];
            loop {
                let count = file.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                tail.extend_from_slice(&chunk[..count]);
                if tail.windows(canary.len()).any(|bytes| bytes == canary) {
                    hits.push(path.clone());
                    break;
                }
                let keep = tail.len().min(canary.len() - 1);
                tail.drain(..tail.len() - keep);
            }
        }
    }
    hits.sort();
    Ok(hits)
}
