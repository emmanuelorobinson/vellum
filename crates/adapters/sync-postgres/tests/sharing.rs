//! TEST DATA. @cloud: disposable Postgres only; changes affect existing capabilities.
use vellum_contracts::samples::*;
use vellum_core::ports::{SecretString, sync::*};
use vellum_types::sync::*;
#[test]
#[ignore = "@cloud: VELLUM_TEST_DATABASE_URL required"]
fn owner_management_matches_self_host_policy() {
    let db = vellum_sync_postgres::PostgresLog::connect(
        &std::env::var("VELLUM_TEST_DATABASE_URL").unwrap(),
    )
    .unwrap();
    let room = RoomId::new(format!("sharing-{}", std::process::id())).unwrap();
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
        None,
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
        None,
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
    db.manage(
        &owner,
        &room,
        SharingCommand::SetRole {
            actor: bob(),
            role: Role::Commenter,
        },
        0,
    )
    .unwrap();
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
        db.manage(&owner, &room, SharingCommand::Remove { actor: alice() }, 0)
            .is_err()
    );
}
