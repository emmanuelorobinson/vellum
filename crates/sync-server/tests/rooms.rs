//! TEST DATA. Deterministic room acceptance; no sockets or vendor services.
use proptest::prelude::*;
use std::sync::Arc;
use vellum_contracts::{fakes::SequentialIds, samples::*};
use vellum_core::{
    ops,
    ports::{DocStore, SecretString, SyncCursor},
};
use vellum_doc_loro::LoroDocStore;
use vellum_sync_server::{SyncService, memory::*, ports::*};
use vellum_types::{Actor, sync::*};

fn room() -> RoomId {
    RoomId::new("test-room").unwrap()
}
fn access(actor: Actor) -> Grant {
    Grant {
        actor,
        role: Role::Editor,
    }
}
fn setup() -> (
    SyncService,
    Arc<MemoryLog>,
    Arc<MemorySnapshots>,
    Arc<MemoryAccess>,
) {
    let log = Arc::new(MemoryLog::default());
    let snapshots = Arc::new(MemorySnapshots::default());
    let access = Arc::new(MemoryAccess::default());
    access.set("test-alice", &room(), access_grant(alice(), Role::Editor));
    access.set("test-bob", &room(), access_grant(bob(), Role::Viewer));
    let service = SyncService::new(log.clone(), snapshots.clone(), access.clone());
    service.seed(&room(), &document()).unwrap();
    (service, log, snapshots, access)
}
fn access_grant(actor: Actor, role: Role) -> Grant {
    Grant { actor, role }
}
fn token(value: &str) -> SecretString {
    SecretString::new(value)
}
fn push(service: &SyncService, who: &str, epoch: u64, store: &impl DocStore) -> Result<u64, Error> {
    service.push(
        &token(who),
        &room(),
        epoch,
        &store.export_updates(&SyncCursor::empty()),
        0,
    )
}
fn write(store: &mut LoroDocStore, actor: &Actor, name: &str) {
    ops::apply(
        store,
        &mut SequentialIds::new(),
        tx(actor, vec![rename("rect000001", name)]),
    )
    .unwrap();
}

#[test]
fn viewers_commenters_and_revoked_editors_cannot_write() {
    let (service, log, _, acl) = setup();
    let mut store = LoroDocStore::new(&document(), 1);
    write(&mut store, &bob(), "TEST DATA: denied");
    let before = log.load(&room()).unwrap().unwrap();
    assert_eq!(push(&service, "test-bob", 1, &store), Err(Error::Forbidden));
    acl.set("test-bob", &room(), access_grant(bob(), Role::Commenter));
    assert_eq!(push(&service, "test-bob", 1, &store), Err(Error::Forbidden));
    acl.set("test-alice", &room(), access_grant(alice(), Role::Viewer));
    assert_eq!(
        push(&service, "test-alice", 1, &store),
        Err(Error::Forbidden)
    );
    assert_eq!(log.load(&room()).unwrap().unwrap(), before);
}

#[test]
fn accepted_updates_are_durable_idempotent_and_room_scoped() {
    let (service, log, snapshots, acl) = setup();
    let mut store = LoroDocStore::new(&document(), 1);
    write(&mut store, &alice(), "TEST DATA: durable");
    let revision = push(&service, "test-alice", 1, &store).unwrap();
    assert_eq!(push(&service, "test-alice", 1, &store).unwrap(), revision);
    let reopened = SyncService::new(log, snapshots, acl);
    let read = reopened
        .pull(&token("test-alice"), &room(), 1, 0, 0)
        .unwrap();
    let mut receiver = LoroDocStore::new(&document(), 44);
    for update in read.updates {
        receiver.import_updates(&update).unwrap();
    }
    assert_eq!(receiver.snapshot(), store.snapshot());
    assert_eq!(
        reopened.pull(
            &token("test-alice"),
            &RoomId::new("other-room").unwrap(),
            1,
            0,
            0
        ),
        Err(Error::Forbidden)
    );
}

#[test]
fn permission_is_bound_to_every_new_changes_origin() {
    let (service, log, _, _) = setup();
    let mut store = LoroDocStore::new(&document(), 1);
    write(&mut store, &bob(), "TEST DATA: forged first change");
    write(
        &mut store,
        &alice(),
        "TEST DATA: apparently allowed last change",
    );
    let before = log.load(&room()).unwrap().unwrap();
    assert_eq!(
        push(&service, "test-alice", 1, &store),
        Err(Error::InvalidUpdate)
    );
    assert_eq!(log.load(&room()).unwrap().unwrap(), before);
}

