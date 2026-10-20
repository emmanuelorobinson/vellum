//! TEST DATA. @network: real loopback WebSockets and the production room service.
use std::sync::Arc;
use vellum_contracts::{fakes::SequentialIds, samples::*};
use vellum_core::{
    ops,
    ports::{DocStore, SecretString, SyncCursor},
};
use vellum_sync_server::{SyncService, memory::*};
use vellum_types::sync::*;
type Socket = vellum_sync_ws::testing::Connection;
fn connect(address: std::net::SocketAddr, key: &str) -> Socket {
    let mut socket = Socket::connect(
        address,
        &RoomId::new("socket-room").unwrap(),
        &SecretString::new(key),
    )
    .unwrap();
    send(&mut socket, 0, SyncRequest::Pull { after: 0 });
    assert!(matches!(
        read(&mut socket).body,
        SyncReply::State {
            snapshot: Some(_),
            ..
        }
    ));
    socket
}
fn send(socket: &mut Socket, epoch: u64, request: SyncRequest) {
    socket
        .send(&ClientFrame {
            v: 1,
            adapter: SYNC_ADAPTER.into(),
            epoch,
            request,
        })
        .unwrap();
}
fn read(socket: &mut Socket) -> ServerFrame {
    socket.read().unwrap()
}
#[test]
#[ignore = "@network: loopback TCP listener required"]
fn websocket_broadcasts_across_servers_rechecks_roles_and_resets_after_compaction() {
    let log = Arc::new(MemoryLog::default());
    let snapshots = Arc::new(MemorySnapshots::default());
    let acl = Arc::new(MemoryAccess::default());
    let room = RoomId::new("socket-room").unwrap();
    acl.set(
        "test-alice",
        &room,
        Grant {
            actor: alice(),
            role: Role::Editor,
        },
    );
    acl.set(
        "test-bob",
        &room,
        Grant {
            actor: bob(),
            role: Role::Viewer,
        },
    );
    let service = Arc::new(SyncService::new(
        log.clone(),
        snapshots.clone(),
        acl.clone(),
    ));
    service.seed(&room, &document()).unwrap();
    let server = vellum_sync_ws::Server::start(
        "127.0.0.1:0".parse().unwrap(),
        service.clone(),
        Some(Arc::new(MemoryPresence::default())),
        "test-instance".into(),
        vec![],
    )
    .unwrap();
    let mut writer = connect(server.address(), "test-alice");
    let mut viewer = connect(server.address(), "test-bob");
    let mut client = vellum_doc_loro::LoroDocStore::new(&document(), 1);
    ops::apply(
        &mut client,
        &mut SequentialIds::new(),
        tx(&alice(), vec![rename("rect000001", "TEST DATA: live")]),
    )
    .unwrap();
    let payload = client.export_updates(&SyncCursor::empty());
    send(
        &mut viewer,
        1,
        SyncRequest::Push {
            bytes: payload.bytes.clone(),
        },
    );
    assert!(matches!(read(&mut viewer).body, SyncReply::Error { code } if code == "forbidden"));
    send(
        &mut writer,
        1,
        SyncRequest::Push {
            bytes: payload.bytes,
        },
    );
    assert!(matches!(read(&mut writer).body, SyncReply::Ack));
    let broadcast = read(&mut viewer);
    let SyncReply::State { updates, .. } = broadcast.body else {
        panic!("durable broadcast");
    };
    let mut receiver = vellum_doc_loro::LoroDocStore::new(&document(), 3);
    for bytes in updates {
        receiver
            .import_updates(&vellum_core::ports::SyncPayload {
                adapter: SYNC_ADAPTER.into(),
                bytes,
            })
            .unwrap();
    }
    assert_eq!(receiver.snapshot(), client.snapshot());
    // A second process/composition root writes the shared durable log. The first server must deliver it.
    let other = SyncService::new(log, snapshots, acl.clone());
    acl.set(
        "test-bob",
        &room,
        Grant {
            actor: bob(),
            role: Role::Editor,
        },
    );
    ops::apply(
        &mut receiver,
        &mut SequentialIds::new(),
        tx(
            &bob(),
            vec![rename("rect000002", "TEST DATA: other server")],
        ),
    )
    .unwrap();
    other
        .push(
            &SecretString::new("test-bob"),
            &room,
            1,
            &receiver.export_updates(&SyncCursor::empty()),
            0,
        )
        .unwrap();
    assert!(
        matches!(read(&mut viewer).body, SyncReply::State { updates, .. } if !updates.is_empty())
    );
    acl.set(
        "test-bob",
        &room,
        Grant {
            actor: bob(),
            role: Role::Viewer,
        },
    );
    send(
        &mut viewer,
        1,
        SyncRequest::Push {
            bytes: receiver.export_updates(&SyncCursor::empty()).bytes,
        },
    );
    assert!(matches!(read(&mut viewer).body, SyncReply::Error { code } if code == "forbidden"));
    service
        .compact(&SecretString::new("test-alice"), &room, 1, 0)
        .unwrap();
    let reset = read(&mut viewer);
    assert_eq!(reset.epoch, 2);
    assert!(matches!(
        reset.body,
        SyncReply::State {
            snapshot: Some(_),
            ..
        }
    ));
    writer.close().unwrap();
    viewer.close().unwrap();
    server.stop();
}
