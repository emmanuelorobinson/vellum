use crate::{
    DEADLINE, HandlerFactory, MAX_CLIENTS, MAX_FRAME, Worker, frame, invalid,
    private::{Metadata, RunDir, token},
    rpc, runtime, send,
    transport::{Listener, Stream},
};
use std::{fs, io, net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{Semaphore, watch},
    task::JoinSet,
};
use vellum_types::daemon::{DAEMON_PROTOCOL_VERSION, DaemonHello, DaemonWelcome, MAX_CWD};

pub struct DaemonConfig {
    pub dir: RunDir,
    pub listen: Option<SocketAddr>,
    pub allowed_origins: Vec<String>,
    /// Answers the JSON-RPC requests; built on the worker thread that runs it.
    pub handler: HandlerFactory,
}
struct Cleanup(RunDir);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.file("daemon.json"));
        #[cfg(unix)]
        let _ = fs::remove_file(self.0.file("vellum.sock"));
    }
}

pub fn run(config: DaemonConfig) -> io::Result<()> {
    runtime()?.block_on(serve(config))
}
async fn serve(config: DaemonConfig) -> io::Result<()> {
    let lock = config.dir.lock("daemon.lock")?;
    lock.try_lock()
        .map_err(|_| io::Error::new(io::ErrorKind::AddrInUse, "daemon already running"))?;
    // A lock file is permanent: unlinking it would allow waiters to lock a different inode.
    let mut local = Listener::bind(&config.dir)?;
    let _cleanup = Cleanup(config.dir.clone());
    let tcp = match config.listen {
        Some(address) => Some(tokio::net::TcpListener::bind(address).await?),
        None => None,
    };
    let metadata = Arc::new(Metadata {
        v: DAEMON_PROTOCOL_VERSION,
        pid: std::process::id(),
        endpoint: config.dir.endpoint()?,
        token: token()?,
        tcp: tcp
            .as_ref()
            .map(|l| l.local_addr().map(|a| a.to_string()))
            .transpose()?,
    });
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    let (shutdown, mut stopped) = watch::channel(false);
    let worker = Arc::new(Worker::start(config.handler)?);
    let semaphore = Arc::new(Semaphore::new(MAX_CLIENTS));
    let origins = Arc::new(config.allowed_origins);
    let mut tasks = JoinSet::new();
    config.dir.publish(&metadata)?;
    loop {
        tokio::select! {
            result=local.accept()=> {
                let stream=result?;
                if let Ok(permit)=semaphore.clone().try_acquire_owned() {
                    let m=metadata.clone(); let stop=shutdown.clone(); let mut closed=shutdown.subscribe(); let w=worker.clone();
                    tasks.spawn(async move { let _permit=permit; let conn=w.connection(); tokio::select! { _=session(stream,m,stop,&w,conn)=>(), _=closed.changed()=>() } w.closed(conn); });
                }
            },
            result=async { match &tcp { Some(l)=>l.accept().await.map(|(s,_)|s), None=>std::future::pending().await } }=> {
                let stream=result?;
                if let Ok(permit)=semaphore.clone().try_acquire_owned() {
                    let m=metadata.clone(); let allowed=origins.clone(); let w=worker.clone();
                    tasks.spawn(async move { let _permit=permit; let _=http(stream,&m,&allowed,&w).await; });
                }
            },
            _=stopped.changed()=>break,
            _=&mut interrupt=>break,
            _=async { #[cfg(unix)] { terminate.recv().await; } #[cfg(not(unix))] { std::future::pending::<()>().await; } }=>break,
            _=tasks.join_next(), if !tasks.is_empty()=>(),
        }
    }
    let _ = shutdown.send(true);
    drop(local);
    drop(tcp);
    // Auth/read waiters are cancelled; completed replies have already been flushed.
    let _ = tokio::time::timeout(DEADLINE, async {
        while tasks.join_next().await.is_some() {}
    })
    .await;
    tasks.abort_all();
    Ok(())
}

async fn session(
    stream: Stream,
    metadata: Arc<Metadata>,
    shutdown: watch::Sender<bool>,
    worker: &Worker,
    conn: u64,
) -> io::Result<()> {
    let mut stream = BufReader::new(stream);
    let hello = tokio::time::timeout(DEADLINE, frame(&mut stream, 4096 + MAX_CWD))
        .await
        .map_err(|_| invalid("handshake timed out"))??
        .ok_or_else(|| invalid("missing handshake"))?;
    let hello: DaemonHello = match serde_json::from_slice(&hello) {
        Ok(h) => h,
        Err(_) => {
            send(stream.get_mut(), &DaemonWelcome::Unauthorized).await?;
            return Ok(());
        }
    };
    if !same_token(&hello.token, &metadata.token) {
        send(stream.get_mut(), &DaemonWelcome::Unauthorized).await?;
        return Ok(());
    }
    if hello.v != DAEMON_PROTOCOL_VERSION {
        send(
            stream.get_mut(),
            &DaemonWelcome::Version {
                supported: vec![DAEMON_PROTOCOL_VERSION],
            },
        )
        .await?;
        return Ok(());
    }
    send(
        stream.get_mut(),
        &DaemonWelcome::Accepted {
            v: DAEMON_PROTOCOL_VERSION,
        },
    )
    .await?;
    match hello.channel {
        vellum_types::daemon::DaemonChannel::Control => {
            let request = tokio::time::timeout(DEADLINE, frame(&mut stream, 1024))
                .await
                .map_err(|_| invalid("control timed out"))??
                .ok_or_else(|| invalid("missing command"))?;
            let command: serde_json::Value =
                serde_json::from_slice(&request).map_err(|_| invalid("invalid control command"))?;
            match command["method"].as_str() {
                Some("status")=>send(stream.get_mut(),&serde_json::json!({"v":metadata.v,"pid":metadata.pid,"endpoint":metadata.endpoint,"tcp":metadata.tcp})).await?,
                Some("shutdown")=> { send(stream.get_mut(),&serde_json::json!({"stopping":true})).await?; let _=shutdown.send(true); },
                _=>return Err(invalid("unknown control command")),
            }
        }
        vellum_types::daemon::DaemonChannel::Mcp => {
            // Only an absolute directory means anything; anything else is a client that does not know its own.
            let cwd = hello
                .cwd
                .filter(|cwd| cwd.len() <= MAX_CWD && std::path::Path::new(cwd).is_absolute());
            worker.opened(conn, cwd);
            while let Some(bytes) = frame(&mut stream, MAX_FRAME).await? {
                // A blank line is the private transport end marker. It gives named pipes an
                // application-level half-close; the bridge never forwards it to MCP stdout.
                if bytes == b"\n" {
                    break;
                }
                if let Some(response) = rpc(&bytes, worker, conn).await {
                    send(stream.get_mut(), &response).await?;
                }
            }
            stream.get_mut().write_all(b"\n").await?;
            stream.get_mut().flush().await?;
            // Keep a named-pipe instance alive until the bridge has read the end marker
            // and closed. Otherwise CloseHandle can discard unread response bytes.
            let _ = tokio::time::timeout(DEADLINE, stream.read_u8()).await;
        }
    }
    stream.get_mut().shutdown().await?;
    Ok(())
}

pub(crate) fn same_token(a: &str, b: &str) -> bool {
    let mut difference = a.len() ^ b.len();
    for i in 0..64 {
        difference |= usize::from(
            a.as_bytes().get(i).copied().unwrap_or(0) ^ b.as_bytes().get(i).copied().unwrap_or(0),
        );
    }
    difference == 0
}

// One bounded HTTP POST per connection. No CORS, redirects, cookies, chunking or websocket upgrade.
// An IP-literal Host must exactly match the bound address. Origin, when present, is exact-allowlisted.
async fn http(
    stream: tokio::net::TcpStream,
    metadata: &Metadata,
    origins: &[String],
    worker: &Worker,
) -> io::Result<()> {
    let mut stream = BufReader::new(stream);
    // The deadline bounds reading the request, not the tool that answers it.
    let result = tokio::time::timeout(DEADLINE, http_request(&mut stream, metadata, origins))
        .await
        .unwrap_or(Err(408));
    let (code, body) = match result {
        Ok(bytes) => {
            let conn = worker.connection();
            let answer = rpc(&bytes, worker, conn).await;
            worker.closed(conn);
            (
                200,
                answer
                    .map(|r| serde_json::to_vec(&r).unwrap())
                    .unwrap_or_default(),
            )
        }
        Err(code) => (code, Vec::new()),
    };
    let header = format!(
        "HTTP/1.1 {code} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.get_mut().write_all(header.as_bytes()).await?;
    stream.get_mut().write_all(&body).await?;
    stream.get_mut().shutdown().await
}
async fn http_request(
    stream: &mut BufReader<tokio::net::TcpStream>,
    metadata: &Metadata,
    origins: &[String],
) -> Result<Vec<u8>, u16> {
    let first = frame(stream, 4096)
        .await
        .map_err(|_| 400_u16)?
        .ok_or(400_u16)?;
    if first != b"POST /rpc HTTP/1.1\r\n" {
        return Err(400);
    }
    let mut headers = std::collections::BTreeMap::new();
    let mut total = first.len();
    loop {
        let bytes = frame(stream, 8192)
            .await
            .map_err(|_| 400_u16)?
            .ok_or(400_u16)?;
        total += bytes.len();
        if total > 16384 {
            return Err(431);
        }
        if bytes == b"\r\n" {
            break;
        }
        let line = std::str::from_utf8(&bytes)
            .map_err(|_| 400_u16)?
            .strip_suffix("\r\n")
            .ok_or(400_u16)?;
        let (key, value) = line.split_once(':').ok_or(400_u16)?;
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(400);
        }
        if headers
            .insert(key.to_ascii_lowercase(), value.trim().to_owned())
            .is_some()
        {
            return Err(400);
        }
    }
    if !headers
        .get("authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| same_token(t, &metadata.token))
    {
        return Err(401);
    }
    if headers.get("host") != metadata.tcp.as_ref() {
        return Err(403);
    }
    if let Some(origin) = headers.get("origin")
        && (origin == "null" || !origins.contains(origin))
    {
        return Err(403);
    }
    if headers.contains_key("transfer-encoding") {
        return Err(400);
    }
    let length = headers
        .get("content-length")
        .ok_or(411_u16)?
        .parse::<usize>()
        .map_err(|_| 400_u16)?;
    if length > MAX_FRAME {
        return Err(413);
    }
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await.map_err(|_| 400_u16)?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};
    #[test]
    fn local_handshake_authenticates_before_version_and_never_echoes_credentials() {
        runtime().unwrap().block_on(async {
            for (hello, expected) in [
                (serde_json::json!({"v":1,"token":"bad","channel":"control"}), "unauthorized"),
                (serde_json::json!({"v":99,"token":"a".repeat(64),"channel":"control"}), "version"),
                (serde_json::json!({"v":1,"token":"a".repeat(64),"channel":"mcp"}), "accepted"),
                (serde_json::json!({"v":1,"token":"a".repeat(64),"channel":"mcp","unknown":true}), "unauthorized"),
            ] {
                let (client,server)=tokio::io::duplex(8192);
                let metadata=Arc::new(Metadata { v:1,pid:1,endpoint:String::new(),token:"a".repeat(64),tcp:None });
                let (shutdown,_)=watch::channel(false);
                let worker=Arc::new(Worker::start(crate::no_tools()).unwrap());
                let task=tokio::spawn(async move { session(Box::new(server),metadata,shutdown,&worker,1).await });
                let mut client=BufReader::new(client); send(client.get_mut(),&hello).await.unwrap();
                let reply=frame(&mut client,4096).await.unwrap().unwrap();
                assert!(std::str::from_utf8(&reply).unwrap().contains(expected));
                assert!(!reply.windows(64).any(|w|w==b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
                client.get_mut().shutdown().await.unwrap(); let _=task.await.unwrap();
            }
        });
    }
}
