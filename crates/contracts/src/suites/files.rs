//! `FileSystem`.

use vellum_core::ports::{FileSystem, FsError};

/// Whole files, folders, rename and listing behave as a disk does, and nothing escapes the root.
///
/// `fs` is empty or at least has no folder called `contract-suite`; the suite works inside that folder and
/// removes the files it makes (the port has no way to remove a folder).
pub fn run(fs: &dyn FileSystem) {
    let all_bytes: Vec<u8> = (0..=255u8).collect();

    assert!(
        matches!(fs.read("contract-suite/nope"), Err(FsError::NotFound(_))),
        "FileSystem: reading a missing file must be NotFound"
    );
    assert!(
        matches!(
            fs.write("contract-suite/early.bin", b"x"),
            Err(FsError::NotFound(_))
        ),
        "FileSystem: writing into a folder that is not there must be NotFound"
    );
    assert!(
        matches!(fs.list("contract-suite"), Err(FsError::NotFound(_))),
        "FileSystem: listing a missing folder must be NotFound"
    );
    assert!(!fs.exists("contract-suite").expect("exists"));

    fs.create_dir_all("contract-suite/a/b")
        .expect("make folders");
    fs.create_dir_all("contract-suite/a/b")
        .expect("make them again");
    assert!(fs.exists("contract-suite/a").expect("exists"));
    fs.sync_dir("contract-suite/a/b").expect("sync folder");

    fs.write("contract-suite/a/b/data.bin", &all_bytes)
        .expect("write");
    assert_eq!(
        fs.read("contract-suite/a/b/data.bin").expect("read"),
        all_bytes,
        "FileSystem: bytes changed on the way through"
    );
    fs.sync_file("contract-suite/a/b/data.bin").expect("sync");
    assert!(matches!(
        fs.sync_file("contract-suite/a/b/missing.bin"),
        Err(FsError::NotFound(_))
    ));

    fs.write("contract-suite/a/b/data.bin", b"short")
        .expect("overwrite shorter");
    assert_eq!(
        fs.read("contract-suite/a/b/data.bin").expect("read again"),
        b"short",
        "FileSystem: an overwrite kept the tail of the old file"
    );
    fs.write("contract-suite/a/b/empty.bin", b"")
        .expect("write nothing");
    assert_eq!(
        fs.read("contract-suite/a/b/empty.bin").expect("read empty"),
        Vec::<u8>::new()
    );

    fs.write("contract-suite/a/b/from.bin", b"from")
        .expect("write from");
    fs.rename("contract-suite/a/b/from.bin", "contract-suite/a/b/data.bin")
        .expect("rename over an existing file");
    assert_eq!(
        fs.read("contract-suite/a/b/data.bin")
            .expect("read renamed"),
        b"from",
        "FileSystem: rename must replace what was there"
    );
    assert!(
        matches!(
            fs.read("contract-suite/a/b/from.bin"),
            Err(FsError::NotFound(_))
        ),
        "FileSystem: the old name survived a rename"
    );
    assert!(matches!(
        fs.rename("contract-suite/a/b/from.bin", "contract-suite/a/b/x.bin"),
        Err(FsError::NotFound(_))
    ));

    fs.write("contract-suite/a/z.bin", b"z").expect("write z");
    assert_eq!(
        fs.list("contract-suite/a").expect("list"),
        vec!["b".to_owned(), "z.bin".to_owned()],
        "FileSystem: list gives sorted names, files and folders, not paths"
    );
    assert_eq!(
        fs.list("contract-suite/a/b").expect("list"),
        vec!["data.bin".to_owned(), "empty.bin".to_owned()]
    );
    assert!(
        fs.list("")
            .expect("list the root")
            .contains(&"contract-suite".to_owned())
    );

    fs.remove_file("contract-suite/a/z.bin").expect("remove");
    assert!(matches!(
        fs.remove_file("contract-suite/a/z.bin"),
        Err(FsError::NotFound(_))
    ));
    assert!(!fs.exists("contract-suite/a/z.bin").expect("exists"));

    for bad in ["../x", "/x", "a//b", "a\\b", "a/./b", "a/../b", "a\0b"] {
        assert!(
            matches!(fs.read(bad), Err(FsError::InvalidPath(_))),
            "FileSystem: read accepted {bad:?}"
        );
        assert!(
            matches!(fs.write(bad, b"x"), Err(FsError::InvalidPath(_))),
            "FileSystem: write accepted {bad:?}"
        );
        assert!(
            matches!(fs.list(bad), Err(FsError::InvalidPath(_))),
            "FileSystem: list accepted {bad:?}"
        );
        assert!(
            matches!(
                fs.rename("contract-suite/a/b/data.bin", bad),
                Err(FsError::InvalidPath(_))
            ),
            "FileSystem: rename accepted {bad:?}"
        );
    }
    assert!(
        matches!(fs.read(""), Err(FsError::InvalidPath(_))),
        "FileSystem: the root is a folder, not a file"
    );

    // Leave nothing behind for the next implementation.
    for file in [
        "contract-suite/a/b/data.bin",
        "contract-suite/a/b/empty.bin",
    ] {
        fs.remove_file(file).expect("clean up");
    }
}
