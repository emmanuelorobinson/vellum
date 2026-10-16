//! TEST DATA. The daemon's documents on the real disk with native file events, Loro and Git (guide Prompt 28):
//! `git checkout` and `git merge` reach open documents as ops, the daemon's own writes are ignored, atomic saves
//! by other programs are followed, and a headless target renders through the shared Skia renderer.
//!
//! Needs `git` on PATH (every CI image has it). No network, GPU or Chrome.

mod common;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{FakeEditor, decode, encode, hash};
use vellum::daemon::{Documents, EditorId, Env, External, Recovery, Role};
use vellum_assets_fs::StdFs;
use vellum_codec_json::JsonCodec;
use vellum_codec_json_dir::DirCodec;
use vellum_contracts::fakes::{ManualClock, SequentialIds};
use vellum_contracts::samples::{self, alice, insert_at, node, rename, tx};
use vellum_core::doc_api::{RenderRead, Rendered, SceneConfig, SceneServices, Services};
use vellum_core::ports::{
    Clock, DocStore, DocumentFiles, FileSystem, Rect, ScriptError, ScriptValue, TargetMode,
};
use vellum_core::resolver::libraries::LibrarySource;
use vellum_types::{Actor, Document, FileReason};
use vellum_watch_notify::NotifyWatcher;

/// A scratch folder, removed afterwards.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("vellum-daemon-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("repo")).expect("make the folder");
        std::fs::create_dir_all(path.join("recovery")).expect("make the folder");
        Self(path)
    }
    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }
    fn doc(&self, name: &str) -> String {
        self.repo().join(name).to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Vellum Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git is installed");
    String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr)
}

fn git_ok(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Vellum Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .expect("git is installed");
    assert!(out.success(), "git {args:?} failed");
}

fn commit(repo: &Path, message: &str) {
    git_ok(repo, &["add", "-A"]);
    git_ok(repo, &["commit", "-q", "-m", message]);
}

fn real_env(services: Rc<dyn Services>) -> Env {
    Env {
        locate: Box::new(|path| {
            let path = Path::new(path);
            let parent = path.parent().expect("a folder");
            let name = path
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            Ok((Arc::new(StdFs::new(parent)) as Arc<dyn FileSystem>, name))
        }),
        codec: Arc::new(JsonCodec::new()),
        hash,
        clock: Arc::new(ManualClock::new(1_000_000)) as Arc<dyn Clock>,
        store: Box::new(|doc| {
            Box::new(vellum_doc_loro::LoroDocStore::new(doc, 1)) as Box<dyn DocStore>
        }),
        ids: Box::new(|| Box::new(SequentialIds::new())),
        services,
    }
}

fn daemon(scratch: &Scratch, services: Rc<dyn Services>) -> Documents {
    Documents::new(
        real_env(services),
        Box::new(NotifyWatcher::new(Duration::from_millis(100)).expect("watcher")),
        Recovery::new(
            Arc::new(StdFs::new(scratch.0.join("recovery"))),
            Arc::new(JsonCodec::new()),
            hash,
        ),
    )
}

/// Polls until the watcher reports something other than `Unchanged` for `path`, then makes sure nothing more
/// comes (no loop of the daemon reacting to its own writes).
fn wait_for(docs: &mut Documents, path: &str) -> External {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        for (p, outcome) in docs.poll() {
            if p == path && outcome != External::Unchanged {
                settle(docs, path);
                return outcome;
            }
        }
    }
    panic!("the watcher never reported a change to {path}");
}

fn settle(docs: &mut Documents, path: &str) {
    for _ in 0..12 {
        std::thread::sleep(Duration::from_millis(50));
        for (p, outcome) in docs.poll() {
            assert!(
                p != path || outcome == External::Unchanged,
                "a second report for {path} after it settled: {outcome:?}"
            );
        }
    }
}

fn with_name(doc: &Document, id: &str, name: &str) -> Document {
    let mut store = vellum_contracts::fakes::InMemoryDocStore::new(doc, 9);
    vellum_core::ops::apply(
        &mut store,
        &mut SequentialIds::new(),
        tx(&alice(), vec![rename(id, name)]),
    )
    .expect("rename");
    store.snapshot()
}

