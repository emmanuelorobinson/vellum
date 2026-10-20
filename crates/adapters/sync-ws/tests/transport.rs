//! TEST DATA. @network: loopback WebSocket acceptance, no vendor account.
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
struct Endpoint(AtomicU64);
impl SyncEndpoint for Endpoint {
    fn authorize(&self, key: &SecretString, _: &RoomId, _: i64) -> Result<Grant, Error> {
        if key.expose() != "test-transport-credential" {
            return Err(Error::Forbidden);
        }
        Ok(Grant {
            actor: vellum_contracts::samples::alice(),
            role: Role::Editor,
        })
    }
    fn handle(
        &self,
        key: &SecretString,
        room: &RoomId,
        frame: ClientFrame,
        now: i64,
    ) -> ServerFrame {
        self.authorize(key, room, now).unwrap();
        let revision = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        ServerFrame {
            v: 1,
            adapter: SYNC_ADAPTER.into(),
            epoch: 1,
            revision,
            body: match frame.request {
                SyncRequest::Pull { .. } => SyncReply::State {
                    snapshot: Some(b"TEST DATA: canonical".to_vec()),
                    updates: vec![],
                },
                SyncRequest::Push { .. } => SyncReply::Ack,
                SyncRequest::Comment { .. }
                | SyncRequest::Presence { .. }
                | SyncRequest::Sharing { .. } => SyncReply::Error {
                    code: "invalid_update".into(),
                },
            },
        }
    }
}
#[test]
#[ignore = "@network: permits a local TCP listener"]
fn authenticated_bounded_socket_and_origin_refusal() {
    use tungstenite::{Message, client::IntoClientRequest};
    let server = vellum_sync_ws::Server::start(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Endpoint(AtomicU64::new(0))),
        None,
        "test-instance".into(),
        vec!["https://allowed.example".into()],
    )
    .unwrap();
    let url = format!("ws://{}/v1/rooms/test-room", server.address());
    let mut request = url.clone().into_client_request().unwrap();
    request.headers_mut().insert(
        "Authorization",
        "Bearer test-transport-credential".parse().unwrap(),
    );
    request
        .headers_mut()
        .insert("Origin", "https://forbidden.example".parse().unwrap());
    assert!(tungstenite::connect(request).is_err());
    assert!(tungstenite::connect(&url).is_err());
    let mut request = url.into_client_request().unwrap();
    request.headers_mut().insert(
        "Authorization",
        "Bearer test-transport-credential".parse().unwrap(),
    );
    let (mut ws, _) = tungstenite::connect(request).unwrap();
    ws.send(Message::Text(
        serde_json::to_string(&ClientFrame {
            v: 1,
            adapter: SYNC_ADAPTER.into(),
            epoch: 0,
            request: SyncRequest::Pull { after: 0 },
        })
        .unwrap()
        .into(),
    ))
    .unwrap();
    let frame: ServerFrame = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
    assert!(matches!(
        frame.body,
        SyncReply::State {
            snapshot: Some(_),
            ..
        }
    ));
    ws.close(None).unwrap();
    server.stop();
}

#[test]
#[ignore = "@network: permits a local TCP listener"]
fn a_newer_envelope_with_unknown_request_fields_gets_an_upgrade_error() {
    use tungstenite::{Message, client::IntoClientRequest};
    let server = vellum_sync_ws::Server::start(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Endpoint(AtomicU64::new(0))),
        None,
        "test-future-instance".into(),
        vec![],
    )
    .unwrap();
    let mut request = format!("ws://{}/v1/rooms/test-room", server.address())
        .into_client_request()
        .unwrap();
    request.headers_mut().insert(
        "Authorization",
        "Bearer test-transport-credential".parse().unwrap(),
    );
    let (mut ws, _) = tungstenite::connect(request).unwrap();
    ws.send(Message::Text(r#"{"v":2,"adapter":"loro-1","epoch":0,"request":{"type":"future_request","new_field":true}}"#.into())).unwrap();
    let reply: ServerFrame = serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
    assert!(matches!(reply.body, SyncReply::Error { code } if code == "upgrade_required"));
    let _ = ws.close(None);
    server.stop();
}
