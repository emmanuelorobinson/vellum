//! TEST DATA. Every sync port has a deterministic fake and a suite that rejects a broken implementation.
use vellum_contracts::{fakes::sync::*, suites::sync};
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
#[test]
fn deterministic_fakes_pass_the_sync_suites() {
    let room = RoomId::new("fake-room").unwrap();
    sync::update_log(&FakeUpdateLog::default(), &room);
    sync::snapshots(&FakeSnapshotStore::default());
    sync::presence(&FakePresence::default(), &room);
    let access = FakeAccess::default();
    let grant = Grant {
        actor: vellum_contracts::samples::alice(),
        role: Role::Editor,
    };
    access.grant("test-capability", &room, grant.clone(), 2000);
    sync::access(
        &access,
        &SecretString::new("test-capability"),
        &room,
        &grant,
        2000,
    );
}
struct IgnoresRevision(FakeUpdateLog);
impl UpdateLog for IgnoresRevision {
    fn load(&self, r: &RoomId) -> Result<Option<RoomLog>, Error> {
        self.0.load(r)
    }
    fn create(&self, r: &RoomId, s: &str) -> Result<(), Error> {
        self.0.create(r, s)
    }
    fn append(&self, r: &RoomId, _: u64, e: u64, b: &[u8]) -> Result<u64, Error> {
        self.0.append(r, self.0.load(r)?.unwrap().revision, e, b)
    }
    fn compact(&self, r: &RoomId, n: u64, e: u64, s: &str) -> Result<u64, Error> {
        self.0.compact(r, n, e, s)
    }
}
struct Overwrites;
impl SnapshotStore for Overwrites {
    fn put(&self, _: &[u8]) -> Result<String, Error> {
        Ok("same-key".into())
    }
    fn get(&self, _: &str) -> Result<Vec<u8>, Error> {
        Ok(vec![])
    }
}
struct NoPresence;
impl Presence for NoPresence {
    fn touch(&self, _: &RoomId, _: &PresenceEntry, _: i64) -> Result<(), Error> {
        Ok(())
    }
    fn list(&self, _: &RoomId, _: i64) -> Result<Vec<PresenceEntry>, Error> {
        Ok(vec![])
    }
    fn remove(&self, _: &RoomId, _: &str) -> Result<(), Error> {
        Ok(())
    }
}
struct NeverExpires(Grant);
impl AccessControl for NeverExpires {
    fn authorize(&self, _: &SecretString, _: &RoomId, _: i64) -> Result<Grant, Error> {
        Ok(self.0.clone())
    }
}
#[test]
fn broken_sync_ports_fail_the_contracts() {
    let room = RoomId::new("broken-room").unwrap();
    assert!(
        std::panic::catch_unwind(|| sync::update_log(
            &IgnoresRevision(FakeUpdateLog::default()),
            &room
        ))
        .is_err()
    );
    assert!(std::panic::catch_unwind(|| sync::snapshots(&Overwrites)).is_err());
    assert!(std::panic::catch_unwind(|| sync::presence(&NoPresence, &room)).is_err());
    let grant = Grant {
        actor: vellum_contracts::samples::alice(),
        role: Role::Editor,
    };
    assert!(
        std::panic::catch_unwind(|| sync::access(
            &NeverExpires(grant.clone()),
            &SecretString::new("test-capability"),
            &room,
            &grant,
            2000
        ))
        .is_err()
    );
}
