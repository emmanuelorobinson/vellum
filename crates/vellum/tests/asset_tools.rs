//! TEST DATA. Icons, pictures and imports through the tool registry (guide Prompt 42), over fakes except where a
//! real permission broker and a real disk are the point.
mod common;

use std::path::PathBuf;
use std::rc::Rc;

use common::Daemon;
use serde_json::{Value, json};
use vellum::permissions::{Brokered, Outside};
use vellum::tools::{ErrorKind, Registry, Session, ToolCtx, ToolError, Verb};
use vellum_contracts::fakes::{ManualClock, MemorySessions, ScriptedPrompt};
use vellum_core::permissions::{Broker, PermissionSettings};
use vellum_core::ports::*;
use vellum_types::{Actor, AgentId, DeviceId, Document, Node};

const PATH: &str = "app.vellum";

fn doc() -> Document {
    serde_json::from_value(json!({
        "meta": {"formatVersion": 1, "name": "TEST DATA: assets", "createdWith": "Vellum 0.0.0"},
        "variables": {"varInk000001": {"type": "color", "name": "color/ink", "values": [{"value": "#112233"}]}},
        "pages": [{"id": "page000001", "name": "Page", "children": [
            {"type": "frame", "id": "frame00001", "name": "Card", "width": 200, "height": 100,
             "fills": [{"type": "solid", "color": "#FFFFFF"}], "metadata": {"keep": "me", "attribution": {"stale": true}}},
            {"type": "image", "id": "image00001", "name": "Photo", "width": 100, "height": 80, "asset": "a0000001"},
            {"type": "text", "id": "text000001", "name": "Title", "width": "hug", "height": "hug",
             "content": [{"text": "Hi", "font": "Inter", "size": 16}]}
        ]}]
    }))
    .unwrap()
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n".to_vec();
    b.extend_from_slice(&13u32.to_be_bytes());
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b
}

fn b64(bytes: &[u8]) -> String {
    vellum_core::base64::encode(bytes)
}

struct Rig {
    daemon: Daemon,
    registry: Registry,
    session: Session,
}

impl Rig {
    fn new() -> Self {
        let mut rig = Self {
            daemon: Daemon::new(&doc(), &[PATH]),
            registry: Registry::standard(),
            session: Session::default(),
        };
        rig.ok("open_document", json!({"path": PATH}));
        rig
    }

    fn call(&mut self, tool: &str, input: Value) -> Result<Value, ToolError> {
        let mut ctx = ToolCtx::new(&mut self.daemon.docs, &mut self.session);
        self.registry.call(&mut ctx, tool, input)
    }

    fn ok(&mut self, tool: &str, input: Value) -> Value {
        self.call(tool, input)
            .unwrap_or_else(|e| panic!("{tool} failed: {e} ({e:?})"))
    }

    fn doc(&mut self) -> Document {
        self.daemon.docs.target(PATH).unwrap().snapshot().unwrap()
    }

    fn node(&mut self, id: &str) -> Value {
        fn walk(nodes: &[Node], id: &str) -> Option<Value> {
            nodes.iter().find_map(|n| {
                if n.id().as_str() == id {
                    serde_json::to_value(n).ok()
                } else {
                    walk(n.children(), id)
                }
            })
        }
        walk(&self.doc().pages[0].children, id).unwrap_or_else(|| panic!("no node {id}"))
    }
}

// ---- search_icons -------------------------------------------------------------------------------------

#[test]
fn search_lists_the_libraries_and_finds_icons_across_them_best_first() {
    let mut rig = Rig::new();
    let listing = rig.ok("search_icons", json!({}));
    let ids: Vec<_> = listing["libraries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "lucide",
            "material-symbols-outlined",
            "material-symbols-rounded",
            "material-symbols-sharp",
            "phosphor",
            "feather"
        ]
    );
    assert!(listing.get("results").is_none());
    let found = rig.ok("search_icons", json!({"query": "settings", "limit": 6}));
    let results = found["results"].as_array().unwrap();
    assert_eq!(results.len(), 6);
    let libs: std::collections::BTreeSet<_> = results
        .iter()
        .map(|r| r["library"].as_str().unwrap())
        .collect();
    assert!(
        libs.len() >= 4,
        "one set does not crowd out the rest: {libs:?}"
    );
    assert!(results.iter().all(|r| r["name"].as_str().is_some()));
    let one = rig.ok(
        "search_icons",
        json!({"query": "arrow right", "library": "lucide", "limit": 3}),
    );
    assert_eq!(one["results"][0]["name"], "arrow-right");
    assert_eq!(one["libraries"].as_array().unwrap().len(), 1);
}