fn with_node(doc: &Document, page: &str, id: &str) -> Document {
    let mut store = vellum_contracts::fakes::InMemoryDocStore::new(doc, 9);
    vellum_core::ops::apply(
        &mut store,
        &mut SequentialIds::new(),
        tx(&alice(), vec![insert_at(page, None, 0, node("rect", id))]),
    )
    .expect("insert");
    store.snapshot()
}

fn name_of(doc: &Document, id: &str) -> String {
    samples::find(doc, id).expect("node").name().to_owned()
}

#[test]
fn git_checkout_reaches_a_headless_document_as_ops_and_back() {
    let scratch = Scratch::new("checkout");
    let repo = scratch.repo();
    let path = scratch.doc("app.vellum");
    let v1 = samples::document();
    let v2 = with_node(
        &with_name(&v1, "rect000003", "On the branch"),
        "page00002",
        "rect000009",
    );
    git_ok(&repo, &["init", "-q"]);
    std::fs::write(&path, encode(&v1)).unwrap();
    commit(&repo, "v1");
    git_ok(&repo, &["checkout", "-q", "-b", "branch"]);
    std::fs::write(&path, encode(&v2)).unwrap();
    commit(&repo, "v2");
    git_ok(&repo, &["checkout", "-q", "main"]);

    let mut docs = daemon(&scratch, Rc::new(vellum_core::doc_api::NoServices));
    assert_eq!(docs.target(&path).unwrap().snapshot().unwrap(), v1);

    git_ok(&repo, &["checkout", "-q", "branch"]);
    assert!(matches!(
        wait_for(&mut docs, &path),
        External::Applied { conflicts: 0, .. }
    ));
    assert_eq!(docs.target(&path).unwrap().snapshot().unwrap(), v2);
    assert_eq!(
        git(&repo, &["status", "--porcelain"]),
        "",
        "the daemon wrote nothing back"
    );

    git_ok(&repo, &["checkout", "-q", "main"]);
    assert!(matches!(
        wait_for(&mut docs, &path),
        External::Applied { .. }
    ));
    assert_eq!(docs.target(&path).unwrap().snapshot().unwrap(), v1);
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
}

#[test]
fn git_checkout_of_a_folder_document_reaches_it() {
    let scratch = Scratch::new("folder");
    let repo = scratch.repo();
    let path = scratch.doc("app.vellum");
    let codec = JsonCodec::new();
    let dir = DirCodec::new(&codec);
    let fs = StdFs::new(&repo);
    let v1 = samples::document();
    let v2 = with_name(&v1, "rect000001", "Page file changed");
    git_ok(&repo, &["init", "-q"]);
    dir.write(&fs, "app.vellum", &v1).unwrap();
    commit(&repo, "v1");
    git_ok(&repo, &["checkout", "-q", "-b", "branch"]);
    dir.write(&fs, "app.vellum", &v2).unwrap();
    commit(&repo, "v2");
    git_ok(&repo, &["checkout", "-q", "main"]);

    let mut docs = daemon(&scratch, Rc::new(vellum_core::doc_api::NoServices));
    assert_eq!(docs.target(&path).unwrap().snapshot().unwrap(), v1);
    git_ok(&repo, &["checkout", "-q", "branch"]);
    assert!(matches!(
        wait_for(&mut docs, &path),
        External::Applied { .. }
    ));
    assert_eq!(docs.target(&path).unwrap().snapshot().unwrap(), v2);
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
}

