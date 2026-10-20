//! TEST DATA.
use vellum_contracts::suites::sync;
use vellum_sync_memory::*;
use vellum_types::sync::RoomId;
#[test]
fn memory_ports() {
    let room = RoomId::new("contract").unwrap();
    sync::update_log(&MemoryLog::default(), &room);
    sync::snapshots(&MemorySnapshots::default());
    sync::presence(&MemoryPresence::default(), &room);
}

#[test]
fn access_contract() {
    let room = RoomId::new("test-access").unwrap();
    let access = MemoryAccess::default();
    let grant = vellum_types::sync::Grant {
        actor: vellum_contracts::samples::alice(),
        role: vellum_types::sync::Role::Editor,
    };
    access.set_expiring("test-key", &room, grant.clone(), 2000);
    sync::access(
        &access,
        &vellum_core::ports::SecretString::new("test-key"),
        &room,
        &grant,
        2000,
    );
}
