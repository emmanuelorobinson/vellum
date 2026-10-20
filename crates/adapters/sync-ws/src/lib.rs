//! Authenticated bounded WebSocket transport. Blocking storage/core work runs outside Tokio's I/O workers.
use axum::{
    Router,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Semaphore, oneshot};
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
const MAX_WIRE: usize = MAX_UPDATE_BYTES * 4 + 4096;
#[derive(Clone)]
struct App {
    endpoint: Arc<dyn SyncEndpoint>,
    presence: Option<Arc<dyn Presence>>,
    instance: String,
    origins: Arc<Vec<String>>,
    connections: Arc<Semaphore>,
    workers: Arc<Semaphore>,
    next: Arc<AtomicU64>,
}
pub struct Server {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    /// Starting a service requires an explicit address. This never runs from the local daemon.
    pub fn start(
        address: SocketAddr,
        endpoint: Arc<dyn SyncEndpoint>,
        presence: Option<Arc<dyn Presence>>,
        instance: String,
        origins: Vec<String>,
    ) -> Result<Self, Error> {
        RoomId::new(instance.clone()).map_err(|_| Error::InvalidUpdate)?;
        let listener = std::net::TcpListener::bind(address).map_err(|_| Error::Unavailable)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| Error::Unavailable)?;
        let address = listener.local_addr().map_err(|_| Error::Unavailable)?;
        let (stop, stopped) = oneshot::channel();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(8)
            .thread_stack_size(16 * 1024 * 1024)
            .enable_all()
            .build()
            .map_err(|_| Error::Unavailable)?;
        let app = App {
            endpoint,
            presence,
            instance,
            origins: Arc::new(origins),
            connections: Arc::new(Semaphore::new(32)),
            workers: Arc::new(Semaphore::new(8)),
            next: Arc::new(AtomicU64::new(1)),
        };
        let thread = std::thread::spawn(move || {
            runtime.block_on(async move {
                let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
                    return;
                };
                let router = Router::new()
                    .route("/health", get(|| async { "ok" }))
                    .route("/v1/rooms/{room}", get(upgrade))
                    .with_state(app);
                let _ = axum::serve(listener, router)
                    .with_graceful_shutdown(async {
                        let _ = stopped.await;
                    })
                    .await;
            });
            runtime.shutdown_timeout(Duration::from_secs(6));
        });
        Ok(Self {
            address,
            stop: Some(stop),
            thread: Some(thread),
        })
    }
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    pub fn stop(mut self) {
        self.finish();
    }
    fn finish(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.finish();
    }
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}
fn credential(headers: &HeaderMap) -> Result<SecretString, StatusCode> {
    let mut auth = headers.get_all("authorization").iter();
    let value = auth
        .next()
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if auth.next().is_some()
        || value.is_empty()
        || value.len() > 4096
        || !value.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(SecretString::new(value))
}
async fn upgrade(
    State(app): State<App>,
    Path(room): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if headers.contains_key("cookie") {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if let Some(origin) = headers.get("origin")
        && !origin
            .to_str()
            .is_ok_and(|v| app.origins.iter().any(|o| o == v))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(room) = RoomId::new(room) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(credential) = credential(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Ok(permit) = app.connections.clone().try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let endpoint = app.endpoint.clone();
    let key = credential.clone();
    let id = room.clone();
    let grant = tokio::task::spawn_blocking(move || endpoint.authorize(&key, &id, now())).await;
    if !matches!(grant, Ok(Ok(_))) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.max_message_size(MAX_WIRE)
        .max_frame_size(MAX_WIRE)
        .max_write_buffer_size(192 * 1024 * 1024)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            session(socket, app, credential, room).await
        })
}
async fn dispatch(
    app: &App,
    key: &SecretString,
    room: &RoomId,
    frame: ClientFrame,
) -> Result<ServerFrame, ()> {
    let permit = app.workers.clone().acquire_owned().await.map_err(|_| ())?;
    let endpoint = app.endpoint.clone();
    let key = key.clone();
    let room = room.clone();
    tokio::time::timeout(
        Duration::from_secs(15),
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            endpoint.handle(&key, &room, frame, now())
        }),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())
}
async fn send(socket: &mut WebSocket, frame: &ServerFrame) -> Result<(), ()> {
    let text = serde_json::to_string(frame).map_err(|_| ())?;
    tokio::time::timeout(
        Duration::from_secs(5),
        socket.send(Message::Text(text.into())),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())
}
#[derive(serde::Deserialize)]
struct EnvelopeVersion {
    v: u32,
    adapter: String,
}

fn rejected(error: Error) -> ServerFrame {
    ServerFrame {
        v: SYNC_VERSION,
        adapter: SYNC_ADAPTER.into(),
        epoch: 0,
        revision: 0,
        body: SyncReply::Error {
            code: error.code().into(),
        },
    }
}