#[test]
fn git_merge_updates_an_open_editor_in_place() {
    let scratch = Scratch::new("merge");
    let repo = scratch.repo();
    let path = scratch.doc("app.vellum");
    let base = samples::document();
    git_ok(&repo, &["init", "-q"]);
    std::fs::write(&path, encode(&base)).unwrap();
    commit(&repo, "base");
    git_ok(&repo, &["checkout", "-q", "-b", "other"]);
    std::fs::write(&path, encode(&with_node(&base, "page00002", "rect000009"))).unwrap();
    commit(&repo, "add a node on page two");
    git_ok(&repo, &["checkout", "-q", "main"]);
    std::fs::write(
        &path,
        encode(&with_name(&base, "rect000001", "Renamed on main")),
    )
    .unwrap();
    commit(&repo, "rename on main");

    let mut docs = daemon(&scratch, Rc::new(vellum_core::doc_api::NoServices));
    let main = decode(&std::fs::read(&path).unwrap());
    let editor = FakeEditor::new(&main);
    let opened = docs
        .open_editor(&path, EditorId("e1".into()), editor.link())
        .unwrap();
    assert_eq!(opened.role, Role::Writer);

    git_ok(&repo, &["merge", "-q", "--no-edit", "other"]);
    assert!(matches!(
        wait_for(&mut docs, &path),
        External::Applied { conflicts: 0, .. }
    ));
    let now = editor.doc();
    assert_eq!(name_of(&now, "rect000001"), "Renamed on main");
    assert!(
        samples::find(&now, "rect000009").is_some(),
        "the merged node arrived in the editor"
    );
    assert_eq!(
        editor.0.borrow().actors,
        vec![Actor::File {
            reason: FileReason::ExternalChange
        }]
    );
    assert_eq!(now, decode(&std::fs::read(&path).unwrap()));
}

#[test]
fn a_text_conflict_applies_nothing_until_it_is_resolved() {
    let scratch = Scratch::new("conflict");
    let repo = scratch.repo();
    let path = scratch.doc("app.vellum");
    let base = samples::document();
    git_ok(&repo, &["init", "-q"]);
    std::fs::write(&path, encode(&base)).unwrap();
    commit(&repo, "base");
    git_ok(&repo, &["checkout", "-q", "-b", "other"]);
    let theirs = with_name(&base, "rect000001", "Theirs");
    std::fs::write(&path, encode(&theirs)).unwrap();
    commit(&repo, "theirs");
    git_ok(&repo, &["checkout", "-q", "main"]);
    let ours = with_name(&base, "rect000001", "Ours");
    std::fs::write(&path, encode(&ours)).unwrap();
    commit(&repo, "ours");

    let mut docs = daemon(&scratch, Rc::new(vellum_core::doc_api::NoServices));
    docs.target(&path).unwrap();
    let out = git(&repo, &["merge", "--no-edit", "other"]);
    assert!(out.contains("CONFLICT"), "{out}");
    assert!(matches!(
        wait_for(&mut docs, &path),
        External::Unreadable(_)
    ));
    assert_eq!(docs.target(&path).unwrap().snapshot().unwrap(), ours);

    git_ok(&repo, &["checkout", "--theirs", "app.vellum"]);
    assert!(matches!(
        wait_for(&mut docs, &path),
        External::Applied { .. }
    ));
    assert_eq!(docs.target(&path).unwrap().snapshot().unwrap(), theirs);
}

#[test]
fn headless_writes_are_not_reported_back_and_atomic_saves_by_others_are() {
    let scratch = Scratch::new("atomic");
    let path = scratch.doc("app.vellum");
    let doc = samples::document();
    std::fs::write(&path, encode(&doc)).unwrap();
    let mut docs = daemon(&scratch, Rc::new(vellum_core::doc_api::NoServices));
    let target = docs.target(&path).unwrap();
    assert_eq!(target.mode(), TargetMode::Headless);
    target
        .transact(tx(&alice(), vec![rename("rect000003", "Agent wrote this")]))
        .unwrap();
    assert_eq!(
        name_of(&decode(&std::fs::read(&path).unwrap()), "rect000003"),
        "Agent wrote this"
    );
    settle(&mut docs, &path);

    let theirs = with_name(&doc, "rect000002", "Another program");
    let temp = scratch.repo().join(".app.vellum.swp");
    std::fs::write(&temp, encode(&theirs)).unwrap();
    std::fs::rename(&temp, &path).unwrap();
    assert!(matches!(
        wait_for(&mut docs, &path),
        External::Applied { .. }
    ));
    let now = docs.target(&path).unwrap().snapshot().unwrap();
    assert_eq!(name_of(&now, "rect000002"), "Another program");
    // Not a merge with the agent's write: that write was already in the file the other program replaced.
    assert_eq!(now, theirs);
}