#[test]
fn failed_persistence_does_not_acknowledge_or_change_the_room() {
    let (service, log, _, _) = setup();
    let mut store = LoroDocStore::new(&document(), 1);
    write(&mut store, &alice(), "TEST DATA: retry");
    log.fail_next_write();
    assert_eq!(
        push(&service, "test-alice", 1, &store),
        Err(Error::Unavailable)
    );
    assert_eq!(
        service
            .pull(&token("test-alice"), &room(), 1, 1, 0)
            .unwrap()
            .updates
            .len(),
        0
    );
    push(&service, "test-alice", 1, &store).unwrap();
}

#[test]
fn compaction_and_snapshot_only_rebuild_need_no_loro_bytes() {
    let (service, log, snapshots, acl) = setup();
    let mut store = LoroDocStore::new(&document(), 1);
    write(&mut store, &alice(), "TEST DATA: compacted");
    let revision = push(&service, "test-alice", 1, &store).unwrap();
    let epoch = service
        .compact(&token("test-alice"), &room(), 1, 0)
        .unwrap();
    assert_eq!(epoch, 2);
    assert_eq!(
        push(&service, "test-alice", 1, &store),
        Err(Error::ResetRequired)
    );
    let head = log.load(&room()).unwrap().unwrap();
    assert!(head.updates.is_empty());
    let canonical = snapshots.get(&head.snapshot).unwrap();
    let restored_id = RoomId::new("test-restored").unwrap();
    acl.set("test-alice", &restored_id, access(alice()));
    let restored = SyncService::new(
        Arc::new(MemoryLog::default()),
        Arc::new(MemorySnapshots::default()),
        acl,
    );
    let doc = vellum_codec_json::JsonCodec::new()
        .decode(&canonical)
        .unwrap();
    restored.seed(&restored_id, &doc).unwrap();
    let bootstrap = restored
        .pull(&token("test-alice"), &restored_id, 0, 0, 0)
        .unwrap();
    assert_eq!(bootstrap.snapshot.as_deref(), Some(canonical.as_slice()));
    let mut resumed = LoroDocStore::new(&doc, 51);
    write(
        &mut resumed,
        &alice(),
        "TEST DATA: edit after snapshot-only recovery",
    );
    restored
        .push(
            &token("test-alice"),
            &restored_id,
            1,
            &resumed.export_updates(&SyncCursor::empty()),
            0,
        )
        .unwrap();
    let reset = service
        .pull(&token("test-alice"), &room(), 1, revision, 0)
        .unwrap();
    assert_eq!(reset.snapshot, Some(canonical));
    assert_eq!(doc, store.snapshot());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn partitions_random_operations_and_n_clients_converge(
        count in 2usize..7,
        actions in prop::collection::vec((0usize..30, 0u8..6, any::<u16>()), 1..100)
    ) {
        let log = Arc::new(MemoryLog::default());
        let snapshots = Arc::new(MemorySnapshots::default());
        let acl = Arc::new(MemoryAccess::default());
        let service = SyncService::new(log, snapshots, acl.clone());
        service.seed(&room(), &document()).unwrap();
        let mut clients: Vec<_> = (0..count).map(|i| LoroDocStore::new(&document(), i as u64 + 1)).collect();
        let actors: Vec<_> = (0..count).map(|i| Actor::Person { device: vellum_types::DeviceId::new(format!("{:026}", i+1)).unwrap() }).collect();
        for (i, actor) in actors.iter().enumerate() { acl.set(&format!("test-{i}"), &room(), access(actor.clone())); }
        for (step, (i, kind, value)) in actions.into_iter().enumerate() {
            let i = i % count;
            let op = match kind {
                0 => rename("rect000001", &format!("TEST DATA: {value}")),
                1 => ops::Op::Update { id: node_id("rect000002"), patch: patch(serde_json::json!({"locked": value % 2 == 0})) },
                2 => insert_at("page00002", None, 0, node("rect", &format!("n{i:03}{step:06}"))),
                3 => ops::Op::Move { id: node_id("rect000001"), to: ops::Place { parent: ops::Parent::Page(page_id("page00002")), index: 0 } },
                4 if find(&clients[i].snapshot(), "rect000003").is_some() => ops::Op::Delete { id:node_id("rect000003") },
                _ => { // Network delivery is withheld on other steps: each replica can edit while partitioned.
                    push(&service, &format!("test-{i}"), 1, &clients[i]).unwrap();
                    let read = service.pull(&token(&format!("test-{i}")), &room(), 1, 0, 0).unwrap();
                    for update in read.updates { clients[i].import_updates(&update).unwrap(); }
                    continue;
                }
            };
            ops::apply(&mut clients[i], &mut SequentialIds::new(), tx(&actors[i], vec![op])).unwrap();
        }
        for (i, client) in clients.iter().enumerate() { push(&service, &format!("test-{i}"), 1, client).unwrap(); }
        let all = service.pull(&token("test-0"), &room(), 1, 0, 0).unwrap();
        for client in &mut clients { for update in &all.updates { client.import_updates(update).unwrap(); } }
        for client in &clients { prop_assert_eq!(client.snapshot(), clients[0].snapshot()); }
    }
}

