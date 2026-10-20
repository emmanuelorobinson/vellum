//! TEST DATA. Private SQLite fixture; role changes must affect existing capabilities immediately.
use vellum_contracts::samples::*;
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
#[test]
fn only_owner_changes_live_access_and_roles() {
    let db = vellum_sync_sqlite::SqliteLog::open(":memory:").unwrap();
    let room = RoomId::new("sharing-test").unwrap();
    let owner = SecretString::new("test-owner");
    let reader = SecretString::new("test-reader");
    db.grant(
        &owner,
        &room,
        &Grant {
            actor: alice(),
            role: Role::Editor,
        },
        10000,
    )
    .unwrap();
    db.grant(
        &reader,
        &room,
        &Grant {
            actor: bob(),
            role: Role::Editor,
        },
        10000,
    )
    .unwrap();
    db.set_owner(&room, &alice()).unwrap();
    assert_eq!(
        db.manage(
            &reader,
            &room,
            SharingCommand::SetRole {
                actor: alice(),
                role: Role::Viewer
            },
            0
        ),
        Err(Error::Forbidden)
    );
    let s = db
        .manage(
            &owner,
            &room,
            SharingCommand::SetRole {
                actor: bob(),
                role: Role::Commenter,
            },
            0,
        )
        .unwrap();
    assert!(s.owner);
    assert_eq!(
        db.authorize(&reader, &room, 0).unwrap().role,
        Role::Commenter
    );
    db.manage(&owner, &room, SharingCommand::SetLive { enabled: false }, 0)
        .unwrap();
    assert_eq!(db.authorize(&reader, &room, 0), Err(Error::Forbidden));
    db.manage(&owner, &room, SharingCommand::SetLive { enabled: true }, 0)
        .unwrap();
    db.manage(&owner, &room, SharingCommand::Remove { actor: bob() }, 0)
        .unwrap();
    assert_eq!(db.authorize(&reader, &room, 0), Err(Error::Forbidden));
    assert!(
        db.manage(
            &owner,
            &room,
            SharingCommand::SetRole {
                actor: alice(),
                role: Role::Viewer
            },
            0
        )
        .is_err()
    );
}