#[test]
fn search_refuses_what_it_cannot_do_in_words() {
    let mut rig = Rig::new();
    let e = rig
        .call(
            "search_icons",
            json!({"query": "x", "library": "fontawesome"}),
        )
        .unwrap_err();
    assert_eq!(
        (e.kind(), e.path()),
        (ErrorKind::InvalidInput, Some("library"))
    );
    assert!(e.message().contains("lucide"), "{}", e.message());
    for limit in [0, 101] {
        assert_eq!(
            rig.call("search_icons", json!({"query": "x", "limit": limit}))
                .unwrap_err()
                .path(),
            Some("limit")
        );
    }
}

// ---- insert_icon --------------------------------------------------------------------------------------

#[test]
fn an_outline_icon_becomes_a_stroked_vector_scaled_to_its_size_with_its_origin_recorded() {
    let mut rig = Rig::new();
    let out = rig.ok(
        "insert_icon",
        json!({"parent": "frame00001", "library": "lucide", "name": "star", "size": 48, "weight": 300,
               "color_variable": "varInk000001", "x": 8, "y": 4}),
    );
    let id = out["created"][0].as_str().unwrap().to_owned();
    assert_eq!(
        out["icon"],
        json!({"library": "lucide", "name": "star", "weight": 300, "fill": false})
    );
    assert_eq!(out["notes"], json!([]));
    let node = rig.node(&id);
    assert_eq!(node["type"], "vector");
    assert_eq!(node["name"], "lucide:star");
    assert_eq!(
        (node["width"].as_f64(), node["height"].as_f64()),
        (Some(48.), Some(48.))
    );
    assert_eq!(
        node["position"],
        json!({"type": "absolute", "x": 8.0, "y": 4.0})
    );
    assert_eq!(
        node["viewBox"],
        json!({"x": 0.0, "y": 0.0, "width": 24.0, "height": 24.0})
    );
    // Weight 300 is a line of 1.5 units in the 24 unit grid, so 3 pixels at 48.
    assert_eq!(
        node["stroke"],
        json!({"paints": [{"type": "solid", "color": {"$var": "varInk000001"}}], "width": 3.0, "cap": "round", "join": "round"})
    );
    assert_eq!(
        node["metadata"]["icon"],
        json!({"library": "lucide", "name": "star", "weight": 300, "fill": false})
    );
    assert!(node["path"].as_str().unwrap().starts_with('M'));
    assert!(node.get("fills").is_none());
    assert_eq!(rig.session.steps().last().unwrap().verb, Verb::AddedAsset);
}

#[test]
fn a_filled_icon_is_a_filled_vector_and_the_notes_say_when_the_exact_ask_was_not_possible() {
    let mut rig = Rig::new();
    let out = rig.ok(
        "insert_icon",
        json!({"parent": "page000001", "library": "phosphor", "name": "house", "weight": 250, "fill": true, "color": "#FF0000", "layer_name": "Home"}),
    );
    let node = rig.node(out["created"][0].as_str().unwrap());
    assert_eq!(node["name"], "Home");
    assert_eq!(
        node["fills"],
        json!([{"type": "solid", "color": "#FF0000"}])
    );
    assert!(node.get("stroke").is_none());
    assert_eq!(
        node["viewBox"],
        json!({"x": 0.0, "y": 0.0, "width": 256.0, "height": 256.0})
    );
    assert_eq!(out["icon"]["fill"], json!(true));

    let out = rig.ok("insert_icon", json!({"parent": "page000001", "library": "lucide", "name": "house", "weight": 250, "fill": true}));
    let notes: Vec<_> = out["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(
        notes[0].contains("weight 200") || notes[0].contains("weight 300"),
        "{notes:?}"
    );
    assert!(notes[1].contains("no filled"), "{notes:?}");
}

