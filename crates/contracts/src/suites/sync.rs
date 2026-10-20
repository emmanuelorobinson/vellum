//! Shared sync storage contracts. Call with a fresh store or a unique room in a shared integration store.
use vellum_core::ports::sync::*;
use vellum_types::{Actor, DeviceId, sync::RoomId};

pub fn update_log(log: &dyn UpdateLog, room: &RoomId) {
    assert_eq!(log.load(room).unwrap(), None);
    log.create(room, "snapshot-one").unwrap();
    assert_eq!(log.create(room, "another"), Err(Error::Conflict));
    assert_eq!(log.append(room, 2, 1, b"wrong"), Err(Error::Conflict));
    assert_eq!(log.append(room, 1, 2, b"wrong-epoch"), Err(Error::Conflict));
    assert_eq!(log.append(room, 1, 1, b"first"), Ok(2));
    assert_eq!(log.append(room, 1, 1, b"loser"), Err(Error::Conflict));
    assert_eq!(log.append(room, 2, 1, b"second"), Ok(3));
    let before = log.load(room).unwrap().unwrap();
    assert_eq!(
        before
            .updates
            .iter()
            .map(|u| u.bytes.as_slice())
            .collect::<Vec<_>>(),
        [b"first".as_slice(), b"second".as_slice()]
    );
    assert_eq!(
        log.compact(room, 2, 1, "snapshot-two"),
        Err(Error::Conflict)
    );
    assert_eq!(log.load(room).unwrap().unwrap(), before);
    assert_eq!(log.compact(room, 3, 1, "snapshot-two"), Ok(2));
    let compacted = log.load(room).unwrap().unwrap();
    assert_eq!(
        (
            compacted.epoch,
            compacted.revision,
            compacted.snapshot.as_str()
        ),
        (2, 4, "snapshot-two")
    );
    assert!(compacted.updates.is_empty());
    assert_eq!(log.append(room, 4, 1, b"stale"), Err(Error::Conflict));
    assert_eq!(log.append(room, 4, 2, b"new"), Ok(5));
}
pub fn snapshots(store: &dyn SnapshotStore) {
    let a = store.put(b"TEST DATA: canonical snapshot one").unwrap();
    let b = store.put(b"TEST DATA: canonical snapshot two").unwrap();
    assert_ne!(a, b);
    assert_eq!(store.get(&a).unwrap(), b"TEST DATA: canonical snapshot one");
    store.put(b"TEST DATA: canonical snapshot one").unwrap();
    assert_eq!(store.get(&b).unwrap(), b"TEST DATA: canonical snapshot two");
    assert!(store.get("../missing").is_err());
}
pub fn presence(store: &dyn Presence, room: &RoomId) {
    let actor = Actor::Person {
        device: DeviceId::new("00000000000000000000000001").unwrap(),
    };
    let entry = PresenceEntry {
        connection: "test-connection".into(),
        actor,
        expires_ms: 2000,
    };
    store.touch(room, &entry, 1000).unwrap();
    store.touch(room, &entry, 1000).unwrap();
    assert_eq!(store.list(room, 1000).unwrap(), vec![entry.clone()]);
    let changed = PresenceEntry {
        actor: crate::samples::bob(),
        expires_ms: 2000,
        ..entry.clone()
    };
    store.touch(room, &changed, 1000).unwrap();
    assert_eq!(store.list(room, 1000).unwrap(), vec![changed]);
    assert!(
        store
            .list(&RoomId::new("other-room").unwrap(), 1000)
            .unwrap()
            .is_empty()
    );
    assert!(store.list(room, 2000).unwrap().is_empty());
    store.touch(room, &entry, 1000).unwrap();
    store.remove(room, &entry.connection).unwrap();
    store.remove(room, &entry.connection).unwrap();
    assert!(store.list(room, 1000).unwrap().is_empty());
}

/// Call with a prepared grant expiring at `expires_ms`. Wrong keys/rooms and expired grants fail closed.
pub fn access(
    store: &dyn AccessControl,
    key: &vellum_core::ports::SecretString,
    room: &RoomId,
    grant: &vellum_types::sync::Grant,
    expires_ms: i64,
) {
    assert_eq!(
        store.authorize(key, room, expires_ms - 1),
        Ok(grant.clone())
    );
    assert_eq!(
        store.authorize(key, room, expires_ms),
        Err(Error::Forbidden)
    );
    assert_eq!(
        store.authorize(
            &vellum_core::ports::SecretString::new("TEST DATA: unknown capability"),
            room,
            0
        ),
        Err(Error::Forbidden)
    );
    assert_eq!(
        store.authorize(key, &RoomId::new("ungranted-room").unwrap(), 0),
        Err(Error::Forbidden)
    );
}
