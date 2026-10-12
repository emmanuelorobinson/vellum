//! `AssetStore`.

use vellum_core::ports::{AssetError, AssetHash, AssetStore};

/// Assets come back as stored, by content: the same bytes give the same hash and different bytes do not.
pub fn run(store: &dyn AssetStore) {
    let png = b"\x89PNG TEST DATA one".to_vec();
    let other = b"\x89PNG TEST DATA two".to_vec();

    let hash = store.put(&png, "image/png").expect("put");
    assert_eq!(
        store.get(&hash).expect("get"),
        png,
        "AssetStore: bytes changed on the way through"
    );
    assert_eq!(
        store.put(&png, "image/png").expect("put again"),
        hash,
        "AssetStore: the same bytes got two hashes"
    );
    assert_eq!(
        store
            .put(&png, "image/webp")
            .expect("put with another type"),
        hash,
        "AssetStore: the hash must depend on the bytes alone"
    );
    let other_hash = store.put(&other, "image/png").expect("put other");
    assert_ne!(hash, other_hash, "AssetStore: different bytes share a hash");
    assert_eq!(store.get(&other_hash).expect("get other"), other);
    assert_eq!(
        store.get(&hash).expect("get first again"),
        png,
        "AssetStore: a later put changed an earlier asset"
    );

    let nothing = store
        .put(&[], "application/octet-stream")
        .expect("put no bytes");
    assert_eq!(store.get(&nothing).expect("get no bytes"), Vec::<u8>::new());

    let big: Vec<u8> = (0..1_048_576u32).map(|n| (n % 251) as u8).collect();
    let big_hash = store
        .put(&big, "application/octet-stream")
        .expect("put a megabyte");
    assert!(
        store.get(&big_hash).expect("get a megabyte") == big,
        "AssetStore: a megabyte changed"
    );

    for bad in ["", "png", "image/", "/png", "image png"] {
        assert!(
            matches!(store.put(&png, bad), Err(AssetError::InvalidMime(_))),
            "AssetStore: {bad:?} was accepted as a media type"
        );
    }

    let missing = AssetHash::new("0".repeat(64)).expect("a valid hash");
    assert!(
        matches!(store.get(&missing), Err(AssetError::NotFound(h)) if h == missing),
        "AssetStore: an unknown hash must be NotFound"
    );
}