#[test]
fn insert_icon_says_what_was_wrong_and_what_is_close() {
    let mut rig = Rig::new();
    let base = json!({"parent": "page000001", "library": "lucide", "name": "star"});
    let with = |k: &str, v: Value| {
        let mut m = base.clone();
        m[k] = v;
        m
    };
    let e = rig
        .call("insert_icon", with("name", json!("starr")))
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::NotFound);
    assert!(e.message().contains("star"), "{}", e.message());
    for (field, value) in [
        ("size", json!(2)),
        ("size", json!(5000)),
        ("weight", json!(900)),
        ("color", json!("red")),
        ("library", json!("nope")),
    ] {
        let e = rig.call("insert_icon", with(field, value)).unwrap_err();
        assert_eq!(
            (e.kind(), e.path()),
            (ErrorKind::InvalidInput, Some(field)),
            "{field}"
        );
    }
    let e = rig
        .call("insert_icon", with("parent", json!("missing0001")))
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidInput);
    assert!(rig.doc().pages[0].children.len() == 3, "nothing was added");
}

#[test]
fn an_inserted_icon_is_one_undoable_change_by_the_agent() {
    let mut rig = Rig::new();
    let before = rig.doc();
    rig.ok(
        "insert_icon",
        json!({"parent": "page000001", "library": "feather", "name": "home"}),
    );
    let after = rig.doc();
    assert_eq!(after.pages[0].children.len(), 4);
    assert_eq!(&after.pages[0].children[..3], &before.pages[0].children[..]);
}

// ---- fill_image with inline data -----------------------------------------------------------------------

fn assets_in(rig: &Rig) -> Vec<String> {
    rig.daemon.fs.list("app.assets").unwrap_or_default()
}

#[test]
fn a_shape_gets_an_image_fill_and_the_picture_is_stored_beside_the_document_once() {
    let mut rig = Rig::new();
    let bytes = png(40, 30);
    let out = rig.ok(
        "fill_image",
        json!({"node": "frame00001", "data": b64(&bytes), "fit": "fit"}),
    );
    let hash = vellum_assets_fs::content_hash(&bytes);
    assert_eq!(out["asset"], hash.as_str());
    assert_eq!(out["mime"], "image/png");
    assert_eq!(assets_in(&rig), std::slice::from_ref(&hash));
    assert_eq!(
        rig.daemon.fs.read(&format!("app.assets/{hash}")).unwrap(),
        bytes
    );
    let card = rig.node("frame00001");
    assert_eq!(
        card["fills"],
        json!([{"type": "solid", "color": "#FFFFFF"}, {"type": "image", "asset": hash, "fit": "fit"}]),
        "the picture goes on top of the existing fills"
    );
    assert_eq!(
        card["metadata"],
        json!({"keep": "me"}),
        "other metadata stays; a stale attribution goes"
    );

    // Again with another picture: the image paint is replaced, not stacked.
    let second = png(50, 50);
    rig.ok(
        "fill_image",
        json!({"node": "frame00001", "data": format!("data:image/png;base64,{}", b64(&second))}),
    );
    let card = rig.node("frame00001");
    let fills = card["fills"].as_array().unwrap();
    assert_eq!(fills.len(), 2);
    assert_eq!(
        fills[1]["asset"],
        vellum_assets_fs::content_hash(&second).as_str()
    );
    assert!(
        fills[1].get("fit").is_none(),
        "fill is the default fit, so the file does not spell it"
    );
    assert_eq!(assets_in(&rig).len(), 2);
}

#[test]
fn an_image_layer_gets_the_new_picture() {
    let mut rig = Rig::new();
    let bytes = png(10, 10);
    rig.ok(
        "fill_image",
        json!({"node": "image00001", "data": b64(&bytes), "fit": "stretch"}),
    );
    let photo = rig.node("image00001");
    assert_eq!(
        photo["asset"],
        vellum_assets_fs::content_hash(&bytes).as_str()
    );
    assert_eq!(photo["fit"], "stretch");
    assert_eq!(
        (photo["width"].as_f64(), photo["height"].as_f64()),
        (Some(100.), Some(80.)),
        "its size is its own"
    );
}