async fn session(mut socket: WebSocket, app: App, key: SecretString, room: RoomId) {
    let connection = format!(
        "{}-{}",
        app.instance,
        app.next.fetch_add(1, Ordering::Relaxed)
    );
    let mut epoch = 0;
    let mut revision = 0;
    let mut initialized = false;
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_seen = tokio::time::Instant::now();
    let mut last_ping = tokio::time::Instant::now();
    let mut last_presence = tokio::time::Instant::now() - Duration::from_secs(10);
    loop {
        let incoming =
            tokio::select! { message = socket.recv() => Some(message), _ = tick.tick() => None };
        let frame = match incoming {
            Some(Some(Ok(Message::Text(text)))) => {
                last_seen = tokio::time::Instant::now();
                // Inspect the stable envelope before decoding this version's request union.
                let Ok(version) = serde_json::from_str::<EnvelopeVersion>(&text) else {
                    let _ = send(&mut socket, &rejected(Error::InvalidUpdate)).await;
                    break;
                };
                if version.v != SYNC_VERSION || version.adapter != SYNC_ADAPTER {
                    let _ = send(&mut socket, &rejected(Error::UpgradeRequired)).await;
                    break;
                }
                let Ok(frame) = serde_json::from_str::<ClientFrame>(&text) else {
                    let _ = send(&mut socket, &rejected(Error::InvalidUpdate)).await;
                    break;
                };
                if !initialized && !matches!(frame.request, SyncRequest::Pull { .. }) {
                    break;
                }
                frame
            }
            Some(Some(Ok(Message::Pong(_)))) => {
                last_seen = tokio::time::Instant::now();
                continue;
            }
            Some(Some(Ok(Message::Ping(_)))) => continue,
            Some(_) => break,
            None => {
                if last_seen.elapsed() > Duration::from_secs(30) {
                    break;
                }
                if !initialized {
                    if last_seen.elapsed() > Duration::from_secs(5) {
                        break;
                    }
                    continue;
                }
                if last_ping.elapsed() > Duration::from_secs(10) {
                    if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                        break;
                    }
                    last_ping = tokio::time::Instant::now();
                }
                ClientFrame {
                    v: SYNC_VERSION,
                    adapter: SYNC_ADAPTER.into(),
                    epoch,
                    request: SyncRequest::Pull { after: revision },
                }
            }
        };
        let is_pull = matches!(frame.request, SyncRequest::Pull { .. });
        let Ok(reply) = dispatch(&app, &key, &room, frame).await else {
            break;
        };
        let fatal = matches!(&reply.body, SyncReply::Error { code } if (is_pull && code == "forbidden") || code == "upgrade_required");
        let unchanged = is_pull
            && initialized
            && reply.epoch == epoch
            && reply.revision == revision
            && matches!(&reply.body, SyncReply::State { snapshot: None, updates } if updates.is_empty());
        if !unchanged && send(&mut socket, &reply).await.is_err() {
            break;
        }
        if fatal {
            break;
        }
        if matches!(reply.body, SyncReply::State { .. }) {
            epoch = reply.epoch;
            revision = reply.revision;
            initialized = true;
        }
        if last_presence.elapsed() > Duration::from_secs(5) {
            if let Some(presence) = app.presence.clone() {
                let endpoint = app.endpoint.clone();
                let key = key.clone();
                let id = room.clone();
                let connection = connection.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let now = now();
                    let grant = endpoint.authorize(&key, &id, now)?;
                    presence.touch(
                        &id,
                        &PresenceEntry {
                            connection,
                            actor: grant.actor,
                            expires_ms: now.saturating_add(30_000),
                        },
                        now,
                    )
                })
                .await;
                if !matches!(result, Ok(Ok(()))) {
                    break;
                }
            }
            last_presence = tokio::time::Instant::now();
        }
    }
    if let Some(presence) = app.presence {
        let _ = tokio::task::spawn_blocking(move || presence.remove(&room, &connection)).await;
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.send(Message::Close(None))).await;
}
/// Wait for an OS interrupt in the service's composition root, with no leaked Tokio type.
pub fn wait_for_shutdown() -> Result<(), Error> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_| Error::Unavailable)?.block_on(async {
        #[cfg(unix)] {
            let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).map_err(|_| Error::Unavailable)?;
            tokio::select! { result = tokio::signal::ctrl_c() => result.map_err(|_| Error::Unavailable), _ = terminate.recv() => Ok(()) }
        }
        #[cfg(not(unix))] { tokio::signal::ctrl_c().await.map_err(|_| Error::Unavailable) }
    })
}

/// A test-only client keeps the vendor WebSocket API inside its adapter.
#[cfg(feature = "test-client")]
pub mod testing {
    use super::*;
    use tungstenite::{Message, WebSocket, client::IntoClientRequest, stream::MaybeTlsStream};
    pub struct Connection(WebSocket<MaybeTlsStream<std::net::TcpStream>>);
    impl Connection {
        pub fn connect(
            address: SocketAddr,
            room: &RoomId,
            key: &SecretString,
        ) -> Result<Self, Error> {
            if !address.ip().is_loopback() {
                return Err(Error::Forbidden);
            }
            let mut request = format!("ws://{address}/v1/rooms/{}", room.as_str())
                .into_client_request()
                .map_err(|_| Error::Unavailable)?;
            request.headers_mut().insert(
                "Authorization",
                format!("Bearer {}", key.expose())
                    .parse()
                    .map_err(|_| Error::Forbidden)?,
            );
            let (mut socket, _) = tungstenite::connect(request).map_err(|_| Error::Unavailable)?;
            if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .map_err(|_| Error::Unavailable)?;
            }
            Ok(Self(socket))
        }
        pub fn send(&mut self, frame: &ClientFrame) -> Result<(), Error> {
            self.0
                .send(Message::Text(
                    serde_json::to_string(frame)
                        .map_err(|_| Error::InvalidUpdate)?
                        .into(),
                ))
                .map_err(|_| Error::Unavailable)
        }
        pub fn read(&mut self) -> Result<ServerFrame, Error> {
            loop {
                match self.0.read().map_err(|_| Error::Unavailable)? {
                    Message::Text(text) => {
                        return serde_json::from_str(&text).map_err(|_| Error::InvalidUpdate);
                    }
                    Message::Ping(bytes) => self
                        .0
                        .send(Message::Pong(bytes))
                        .map_err(|_| Error::Unavailable)?,
                    _ => {}
                }
            }
        }
        pub fn close(&mut self) -> Result<(), Error> {
            self.0.close(None).map_err(|_| Error::Unavailable)
        }
    }
}