#[test]
fn protocol_version_adapter_and_invalid_payload_errors_are_explicit() {
    let (service, _, _, _) = setup();
    for (v, adapter) in [(99, "loro-1"), (1, "different")] {
        let reply = service.handle(
            &token("test-alice"),
            &room(),
            ClientFrame {
                v,
                adapter: adapter.into(),
                epoch: 1,
                request: SyncRequest::Pull { after: 0 },
            },
            0,
        );
        assert!(matches!(reply.body, SyncReply::Error { code } if code == "upgrade_required"));
    }
    let payload = vellum_core::ports::SyncPayload {
        adapter: SYNC_ADAPTER.into(),
        bytes: vec![1, 2, 3],
    };
    assert_eq!(
        service.push(&token("test-alice"), &room(), 1, &payload, 0),
        Err(Error::InvalidUpdate)
    );
}

struct FailingSnapshots {
    inner: MemorySnapshots,
    fail: std::sync::atomic::AtomicBool,
}
impl SnapshotStore for FailingSnapshots {
    fn put(&self, bytes: &[u8]) -> Result<String, Error> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            Err(Error::Unavailable)
        } else {
            self.inner.put(bytes)
        }
    }
    fn get(&self, key: &str) -> Result<Vec<u8>, Error> {
        self.inner.get(key)
    }
}
#[test]
fn compaction_failure_never_prunes_the_durable_tail() {
    let log = Arc::new(MemoryLog::default());
    let snapshots = Arc::new(FailingSnapshots {
        inner: MemorySnapshots::default(),
        fail: std::sync::atomic::AtomicBool::new(false),
    });
    let acl = Arc::new(MemoryAccess::default());
    acl.set("test-alice", &room(), access(alice()));
    let service = SyncService::new(log.clone(), snapshots.clone(), acl);
    service.seed(&room(), &document()).unwrap();
    let mut store = LoroDocStore::new(&document(), 1);
    write(&mut store, &alice(), "TEST DATA: retain");
    push(&service, "test-alice", 1, &store).unwrap();
    let before = log.load(&room()).unwrap().unwrap();
    snapshots
        .fail
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        service.compact(&token("test-alice"), &room(), 1, 0),
        Err(Error::Unavailable)
    );
    assert_eq!(log.load(&room()).unwrap().unwrap(), before);
    snapshots
        .fail
        .store(false, std::sync::atomic::Ordering::SeqCst);
    log.fail_next_write();
    assert_eq!(
        service.compact(&token("test-alice"), &room(), 1, 0),
        Err(Error::Unavailable)
    );
    assert_eq!(log.load(&room()).unwrap().unwrap(), before);
    assert_eq!(service.compact(&token("test-alice"), &room(), 1, 0), Ok(2));
}

#[test]
fn documents_that_cannot_reopen_from_canonical_json_are_never_published() {
    // Building the deliberately deep typed fixture needs the same bounded stack as native validation.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let (service, log, _, _) = setup();
            let mut branch = node("rect", "deepLeaf01");
            for i in 0..80 {
                branch = node_with(
                    "frame",
                    &format!("nest{i:06}"),
                    &[branch],
                    serde_json::json!({}),
                );
            }
            let mut deep = document();
            deep.pages[1].children.push(branch.clone());
            assert!(vellum_core::ops::check_document(&deep).is_empty());
            let codec = vellum_codec_json::JsonCodec::new();
            assert!(
                codec.decode(&codec.encode(&deep).unwrap()).is_err(),
                "fixture exceeds the codec's nesting limit"
            );
            let deep_room = RoomId::new("deep-room").unwrap();
            assert_eq!(service.seed(&deep_room, &deep), Err(Error::InvalidUpdate));
            assert_eq!(log.load(&deep_room).unwrap(), None);
            let mut client = LoroDocStore::new(&document(), 71);
            ops::apply(
                &mut client,
                &mut SequentialIds::new(),
                tx(&alice(), vec![insert_at("page00002", None, 0, branch)]),
            )
            .unwrap();
            let before = log.load(&room()).unwrap();
            assert_eq!(
                push(&service, "test-alice", 1, &client),
                Err(Error::InvalidUpdate)
            );
            assert_eq!(log.load(&room()).unwrap(), before);
        })
        .unwrap()
        .join()
        .unwrap();
}