#[test]
fn fill_image_refuses_the_wrong_input_in_words() {
    let mut rig = Rig::new();
    let good = b64(&png(4, 4));
    let cases = [
        (
            json!({"node": "frame00001"}),
            "path",
            ErrorKind::InvalidInput,
        ),
        (
            json!({"node": "frame00001", "data": good, "query": "cat"}),
            "path",
            ErrorKind::InvalidInput,
        ),
        (
            json!({"node": "frame00001", "data": good, "fit": "zoom"}),
            "fit",
            ErrorKind::InvalidInput,
        ),
        (
            json!({"node": "frame00001", "data": "!!!"}),
            "data",
            ErrorKind::InvalidInput,
        ),
        (
            json!({"node": "frame00001", "data": b64(b"GIF89a....")}),
            "",
            ErrorKind::InvalidInput,
        ),
        (
            json!({"node": "text000001", "data": good}),
            "node",
            ErrorKind::InvalidInput,
        ),
        (
            json!({"node": "missing0001", "data": good}),
            "",
            ErrorKind::NotFound,
        ),
    ];
    for (input, path, kind) in cases {
        let e = rig.call("fill_image", input.clone()).unwrap_err();
        assert_eq!(e.kind(), kind, "{input}: {e}");
        if !path.is_empty() {
            assert_eq!(e.path(), Some(path), "{input}");
        }
    }
    assert!(
        assets_in(&rig).is_empty(),
        "nothing was stored for a refused call"
    );
}

#[test]
fn files_and_the_stock_library_are_refused_with_a_way_forward_when_nothing_can_ask() {
    let mut rig = Rig::new();
    let e = rig
        .call(
            "fill_image",
            json!({"node": "frame00001", "path": "/tmp/x.png"}),
        )
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Refused);
    assert!(e.message().contains("`data`"), "{}", e.message());
    let e = rig
        .call("fill_image", json!({"node": "frame00001", "query": "cat"}))
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Refused);
    assert!(e.message().contains("Settings"), "{}", e.message());
    let e = rig
        .call(
            "import",
            json!({"parent": "page000001", "path": "/tmp/x.svg"}),
        )
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Refused);
    assert!(e.message().contains("`data`"), "{}", e.message());
}

// ---- permission: files and the stock library ---------------------------------------------------------

struct Pics;
impl ImageSource for Pics {
    fn id(&self) -> &'static str {
        "stock"
    }
    fn origin(&self) -> &'static str {
        "https://stock.example.com"
    }
    fn search(&self, query: &ImageQuery) -> Result<ImagePage, ImageError> {
        let attribution = Attribution {
            provider: "pixabay".into(),
            author: "TEST DATA Sam".into(),
            author_url: "https://example.com/sam".into(),
            source_url: "https://example.com/p/1".into(),
            licence: "Test licence".into(),
            licence_url: "https://example.com/licence".into(),
        };
        let hits = (0..2)
            .map(|i| ImageHit {
                id: i.to_string(),
                description: format!("TEST DATA {}", query.query),
                width: 64,
                height: 48,
                preview_url: "https://stock.example.com/s.jpg".into(),
                download_url: format!("https://stock.example.com/{i}.png"),
                attribution: attribution.clone(),
            })
            .collect();
        Ok(ImagePage { hits, total: 2 })
    }
    fn fetch(&self, hit: &ImageHit) -> Result<FetchedImage, ImageError> {
        Ok(FetchedImage {
            mime: "image/png".into(),
            bytes: png(64 + hit.id.parse::<u32>().unwrap(), 48),
            attribution: hit.attribution.clone(),
        })
    }
}

struct Asking {
    prompt: Rc<ScriptedPrompt>,
}

/// Lets `rig`'s agent ask: a real broker over a session store that knows the agent's session.
fn let_it_ask(rig: &mut Rig, answer: Answer) -> Asking {
    let session = rig
        .session
        .id()
        .expect("introduced by the first call")
        .clone();
    let agent: AgentId = rig.session.agent().expect("introduced").clone();
    let store = Rc::new(MemorySessions::new());
    store
        .create(NewSession {
            id: session,
            doc: DocKey::new("/repo", "app.vellum").unwrap(),
            kind: SessionKind::External {
                client: "Claude Code".into(),
            },
            agent,
            title: "TEST DATA".into(),
            created_at: Timestamp(0),
        })
        .unwrap();
    let prompt = Rc::new(ScriptedPrompt::always(answer));
    let broker = Broker::new(
        PermissionSettings::default(),
        store,
        Rc::new(ManualClock::new(1)),
        Some(prompt.clone()),
        Actor::Person {
            device: DeviceId::new("01J000000000000000000000QD").unwrap(),
        },
    );
    let outside: Rc<dyn Outside> = Rc::new(Brokered::new(Rc::new(broker), None));
    rig.daemon.docs.set_outside(outside);
    Asking { prompt }
}

