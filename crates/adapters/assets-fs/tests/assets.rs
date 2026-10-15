//! `FsAssets`: the `AssetStore` contract, the layout on disk, damage, crashes and collection (guide Prompt 24).

use std::collections::BTreeSet;
use std::sync::Arc;

use vellum_assets_fs::{FsAssets, StdFs, assets_dir_for};
use vellum_contracts::fakes::MemFs;
use vellum_contracts::suites;
use vellum_core::files::{SaveKind, collect_on_save};
use vellum_core::ports::{AssetCollect, AssetError, AssetHash, AssetStore, FileSystem};
use vellum_types::{AssetId, Document, FontFace, FontStyle};

fn memory() -> (Arc<MemFs>, FsAssets) {
    let fs = Arc::new(MemFs::new());
    let assets = FsAssets::new(fs.clone(), "app.assets").expect("a valid folder");
    (fs, assets)
}

/// A folder under the system temp dir that is removed afterwards. No dependency for this: tests are the only
/// place a real disk is touched.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("vellum-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("make the folder");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn it_passes_the_asset_contracts_in_memory() {
    let (_, assets) = memory();
    suites::assets::run(&assets);
    suites::assets::collect(&assets);
}

#[test]
fn it_passes_the_asset_contracts_on_a_real_disk() {
    let dir = TempDir::new("assets-contract");
    let assets = FsAssets::new(Arc::new(StdFs::new(&dir.0)), "app.assets").unwrap();
    suites::assets::run(&assets);
    suites::assets::collect(&assets);
}

#[test]
fn the_real_disk_passes_the_file_system_contract() {
    let dir = TempDir::new("fs-contract");
    suites::files::run(&StdFs::new(&dir.0));
}

#[test]
fn the_hash_is_sha256_and_the_file_is_named_by_it() {
    let (fs, assets) = memory();
    let hash = assets.put(b"abc", "text/plain").unwrap();
    assert_eq!(
        hash.as_str(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "the SHA-256 of \"abc\""
    );
    assert_eq!(
        fs.read(&format!("app.assets/{hash}")).unwrap(),
        b"abc",
        "content-hash files live in <name>.assets/"
    );
    assert_eq!(
        fs.list("app.assets").unwrap(),
        vec![hash.to_string()],
        "nothing but the asset is in the folder"
    );
}

#[test]
fn storing_the_same_bytes_again_writes_nothing() {
    let (fs, assets) = memory();
    let hash = assets.put(b"same", "image/png").unwrap();
    let before = fs.mutations();
    assert_eq!(assets.put(b"same", "image/webp").unwrap(), hash);
    assert_eq!(fs.mutations(), before, "a repeat put touched the disk");
}

#[test]
fn a_damaged_file_is_reported_and_a_put_repairs_it() {
    let (fs, assets) = memory();
    let hash = assets.put(b"the whole image", "image/png").unwrap();
    fs.write(&format!("app.assets/{hash}"), b"the whole")
        .unwrap();
    match assets.get(&hash) {
        Err(AssetError::Unavailable(why)) => {
            assert!(why.contains("damaged"), "{why}");
        }
        other => panic!("a truncated asset was returned as if it were fine: {other:?}"),
    }
    assert_eq!(assets.put(b"the whole image", "image/png").unwrap(), hash);
    assert_eq!(assets.get(&hash).unwrap(), b"the whole image");
}

#[test]
fn a_crash_while_storing_never_leaves_a_half_asset() {
    let bytes = vec![7u8; 5000];
    let probe_fs = Arc::new(MemFs::new());
    FsAssets::new(probe_fs.clone(), "app.assets")
        .unwrap()
        .put(&bytes, "image/png")
        .unwrap();
    let steps = probe_fs.mutations();
    for crash_at in 0..=steps {
        let fs = Arc::new(MemFs::new());
        let assets = FsAssets::new(fs.clone(), "app.assets").unwrap();
        fs.crash_after(crash_at);
        let _ = assets.put(&bytes, "image/png");
        let disk = Arc::new(fs.after_crash());
        let restarted = FsAssets::new(disk, "app.assets").unwrap();
        let listed = restarted.list().unwrap();
        for hash in &listed {
            assert_eq!(
                restarted.get(hash).unwrap(),
                bytes,
                "crash after {crash_at} of {steps} left a damaged asset"
            );
        }
        assert!(listed.len() <= 1, "{listed:?}");
    }
}

#[test]
fn list_ignores_temporary_and_foreign_files() {
    let (fs, assets) = memory();
    let hash = assets.put(b"one", "image/png").unwrap();
    fs.seed("app.assets/.half.vellum-tmp", b"x");
    fs.seed("app.assets/.DS_Store", b"x");
    fs.seed("app.assets/notes.txt", b"x");
    fs.seed("app.assets/ABCDEF", b"x");
    assert_eq!(assets.list().unwrap(), vec![hash]);
}

#[test]
fn an_empty_or_missing_folder_lists_nothing() {
    let (_, assets) = memory();
    assert_eq!(assets.list().unwrap(), Vec::<AssetHash>::new());
}

#[test]
fn the_folder_is_named_after_the_document() {
    assert_eq!(assets_dir_for("designs/app.vellum"), "designs/app.assets");
    assert_eq!(assets_dir_for("app.vellum"), "app.assets");
    assert_eq!(
        assets_dir_for("designs/brand.lib.vellum"),
        "designs/brand.lib.assets"
    );
    assert_eq!(assets_dir_for("designs/app.vellum/"), "designs/app.assets");
    assert_eq!(assets_dir_for("README"), "README.assets");
}

#[test]
fn a_folder_outside_the_root_is_refused() {
    let fs = Arc::new(MemFs::new());
    for bad in ["../elsewhere", "/abs", "a//b", ""] {
        assert!(
            FsAssets::new(fs.clone(), bad).is_err(),
            "{bad:?} was accepted as an assets folder"
        );
    }
}

#[test]
fn only_an_explicit_save_removes_unused_assets_from_disk() {
    let (_, assets) = memory();
    let used = assets.put(b"a font", "font/ttf").unwrap();
    let unused = assets.put(b"an image nobody uses", "image/png").unwrap();
    let mut doc = Document::new("TEST DATA: gc");
    doc.fonts.insert(
        AssetId::new(used.as_str()).unwrap(),
        FontFace {
            family: "TEST DATA Sans".into(),
            weight: 400,
            style: FontStyle::Normal,
        },
    );
    let keep = BTreeSet::new();

    let report = collect_on_save(SaveKind::Autosave, &assets, &doc, &keep).unwrap();
    assert!(report.removed.is_empty());
    assert_eq!(assets.list().unwrap().len(), 2);

    let report = collect_on_save(SaveKind::Explicit, &assets, &doc, &keep).unwrap();
    assert_eq!(report.removed, vec![unused]);
    assert_eq!(assets.list().unwrap(), vec![used]);
}
