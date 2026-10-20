//! TEST DATA. Authoritative comment policy and canonical recovery, same storage ports as cloud.
use std::sync::Arc;
use vellum_contracts::samples::*;
use vellum_core::ports::{DocStore, SecretString};
use vellum_doc_loro::LoroDocStore;
use vellum_sync_server::{SyncService, memory::*, ports::*};
use vellum_types::{NodeId, Point, comments::CommentRequest, sync::*};
fn room() -> RoomId {
    RoomId::new("comment-room").unwrap()
}
fn key(s: &str) -> SecretString {
    SecretString::new(s)
}
fn create() -> CommentRequest {
    CommentRequest::Create {
        thread: NodeId::new("thread1").unwrap(),
        message: NodeId::new("message1").unwrap(),
        anchor: Some(NodeId::new("rect000001").unwrap()),
        page: None,
        point: Point { x: 10.0, y: 20.0 },
        text: "TEST DATA".into(),
    }
}
#[test]
fn commenters_can_post_but_not_edit_and_snapshot_only_recovery_retains_threads() {
    let log = Arc::new(MemoryLog::default());
    let snapshots = Arc::new(MemorySnapshots::default());
    let access = Arc::new(MemoryAccess::default());
    access.set(
        "owner",
        &room(),
        Grant {
            actor: alice(),
            role: Role::Editor,
        },
    );
    access.set(
        "reader",
        &room(),
        Grant {
            actor: bob(),
            role: Role::Viewer,
        },
    );
    let svc = SyncService::new(log.clone(), snapshots.clone(), access.clone());
    svc.seed(&room(), &document()).unwrap();
    assert_eq!(
        svc.comment(&key("reader"), &room(), 1, create(), 1),
        Err(Error::Forbidden)
    );
    access.set(
        "reader",
        &room(),
        Grant {
            actor: bob(),
            role: Role::Commenter,
        },
    );
    assert_eq!(
        svc.comment(&key("reader"), &room(), 1, create(), 1)
            .unwrap(),
        2
    );
    assert_eq!(
        svc.comment(&key("reader"), &room(), 1, create(), 1)
            .unwrap(),
        2,
        "retried create is idempotent"
    );
    let mut received = LoroDocStore::new(&document(), 900);
    for update in svc.pull(&key("owner"), &room(), 1, 0, 2).unwrap().updates {
        received.import_updates(&update).unwrap();
    }
    assert_eq!(received.snapshot().comments.len(), 1);
    svc.compact(&key("owner"), &room(), 1, 3).unwrap();
    let snapshot = svc
        .pull(&key("reader"), &room(), 0, 0, 4)
        .unwrap()
        .snapshot
        .unwrap();
    let doc = vellum_codec_json::decode(&snapshot).unwrap();
    assert_eq!(doc.comments.len(), 1);
    let rebuilt = SyncService::new(
        Arc::new(MemoryLog::default()),
        Arc::new(MemorySnapshots::default()),
        access,
    );
    rebuilt.seed(&room(), &doc).unwrap();
    assert!(
        rebuilt
            .pull(&key("reader"), &room(), 0, 0, 5)
            .unwrap()
            .snapshot
            .is_some()
    );
}