#[test]
fn stock_pictures_ask_once_download_into_the_repository_and_write_attribution_to_the_layer() {
    let mut rig = Rig::new();
    let asking = let_it_ask(&mut rig, Answer::AllowOnce);
    rig.daemon.docs.set_stock(Rc::new(Pics));
    let out = rig.ok(
        "fill_image",
        json!({"node": "frame00001", "query": "red kettle", "result": 1}),
    );
    let questions = asking.prompt.asked();
    assert_eq!(
        questions.len(),
        1,
        "one question for the search and the download"
    );
    assert_eq!(
        questions[0].action,
        Action::Network {
            url: "https://stock.example.com".into()
        }
    );
    let hash = vellum_assets_fs::content_hash(&png(65, 48));
    assert_eq!(out["asset"], hash.as_str());
    assert_eq!(out["stock"]["result"], 1);
    assert_eq!(out["attribution"]["author"], "TEST DATA Sam");
    assert_eq!(assets_in(&rig), [hash]);
    let card = rig.node("frame00001");
    assert_eq!(
        card["metadata"]["attribution"],
        json!({"provider": "pixabay", "author": "TEST DATA Sam", "authorUrl": "https://example.com/sam",
               "sourceUrl": "https://example.com/p/1", "licence": "Test licence", "licenceUrl": "https://example.com/licence"})
    );
    assert_eq!(card["metadata"]["keep"], "me");
    let e = rig
        .call(
            "fill_image",
            json!({"node": "frame00001", "query": "red kettle", "result": 5}),
        )
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::NotFound);
    assert!(e.message().contains("only 2 results"), "{}", e.message());
}

#[test]
fn a_denied_network_request_stops_the_call_and_changes_nothing() {
    let mut rig = Rig::new();
    let _ = let_it_ask(&mut rig, Answer::Deny);
    rig.daemon.docs.set_stock(Rc::new(Pics));
    let before = rig.doc();
    let e = rig
        .call("fill_image", json!({"node": "frame00001", "query": "cat"}))
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Refused);
    assert!(
        e.message().contains("https://stock.example.com"),
        "{}",
        e.message()
    );
    assert_eq!(rig.doc(), before);
    assert!(assets_in(&rig).is_empty());
}

static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A folder on the real disk, removed at the end.
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "v-assets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_is_read_only_after_the_person_allows_it() {
    let folder = Folder::new();
    std::fs::write(folder.0.join("logo.png"), png(20, 10)).unwrap();
    std::fs::write(folder.0.join("big.png"), vec![0u8; 33 * 1024 * 1024]).unwrap();
    let mut rig = Rig::new();
    rig.session = Session::in_dir(&folder.0);
    // The document is in the folder the client was started in.
    let seeded = folder.0.join(PATH);
    let key = seeded.to_string_lossy().trim_start_matches('/').to_owned();
    rig.daemon.fs.seed(&key, &common::encode(&doc()));
    rig.ok("open_document", json!({"path": PATH}));

    let asking = let_it_ask(&mut rig, Answer::AllowOnce);
    let out = rig.ok(
        "fill_image",
        json!({"node": "frame00001", "path": "logo.png"}),
    );
    assert_eq!(out["mime"], "image/png");
    let asked = asking.prompt.asked();
    assert_eq!(asked.len(), 1);
    assert_eq!(
        asked[0].action,
        Action::ReadFile {
            path: folder.0.join("logo.png").to_string_lossy().into_owned()
        }
    );

    let e = rig
        .call(
            "fill_image",
            json!({"node": "frame00001", "path": "nothing.png"}),
        )
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::NotFound);
    let e = rig
        .call(
            "fill_image",
            json!({"node": "frame00001", "path": "big.png"}),
        )
        .unwrap_err();
    assert!(e.message().contains("at most"), "{}", e.message());

    let denied = let_it_ask(&mut rig, Answer::Deny);
    let e = rig
        .call(
            "fill_image",
            json!({"node": "frame00001", "path": "logo.png"}),
        )
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Refused);
    assert_eq!(denied.prompt.asked().len(), 1);
}

// ---- import --------------------------------------------------------------------------------------------

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20" viewBox="0 0 40 20">
  <rect id="bg" width="40" height="20" rx="4" fill="#123456"/><circle cx="10" cy="10" r="5" fill="#fff"/>
  <path d="M20 5h15" stroke="#fff" stroke-width="2"/></svg>"##;

