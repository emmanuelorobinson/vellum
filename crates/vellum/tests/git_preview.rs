//! TEST DATA. Prompt 60: real Git object reads and low-level merges, native CPU PNGs. Needs Git on PATH;
//! no daemon, network, browser or GPU. Run: cargo test -p vellum --test git_preview.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Repo(PathBuf);
impl Repo {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "vellum git 'fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        let r = Self(p);
        r.git(&["init", "-q", "-b", "main"]);
        r.git(&["config", "user.name", "TEST DATA"]);
        r.git(&["config", "user.email", "test@example.invalid"]);
        r.git(&["config", "commit.gpgsign", "false"]);
        r
    }
    fn git_out(&self, args: &[&str]) -> Output {
        Command::new("git")
            .current_dir(&self.0)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .args(args)
            .output()
            .unwrap()
    }
    fn git(&self, args: &[&str]) -> String {
        let o = self.git_out(args);
        assert!(o.status.success(), "{args:?}: {o:?}");
        String::from_utf8(o.stdout).unwrap().trim().into()
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_vellum"))
            .current_dir(&self.0)
            .env("NO_COLOR", "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .args(args)
            .output()
            .unwrap()
    }
    fn write(&self, path: &str, v: &Value) {
        self.bytes(path, &serde_json::to_vec_pretty(v).unwrap());
    }
    fn bytes(&self, path: &str, bytes: &[u8]) {
        std::fs::create_dir_all(self.0.join(path).parent().unwrap()).unwrap();
        std::fs::write(self.0.join(path), bytes).unwrap();
    }
    fn commit(&self) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-qm", "TEST DATA"]);
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn doc() -> Value {
    json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum 0.0.0"},"pages":[{"id":"pageHome01","name":"Home","children":[
    {"type":"frame","id":"frameCard1","name":"Card","width":16,"height":12,"fills":[{"type":"solid","color":"#FF0000"}],"children":[{"type":"rect","id":"rectChild1","name":"Child","width":4,"height":4}]},
    {"type":"frame","id":"frameSame1","name":"Same","width":8,"height":8,"fills":[{"type":"solid","color":"#FFFFFF"}]}]}]})
}
fn value(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|_| panic!("{o:?}"))
}
fn ok(o: Output) -> Value {
    assert!(o.status.success(), "{o:?}");
    value(&o)
}
fn preview(r: &Repo, range: &str, dest: &str) -> Value {
    ok(r.cli(&["diff", range, "--png", "-o", dest, "--json"]))
}
fn png(r: &Repo, dest: &str, path: &Value) -> Vec<u8> {
    let bytes = std::fs::read(r.0.join(dest).join(path.as_str().unwrap())).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    bytes
}
#[test]
fn setup_is_local_idempotent_and_installs_a_working_clean_merge() {
    let r = Repo::new();
    r.bytes(".gitattributes", b"*.txt text\n");
    r.write("design/app.vellum", &doc());
    r.commit();
    let setup = r.cli(&["git", "setup"]);
    assert!(setup.status.success(), "{setup:?}");
    assert_eq!(
        String::from_utf8(setup.stdout).unwrap(),
        "✓ Installed the Vellum merge driver for this repo\n"
    );
    let before = std::fs::read(r.0.join(".gitattributes")).unwrap();
    assert_eq!(ok(r.cli(&["git", "setup", "--json"]))["changed"], false);
    assert_eq!(before, std::fs::read(r.0.join(".gitattributes")).unwrap());
    assert!(
        String::from_utf8(before)
            .unwrap()
            .starts_with("*.txt text\n")
    );
    assert_eq!(
        r.git(&["check-attr", "merge", "--", "design/app.vellum"]),
        "design/app.vellum: merge: vellum"
    );
    assert_eq!(
        r.git(&["config", "--local", "merge.vellum.recursive"]),
        "binary"
    );
    // Git runs the configured command through sh. Exercise an executable path with spaces and quotes,
    // in addition to the repository path, without adding the binary to Git.
    r.bytes(".gitignore", b"tools/\n");
    std::fs::create_dir_all(r.0.join("tools")).unwrap();
    let relocated = r.0.join("tools/vellum 'quoted.exe");
    std::fs::copy(env!("CARGO_BIN_EXE_vellum"), &relocated).unwrap();
    let setup = Command::new(&relocated)
        .current_dir(&r.0)
        .args(["git", "setup", "--json"])
        .output()
        .unwrap();
    assert_eq!(ok(setup)["changed"], true);
    assert_eq!(
        r.git(&["check-attr", "merge", "--", "nested/app.vellum/index.json"]),
        "nested/app.vellum/index.json: merge: vellum"
    );
    assert_eq!(
        r.git(&[
            "check-attr",
            "merge",
            "--",
            "nested/app.vellum/pages/pageHome01.json"
        ]),
        "nested/app.vellum/pages/pageHome01.json: merge: vellum"
    );
    r.commit();
    r.git(&["checkout", "-qb", "other"]);
    let mut theirs = doc();
    theirs["pages"][0]["children"][0]["height"] = json!(20);
    r.write("design/app.vellum", &theirs);
    r.commit();
    r.git(&["checkout", "-q", "main"]);
    let mut ours = doc();
    ours["pages"][0]["children"][0]["width"] = json!(24);
    r.write("design/app.vellum", &ours);
    r.commit();
    r.git(&["merge", "--no-edit", "other"]);
    let merged: Value =
        serde_json::from_slice(&std::fs::read(r.0.join("design/app.vellum")).unwrap()).unwrap();
    assert_eq!(merged["pages"][0]["children"][0]["width"], 24);
    assert_eq!(merged["pages"][0]["children"][0]["height"], 20);
    assert!(r.git(&["ls-files", "-u"]).is_empty());
}
#[test]
fn conflict_merge_stays_unresolved_and_preserves_both_versions_as_valid_json() {
    let r = Repo::new();
    r.write("app.vellum", &doc());
    r.commit();
    ok(r.cli(&["git", "setup", "--json"]));
    r.commit();
    r.git(&["checkout", "-qb", "other"]);
    let mut theirs = doc();
    theirs["pages"][0]["children"][0]["width"] = json!(32);
    r.write("app.vellum", &theirs);
    r.commit();
    r.git(&["checkout", "-q", "main"]);
    let mut ours = doc();
    ours["pages"][0]["children"][0]["width"] = json!(24);
    r.write("app.vellum", &ours);
    r.commit();
    let out = r.git_out(&["merge", "--no-edit", "other"]);
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let bytes = std::fs::read(r.0.join("app.vellum")).unwrap();
    let merged = vellum_codec_json::JsonCodec::new().decode(&bytes).unwrap();
    assert!(vellum_core::ops::check_document(&merged).is_empty());
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let n = &v["pages"][0]["children"][0];
    assert_eq!(n["type"], "conflict");
    assert_eq!(n["ours"]["width"], 24);
    assert_eq!(n["theirs"]["width"], 32);
    assert!(!r.git(&["ls-files", "-u"]).is_empty());
}
#[test]
fn invalid_merge_input_does_not_overwrite_ours_and_json_is_honest() {
    let r = Repo::new();
    r.write("ours", &doc());
    r.write("base", &doc());
    r.bytes("theirs", b"<<<<<<< bad");
    let before = std::fs::read(r.0.join("ours")).unwrap();
    let o = r.cli(&["merge-driver", "base", "ours", "theirs", "--json"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(value(&o)["ok"], false);
    assert_eq!(before, std::fs::read(r.0.join("ours")).unwrap());
}
#[test]
fn diff_json_text_and_png_goldens_are_deterministic_and_checkout_is_untouched() {
    let r = Repo::new();
    r.write("design/app.vellum", &doc());
    r.commit();
    let mut changed = doc();
    changed["pages"][0]["children"][0]["fills"][0]["color"] = json!("#0000FF");
    r.write("design/app.vellum", &changed);
    r.commit();
    r.bytes("design/app.vellum", b"unsaved checkout bytes");
    let v = preview(&r, "HEAD~1..HEAD", "preview");
    assert_eq!(v["frames"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["frames"][0]["id"], "frameCard1");
    assert_eq!(v["frames"][0]["change"], "modified");
    let before = png(&r, "preview", &v["frames"][0]["before"]);
    let after = png(&r, "preview", &v["frames"][0]["after"]);
    assert_ne!(before, after);
    use vellum_testkit::pixel_diff::Rgba8Image;
    for (side, rgba) in [("before", [255, 0, 0, 255]), ("after", [0, 0, 255, 255])] {
        let image = Rgba8Image::read_png(
            &r.0.join("preview")
                .join(v["frames"][0][side].as_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            image,
            Rgba8Image::new(16, 12, rgba.repeat(16 * 12)).unwrap()
        );
    }

    let again = preview(&r, "HEAD~1..HEAD", "again");
    assert_eq!(v["frames"], again["frames"]);
    assert_eq!(after, png(&r, "again", &again["frames"][0]["after"]));
    assert_eq!(
        std::fs::read(r.0.join("design/app.vellum")).unwrap(),
        b"unsaved checkout bytes"
    );
    assert_eq!(r.git(&["branch", "--show-current"]), "main");
    let disk: Value =
        serde_json::from_slice(&std::fs::read(r.0.join("preview/manifest.json")).unwrap()).unwrap();
    assert_eq!(disk["frames"], v["frames"]);
    let o = r.cli(&["diff", "HEAD~1..HEAD", "design/app.vellum", "-o", "text"]);
    assert!(o.status.success(), "{o:?}");
    let text = String::from_utf8(o.stdout).unwrap();
    assert_eq!(text, include_str!("goldens/git/diff.txt"));
    assert_eq!(
        v["frames"][0],
        json!({"file":"design/app.vellum","id":"frameCard1","name":"Card","change":"modified","before":"0001-before.png","after":"0001-after.png"})
    );
}
#[test]
fn added_removed_frames_and_documents_have_only_the_existing_side() {
    let r = Repo::new();
    r.write("old.vellum", &doc());
    r.write("same.vellum", &doc());
    r.commit();
    r.git(&["rm", "-q", "old.vellum"]);
    r.write("new.vellum", &doc());
    let mut changed = doc();
    changed["pages"][0]["children"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    r.write("same.vellum", &changed);
    r.commit();
    let v = preview(&r, "HEAD~1..HEAD", "out");
    assert_eq!(v["frames"].as_array().unwrap().len(), 5, "{v}");
    for f in v["frames"].as_array().unwrap() {
        match f["change"].as_str().unwrap() {
            "added" => {
                assert!(f["before"].is_null());
                png(&r, "out", &f["after"]);
            }
            "removed" => {
                assert!(f["after"].is_null());
                png(&r, "out", &f["before"]);
            }
            _ => panic!("{f}"),
        }
    }
}
#[test]
fn dependency_change_in_library_renders_its_consumers_from_each_commit() {
    let r = Repo::new();
    let mut lib = doc();
    lib["meta"]["kind"] = json!("library");
    lib["components"] =
        json!({"compCard01":{"name":"Card","root":lib["pages"][0]["children"][0].clone()}});
    lib["pages"] = json!([]);
    let consumer = json!({"meta":doc()["meta"],"imports":{"ui":"../lib.lib.vellum"},"pages":[{"id":"pageHome01","name":"Home","children":[{"type":"frame","id":"frameCard1","name":"Card","width":16,"height":12,"children":[{"type":"instance","id":"instCard01","name":"Instance","componentId":"compCard01","library":"ui","width":16,"height":12}]}]}]});
    r.write("lib.lib.vellum", &lib);
    r.write("design/app.vellum", &consumer);
    r.commit();
    lib["components"]["compCard01"]["root"]["fills"][0]["color"] = json!("#0000FF");
    r.write("lib.lib.vellum", &lib);
    r.commit();
    let v = preview(&r, "HEAD~1..HEAD", "out");
    let f = v["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["file"] == "design/app.vellum")
        .expect("consumer preview");
    assert_ne!(png(&r, "out", &f["before"]), png(&r, "out", &f["after"]));
}
#[test]
fn directory_documents_and_empty_diffs_are_supported() {
    let r = Repo::new();
    let fs = vellum_assets_fs::StdFs::new(&r.0);
    let codec = vellum_codec_json::JsonCodec::new();
    let dir = vellum_codec_json_dir::DirCodec::new(&codec);
    use vellum_core::ports::DocumentFiles;
    let mut d: vellum_types::Document = serde_json::from_value(doc()).unwrap();
    dir.write(&fs, "app.vellum", &d).unwrap();
    r.commit();
    let empty = preview(&r, "HEAD..HEAD", "empty");
    assert_eq!(empty["frames"], json!([]));
    if let vellum_types::Node::Frame(f) = &mut d.pages[0].children[0] {
        f.name = "Renamed".into();
    }
    dir.write(&fs, "app.vellum", &d).unwrap();
    r.commit();
    let v = preview(&r, "HEAD~1..HEAD", "out");
    assert_eq!(v["frames"].as_array().unwrap().len(), 1);
    assert_eq!(v["frames"][0]["file"], "app.vellum");
}
#[test]
fn bad_revisions_paths_newer_documents_and_existing_output_fail_without_damage() {
    let r = Repo::new();
    r.write("app.vellum", &doc());
    r.commit();
    for args in [
        vec!["diff", "--json", "missing..HEAD", "-o", "out"],
        vec!["diff", "--json", "HEAD...HEAD", "-o", "out"],
        vec![
            "diff",
            "--json",
            "HEAD..HEAD",
            "../escape.vellum",
            "-o",
            "out",
        ],
        vec![
            "diff",
            "--json",
            "HEAD..HEAD",
            "missing.vellum",
            "-o",
            "out",
        ],
    ] {
        let o = r.cli(&args);
        assert!(!o.status.success(), "{args:?}: {o:?}");
        assert_eq!(value(&o)["ok"], false);
        assert!(!r.0.join("out").exists());
    }
    r.bytes("out/keep", b"keep");
    let o = r.cli(&["diff", "HEAD..HEAD", "-o", "out", "--json"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(std::fs::read(r.0.join("out/keep")).unwrap(), b"keep");
    let mut newer = doc();
    newer["meta"]["formatVersion"] = json!(999);
    r.write("app.vellum", &newer);
    r.commit();
    let o = r.cli(&["diff", "HEAD~1..HEAD", "-o", "bad", "--json"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(value(&o)["ok"], false);
    assert!(!r.0.join("bad/manifest.json").exists());
}
#[cfg(unix)]
#[test]
fn setup_refuses_symlink_attributes_and_diff_refuses_symlink_documents() {
    let r = Repo::new();
    r.bytes("keep", b"keep");
    std::os::unix::fs::symlink("keep", r.0.join(".gitattributes")).unwrap();
    let o = r.cli(&["git", "setup", "--json"]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(std::fs::read(r.0.join("keep")).unwrap(), b"keep");
    std::fs::remove_file(r.0.join(".gitattributes")).unwrap();
    std::os::unix::fs::symlink("keep", r.0.join("bad.vellum")).unwrap();
    r.commit();
    let o = r.cli(&["diff", "HEAD..HEAD", "-o", "out", "--json"]);
    assert_eq!(o.status.code(), Some(1));
}

#[test]
fn an_unrelated_broken_document_does_not_block_a_preview() {
    let r = Repo::new();
    r.write("app.vellum", &doc());
    r.bytes("unrelated.vellum", b"TEST DATA: intentionally unreadable");
    r.commit();
    let mut d = doc();
    d["pages"][0]["children"][0]["name"] = json!("Changed");
    r.write("app.vellum", &d);
    r.commit();
    let v = preview(&r, "HEAD~1..HEAD", "out");
    assert_eq!(v["frames"].as_array().unwrap().len(), 1);
}

#[test]
fn hidden_frames_are_reported_without_fabricating_visible_images() {
    let r = Repo::new();
    let mut d = doc();
    d["pages"][0]["children"][1]["visible"] = json!(false);
    r.write("app.vellum", &d);
    r.commit();
    d["pages"][0]["children"][0]["name"] = json!("Changed");
    d["pages"][0]["children"][1]["name"] = json!("Hidden rename");
    r.write("app.vellum", &d);
    r.commit();
    let v = preview(&r, "HEAD~1..HEAD", "out");
    assert_eq!(v["frames"].as_array().unwrap().len(), 2);
    let hidden = &v["frames"][1];
    assert_eq!(hidden["id"], "frameSame1");
    assert!(hidden["before"].is_null());
    assert!(hidden["after"].is_null());
    assert_eq!(hidden["beforeState"], "hidden");
    assert_eq!(hidden["afterState"], "hidden");
}

#[test]
fn png_assets_use_historical_bytes_even_when_the_worktree_copy_is_gone() {
    use vellum_testkit::pixel_diff::Rgba8Image;
    let r = Repo::new();
    let mut d = doc();
    for (side, rgba) in [("before", [255, 0, 0, 255]), ("after", [0, 0, 255, 255])] {
        let file = r.0.join("asset.png");
        Rgba8Image::new(4, 4, rgba.repeat(16))
            .unwrap()
            .write_png(&file)
            .unwrap();
        let bytes = std::fs::read(&file).unwrap();
        std::fs::remove_file(file).unwrap();
        let hash = vellum_assets_fs::content_hash(&bytes);
        r.bytes(&format!("app.assets/{hash}"), &bytes);
        d["pages"][0]["children"][0]["children"] = json!([{"type":"image","id":"image00001","name":"TEST DATA image","asset":hash,"width":16,"height":12}]);
        r.write("app.vellum", &d);
        r.commit();
        // All asset changes in the checkout are irrelevant: only committed objects are used.
        if side == "before" {
            std::fs::remove_dir_all(r.0.join("app.assets")).unwrap();
        }
    }
    std::fs::remove_dir_all(r.0.join("app.assets")).unwrap();
    let v = preview(&r, "HEAD~1..HEAD", "out");
    assert_eq!(v["frames"].as_array().unwrap().len(), 1);
    for (side, rgba) in [("before", [255, 0, 0, 255]), ("after", [0, 0, 255, 255])] {
        let image =
            Rgba8Image::read_png(&r.0.join("out").join(v["frames"][0][side].as_str().unwrap()))
                .unwrap();
        assert_eq!(
            image,
            Rgba8Image::new(16, 12, rgba.repeat(16 * 12)).unwrap()
        );
    }
}

#[test]
fn moving_a_frame_between_groups_is_a_change_even_when_the_cropped_pixels_match() {
    let r = Repo::new();
    let mut d = doc();
    let frame = d["pages"][0]["children"][0].clone();
    d["pages"][0]["children"] = json!([
        {"type":"group","id":"groupFrom1","name":"From","width":32,"height":24,"children":[frame]},
        {"type":"group","id":"groupTo001","name":"To","width":32,"height":24,"children":[]}
    ]);
    r.write("app.vellum", &d);
    r.commit();
    let f = d["pages"][0]["children"][0]["children"]
        .as_array_mut()
        .unwrap()
        .remove(0);
    d["pages"][0]["children"][1]["children"] = json!([f]);
    r.write("app.vellum", &d);
    r.commit();
    let v = preview(&r, "HEAD~1..HEAD", "out");
    assert_eq!(v["frames"].as_array().unwrap().len(), 1, "{v}");
}

#[cfg(unix)]
#[test]
fn setup_never_follows_an_existing_temporary_file_symlink() {
    let r = Repo::new();
    r.bytes("keep", b"TEST DATA: preserve");
    std::os::unix::fs::symlink("keep", r.0.join("..gitattributes.vellum-tmp")).unwrap();
    let o = r.cli(&["git", "setup", "--json"]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert_eq!(
        std::fs::read(r.0.join("keep")).unwrap(),
        b"TEST DATA: preserve"
    );
}

#[test]
fn a_directory_with_remaining_pages_and_a_missing_index_is_not_a_removed_document() {
    use vellum_core::ports::DocumentFiles;
    let r = Repo::new();
    let fs = vellum_assets_fs::StdFs::new(&r.0);
    let codec = vellum_codec_json::JsonCodec::new();
    let dir = vellum_codec_json_dir::DirCodec::new(&codec);
    let d: vellum_types::Document = serde_json::from_value(doc()).unwrap();
    dir.write(&fs, "app.vellum", &d).unwrap();
    r.commit();
    r.git(&["rm", "-q", "app.vellum/index.json"]);
    r.commit();
    let o = r.cli(&["diff", "HEAD~1..HEAD", "-o", "out", "--json"]);
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert_eq!(value(&o)["ok"], false);
    assert!(!r.0.join("out/manifest.json").exists());
}

#[cfg(unix)]
#[test]
fn a_missing_promisor_blob_never_runs_a_fetch_or_remote_helper() {
    use std::os::unix::fs::PermissionsExt;
    let r = Repo::new();
    r.write("app.vellum", &doc());
    r.commit();
    let blob = r.git(&["rev-parse", "HEAD:app.vellum"]);
    std::fs::remove_file(r.0.join(format!(".git/objects/{}/{}", &blob[..2], &blob[2..]))).unwrap();
    r.git(&["config", "remote.origin.url", "vellumfixture::missing"]);
    r.git(&["config", "remote.origin.promisor", "true"]);
    r.git(&["config", "extensions.partialClone", "origin"]);
    r.bytes(
        "tools/git-remote-vellumfixture",
        b"#!/bin/sh\nprintf fetched > fetch-was-attempted\nexit 1\n",
    );
    std::fs::set_permissions(
        r.0.join("tools/git-remote-vellumfixture"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let path = format!(
        "{}:{}",
        r.0.join("tools").display(),
        std::env::var("PATH").unwrap()
    );
    let o = Command::new(env!("CARGO_BIN_EXE_vellum"))
        .current_dir(&r.0)
        .env("PATH", path)
        .args(["diff", "HEAD..HEAD", "app.vellum", "-o", "out", "--json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert!(
        !r.0.join("fetch-was-attempted").exists(),
        "A local preview tried to fetch missing Git objects"
    );
}
