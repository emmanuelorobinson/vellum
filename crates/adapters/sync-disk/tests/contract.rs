//! TEST DATA. Local private temporary directory only.
use vellum_sync_disk::DiskSnapshots;
#[test]
fn immutable_snapshots_and_reopen() {
    let root = std::env::temp_dir().join(format!("vellum-sync-disk-{}", std::process::id()));
    let store = DiskSnapshots::open(&root).unwrap();
    vellum_contracts::suites::sync::snapshots(&store);
    use vellum_core::ports::sync::SnapshotStore;
    let key = store.put(b"TEST DATA: reopen").unwrap();
    assert_eq!(
        DiskSnapshots::open(&root).unwrap().get(&key).unwrap(),
        b"TEST DATA: reopen"
    );
    assert!(store.get("../secrets").is_err());
    let bytes = b"TEST DATA: retry after interrupted staging";
    let staged_key = store.put(bytes).unwrap();
    std::fs::remove_file(root.join(format!("{staged_key}.vellum"))).unwrap();
    std::fs::write(
        root.join(format!("{staged_key}.pending")),
        b"TEST DATA: partial",
    )
    .unwrap();
    assert_eq!(store.put(bytes).unwrap(), staged_key);

    #[cfg(unix)]
    {
        let other = store.put(b"TEST DATA: no-follow").unwrap();
        std::fs::remove_file(root.join(format!("{other}.vellum"))).unwrap();
        std::os::unix::fs::symlink("/etc/hosts", root.join(format!("{other}.vellum"))).unwrap();
        assert!(store.get(&other).is_err());
    }
    std::fs::remove_dir_all(root).unwrap();
}