#[test]
fn an_svg_comes_in_as_a_frame_of_editable_layers_at_the_asked_place() {
    let mut rig = Rig::new();
    let out = rig.ok(
        "import",
        json!({"parent": "page000001", "text": SVG, "name": "logo.svg", "x": 300, "y": 40}),
    );
    assert_eq!(out["importer"], "svg");
    assert_eq!(out["nodes"], 4);
    assert_eq!(out["notes"], json!([]));
    let frame = rig.node(out["created"][0].as_str().unwrap());
    assert_eq!(frame["name"], "logo");
    assert_eq!(
        frame["position"],
        json!({"type": "absolute", "x": 300.0, "y": 40.0})
    );
    let kinds: Vec<_> = frame["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(kinds, ["rect", "ellipse", "vector"]);
    assert_eq!(frame["children"][0]["name"], "bg");
    assert_eq!(frame["children"][0]["cornerRadius"], json!(4.0));
}

#[test]
fn a_picture_is_stored_and_laid_out_with_its_note_when_large() {
    let mut rig = Rig::new();
    let bytes = png(3200, 1600);
    let out = rig.ok(
        "import",
        json!({"parent": "frame00001", "data": b64(&bytes), "name": "hero.png"}),
    );
    assert_eq!(out["importer"], "image");
    assert_eq!(
        out["assets"],
        json!([vellum_assets_fs::content_hash(&bytes)])
    );
    let image = rig.node(out["created"][0].as_str().unwrap());
    assert_eq!(
        (
            image["name"].as_str(),
            image["width"].as_f64(),
            image["height"].as_f64()
        ),
        (Some("hero"), Some(1600.), Some(800.))
    );
    assert_eq!(
        image["asset"],
        vellum_assets_fs::content_hash(&bytes).as_str()
    );
    assert!(out["notes"][0].as_str().unwrap().contains("3200 by 1600"));
    assert_eq!(assets_in(&rig).len(), 1);
}

#[test]
fn a_figma_copy_comes_in_as_layers_with_the_standing_note_about_pictures() {
    let mut rig = Rig::new();
    let paste = include_str!("../../adapters/import-figma-clipboard/tests/fixtures/selection.html");
    let out = rig.ok(
        "import",
        json!({"parent": "page000001", "text": paste, "x": 10, "y": 10}),
    );
    assert_eq!(out["importer"], "figma-clipboard");
    assert_eq!(out["created"].as_array().unwrap().len(), 2);
    assert!(
        out["notes"][0]
            .as_str()
            .unwrap()
            .contains("does not carry images")
    );
    let card = rig.node(out["created"][0].as_str().unwrap());
    assert_eq!(card["name"], "Card");
    assert_eq!(
        card["position"],
        json!({"type": "absolute", "x": 90.0, "y": 10.0})
    );
    assert_eq!(card["children"].as_array().unwrap().len(), 8);
}

#[test]
fn import_says_what_is_wrong() {
    let mut rig = Rig::new();
    let base = json!({"parent": "page000001"});
    let with = |k: &str, v: Value| {
        let mut m = base.clone();
        m[k] = v;
        m
    };
    assert_eq!(
        rig.call("import", base.clone()).unwrap_err().path(),
        Some("path")
    );
    let e = rig
        .call("import", with("text", json!("just some words")))
        .unwrap_err();
    assert!(e.message().contains("not an SVG"), "{}", e.message());
    let e = rig
        .call(
            "import",
            json!({"parent": "page000001", "text": SVG, "format": "pdf"}),
        )
        .unwrap_err();
    assert_eq!(e.path(), Some("format"));
    let e = rig
        .call(
            "import",
            json!({"parent": "page000001", "text": "<svg", "format": "svg"}),
        )
        .unwrap_err();
    assert!(e.message().contains("could not be read"), "{}", e.message());
    let e = rig
        .call("import", json!({"parent": "missing0001", "text": SVG}))
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidInput);
    assert_eq!(rig.doc().pages[0].children.len(), 3);
    assert!(assets_in(&rig).is_empty());
}

#[test]
fn an_import_is_one_change_so_one_undo_takes_it_all() {
    let mut rig = Rig::new();
    let before = rig.doc();
    rig.ok("import", json!({"parent": "page000001", "text": SVG}));
    let steps = rig.session.steps().len();
    assert!(steps >= 1);
    let after = rig.doc();
    assert_eq!(
        after.pages[0].children.len(),
        before.pages[0].children.len() + 1
    );
}