#[test]
fn recovery_copies_land_in_the_recovery_folder() {
    let scratch = Scratch::new("recovery");
    let path = scratch.doc("app.vellum");
    let doc = samples::document();
    std::fs::write(&path, encode(&doc)).unwrap();
    let mut docs = daemon(&scratch, Rc::new(vellum_core::doc_api::NoServices));
    let editor = FakeEditor::new(&doc);
    docs.open_editor(&path, EditorId("e1".into()), editor.link())
        .unwrap();
    editor.edit(tx(&alice(), vec![rename("rect000001", "Unsaved")]));
    assert_eq!(docs.tick(), vec![path.clone()]);
    let names: Vec<String> = std::fs::read_dir(scratch.0.join("recovery"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.iter().any(|n| n.ends_with(".vellum.recovery")));
    assert_eq!(docs.recoveries().0[0].path, path);
}

struct Libraries;
impl LibrarySource for Libraries {
    fn load(&self, _: &str) -> Result<Option<Document>, String> {
        Ok(None)
    }
}

struct Skia(vellum_render_skia::Renderer);
impl RenderRead for Skia {
    fn render(
        &self,
        output: &vellum_core::display_list::Output,
        bounds: Rect,
        scale: f64,
        format: &str,
    ) -> Result<Rendered, ScriptError> {
        assert_eq!(format, "png");
        let resources = vellum_render_skia::Resources {
            fonts: output.fonts.clone(),
            ..Default::default()
        };
        let scene = self
            .0
            .prepare(&output.list, &resources)
            .map_err(vellum_core::doc_api::error)?;
        let page = vellum_render_skia::Page {
            width: (bounds.width * scale).ceil() as u32,
            height: (bounds.height * scale).ceil() as u32,
            scene: &scene,
        };
        Ok(Rendered {
            mime: "image/png".into(),
            bytes: self.0.png(&page).map_err(vellum_core::doc_api::error)?,
        })
    }
}

#[test]
fn a_headless_target_renders_with_the_shared_renderer() {
    let scratch = Scratch::new("render");
    let path = scratch.doc("app.vellum");
    std::fs::write(&path, encode(&samples::document())).unwrap();
    let fonts = vellum_text_parley::ParleyFonts::new();
    let text = vellum_text_parley::ParleyText::new(fonts.clone());
    let services = SceneServices::new(
        SceneConfig {
            script_outputs: Default::default(),
            path: path.clone(),
            libraries: Rc::new(Libraries),
            layout: RefCell::new(Box::new(vellum_layout_taffy::TaffyLayout::new())),
            text: Rc::new(text),
            fonts: Rc::new(fonts),
            renderer: Rc::new(Skia(vellum_render_skia::Renderer::new().unwrap())),
        },
        vellum_index_rstar::RstarIndex::new,
    );
    let mut docs = daemon(&scratch, Rc::new(services));
    let target = docs.target(&path).unwrap();
    target
        .transact(tx(
            &alice(),
            vec![vellum_contracts::samples::insert_at(
                "page00002",
                None,
                0,
                samples::node_with(
                    "rect",
                    "rect000010",
                    &[],
                    serde_json::json!({"width": 20, "height": 20, "fills": [{"type": "solid", "color": "#123456"}]}),
                ),
            )],
        ))
        .unwrap();
    let ScriptValue::Map(image) = target.render("rect000010", 2.).unwrap() else {
        panic!("an image")
    };
    assert_eq!(
        image.get("mime"),
        Some(&ScriptValue::Text("image/png".into()))
    );
    let Some(ScriptValue::Text(base64)) = image.get("base64") else {
        panic!("base64")
    };
    assert!(base64.starts_with("iVBORw0KGgo"), "a PNG");
}

#[test]
fn the_recovery_folder_is_private() {
    let scratch = Scratch::new("private");
    let home = scratch.0.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let dir = vellum::daemon::recovery_dir(&home).expect("made");
    assert_eq!(dir, home.join(".vellum/recovery"));
    assert_eq!(
        vellum::daemon::recovery_dir(&home).unwrap(),
        dir,
        "again is fine"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            vellum::daemon::recovery_dir(&home).is_err(),
            "a folder others can read is refused"
        );
        let other = scratch.0.join("other-home");
        std::fs::create_dir_all(other.join("elsewhere")).unwrap();
        std::fs::create_dir_all(other.join(".vellum")).unwrap();
        std::os::unix::fs::symlink(other.join("elsewhere"), other.join(".vellum/recovery"))
            .unwrap();
        assert!(
            vellum::daemon::recovery_dir(&other).is_err(),
            "a symlink is refused"
        );
    }
}
