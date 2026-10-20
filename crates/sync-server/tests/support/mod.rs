//! TEST DATA. One wire scenario reused verbatim by self-host and cloud compositions.
use vellum_contracts::{fakes::SequentialIds, samples::*};
use vellum_core::{
    ops,
    ports::{DocStore, SecretString, SyncCursor, SyncPayload},
};
use vellum_doc_loro::LoroDocStore;
use vellum_types::{Actor, Point, comments::CommentRequest, sync::*};
type Socket = vellum_sync_ws::testing::Connection;
pub fn actors() -> [Actor; 4] {
    [
        alice(),
        bob(),
        Actor::Agent { agent: teal() },
        Actor::Agent { agent: rose() },
    ]
}
pub fn keys() -> [SecretString; 4] {
    [
        "TEST-DATA-person-one",
        "TEST-DATA-person-two",
        "TEST-DATA-agent-one",
        "TEST-DATA-agent-two",
    ]
    .map(SecretString::new)
}
fn send(socket: &mut Socket, request: SyncRequest) {
    socket
        .send(&ClientFrame {
            v: SYNC_VERSION,
            adapter: SYNC_ADAPTER.into(),
            epoch: 1,
            request,
        })
        .unwrap();
}
fn next(
    socket: &mut Socket,
    store: &mut LoroDocStore,
    predicate: impl Fn(&ServerFrame) -> bool,
) -> ServerFrame {
    for _ in 0..64 {
        let frame = socket.read().expect("bounded wire response");
        if let SyncReply::State { updates, .. } = &frame.body {
            for bytes in updates {
                store
                    .import_updates(&SyncPayload {
                        adapter: SYNC_ADAPTER.into(),
                        bytes: bytes.clone(),
                    })
                    .unwrap();
            }
        }
        if predicate(&frame) {
            return frame;
        }
    }
    panic!("expected frame was not delivered");
}
pub fn scenario(address: std::net::SocketAddr, room: &RoomId, keys: &[SecretString; 4]) {
    let mut clients: Vec<(Socket, LoroDocStore)> = keys
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let mut socket = Socket::connect(address, room, key).unwrap();
            socket
                .send(&ClientFrame {
                    v: SYNC_VERSION,
                    adapter: SYNC_ADAPTER.into(),
                    epoch: 0,
                    request: SyncRequest::Pull { after: 0 },
                })
                .unwrap();
            let frame = socket.read().unwrap();
            let SyncReply::State {
                snapshot: Some(bytes),
                updates,
            } = frame.body
            else {
                panic!("canonical bootstrap");
            };
            let doc = vellum_codec_json::decode(&bytes).unwrap();
            let mut store = LoroDocStore::new(&doc, 10 + i as u64);
            for bytes in updates {
                store
                    .import_updates(&SyncPayload {
                        adapter: SYNC_ADAPTER.into(),
                        bytes,
                    })
                    .unwrap();
            }
            (socket, store)
        })
        .collect();
    for (i, ((socket, store), actor)) in clients.iter_mut().zip(actors()).enumerate() {
        ops::apply(
            store,
            &mut SequentialIds::new(),
            tx(
                &actor,
                vec![rename(
                    ["rect000001", "rect000002", "rect000003", "frame00001"][i],
                    &format!("TEST DATA actor {i}"),
                )],
            ),
        )
        .unwrap();
        send(
            socket,
            SyncRequest::Push {
                bytes: store.export_updates(&SyncCursor::empty()).bytes,
            },
        );
        next(socket, store, |f| matches!(f.body, SyncReply::Ack));
        send(
            socket,
            SyncRequest::Presence {
                state: PresenceState {
                    name: format!("TEST DATA actor {i}"),
                    cursor: Some(Point {
                        x: i as f64,
                        y: 20.0,
                    }),
                    activity: "Viewing".into(),
                    ..Default::default()
                },
            },
        );
    }
    let (socket, store) = &mut clients[0];
    let presence = next(
        socket,
        store,
        |f| matches!(&f.body,SyncReply::Collaboration{participants,..} if participants.len()==4 && participants.iter().all(|p|p.state.as_ref().is_some_and(|s|s.cursor.is_some()))),
    );
    let SyncReply::Collaboration { participants, .. } = presence.body else {
        unreachable!()
    };
    assert_eq!(
        participants.iter().filter(|p| p.actor.is_person()).count(),
        2
    );
    assert_eq!(
        participants.iter().filter(|p| p.actor.is_agent()).count(),
        2
    );
    assert!(
        participants
            .iter()
            .all(|p| p.state.as_ref().is_some_and(|s| s.cursor.is_some()))
    );
    send(
        socket,
        SyncRequest::Comment {
            op: CommentRequest::Create {
                thread: node_id("wire-thread"),
                message: node_id("wire-message"),
                anchor: Some(node_id("rect000001")),
                page: None,
                point: Point { x: 10.0, y: 20.0 },
                text: "TEST DATA live review".into(),
            },
        },
    );
    next(socket, store, |f| matches!(f.body, SyncReply::Ack));
    for (socket, store) in &mut clients {
        send(socket, SyncRequest::Pull { after: 0 });
        next(socket, store, |f| {
            matches!(f.body, SyncReply::State { .. }) && f.revision >= 6
        });
        assert_eq!(store.snapshot().comments.len(), 1);
        for (i, id) in ["rect000001", "rect000002", "rect000003", "frame00001"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                find(&store.snapshot(), id).unwrap().name(),
                format!("TEST DATA actor {i}")
            );
        }
    }
    for (socket, _) in &mut clients {
        socket.close().unwrap();
    }
}
