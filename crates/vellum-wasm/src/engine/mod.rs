//! The editor's document engine (ADR-033): the Rust core behind `@vellum/engine-client`'s `EngineBindings`.
//!
//! One engine holds one document in a Loro store. People's transactions go through `vellum_core::ops`, the one
//! write path; agents' and the file's changes arrive as sync updates from the daemon, whose engine this one
//! replicates. The scene (resolver, Taffy layout, Parley text, R-tree) answers the canvas's questions for the
//! page the person is on, and the display list for a viewport comes from the same scene. JSON in and out here
//! is the engine-client facade's shape; the C ABI around it lives in `abi.rs` (Emscripten only).
mod paths;
pub mod tool_ops;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use vellum_core::display_list::{self, Viewport};
use vellum_core::layout;
use vellum_core::ops::{self, Inputs};
use vellum_core::ports::{
    DirtySet, DocStore, FontData, FontHandle, IdGen, LayoutResult, Rect, SyncCursor, SyncPayload,
};
use vellum_core::resolver::libraries::{Libraries, LibrarySource};
use vellum_core::resolver::{ResolvedNode, Resolver};
use vellum_core::scene::{self, HitDepth, Marquee, Scene, SnapAxis, SnapKind, SnapOptions};
use vellum_doc_loro::LoroDocStore;
use vellum_index_rstar::RstarIndex;
use vellum_layout_taffy::TaffyLayout;
use vellum_text_parley::{FontTier, ParleyFonts, ParleyText};
use vellum_types::{Actor, Document, NodeId, NodeIdPath, PageId, Point};

/// How the engine is opened.
#[derive(Debug, Clone)]
pub struct Options {
    /// The document's path, the scope library imports and script outputs resolve against.
    pub path: String,
    /// This replica's sync peer (below 2^63, different for every engine editing the document).
    pub peer: u64,
    /// Where new node ids come from. The host draws it from the platform's random source.
    pub seed: u64,
    /// The file is shown but cannot be edited here (no daemon to own it, or a newer format).
    pub read_only: bool,
}

/// A rendered viewport: the versioned display list and the fonts it draws with.
pub struct Frame {
    pub list: Vec<u8>,
    pub fonts: BTreeMap<FontHandle, FontData>,
}

/// Library documents are files the host has not given the browser engine yet; instances of them show as missing.
struct NoLibraries;
impl LibrarySource for NoLibraries {
    fn load(&self, _: &str) -> Result<Option<Document>, String> {
        Ok(None)
    }
}

/// Ten characters from the node id alphabet, from a seeded generator (SplitMix64).
struct Ids(u64);
impl IdGen for Ids {
    fn node_id(&mut self) -> NodeId {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
        let mut next = || {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        let (a, b) = (next(), next());
        let id: String = (0..10)
            .map(|i| {
                let bits = if i < 8 {
                    a >> (i * 6)
                } else {
                    b >> ((i - 8) * 6)
                };
                ALPHABET[(bits & 63) as usize] as char
            })
            .collect();
        NodeId::new(id).expect("the alphabet is the id alphabet")
    }
}

/// One occurrence on the current page, for `SceneItem`.
struct Item {
    resolved: Arc<ResolvedNode>,
    /// The instance this occurrence is inside, and its path from that instance's root.
    instance: Option<(NodeId, NodeIdPath)>,
}

/// The current page resolved, laid out and indexed, for one revision.
struct View {
    revision: u64,
    page: PageId,
    roots: Vec<Arc<ResolvedNode>>,
    layout: LayoutResult,
    scene: Scene<RstarIndex>,
    items: BTreeMap<NodeIdPath, Item>,
}

pub struct Engine {
    store: LoroDocStore,
    path: String,
    read_only: bool,
    revision: u64,
    page: Option<PageId>,
    ids: Ids,
    fonts: ParleyFonts,
    text: ParleyText,
    resolver: Resolver,
    layout: TaffyLayout,
    view: Option<View>,
}

fn rejected(reason: &str) -> Value {
    json!({"status": "rejected", "reason": reason})
}

pub(crate) fn point(v: &Value) -> Option<Point> {
    Some(Point {
        x: v.get("x")?.as_f64()?,
        y: v.get("y")?.as_f64()?,
    })
}

fn rect(v: &Value) -> Option<Rect> {
    let r = Rect::new(
        v.get("x")?.as_f64()?,
        v.get("y")?.as_f64()?,
        v.get("width")?.as_f64()?,
        v.get("height")?.as_f64()?,
    );
    r.is_valid().then_some(r)
}

fn box_json(r: Rect) -> Value {
    json!({"x": r.x, "y": r.y, "width": r.width, "height": r.height})
}

fn address(text: &str) -> Option<NodeIdPath> {
    text.parse().ok()
}

impl Engine {
    fn with_store(store: LoroDocStore, options: Options) -> Self {
        let fonts = ParleyFonts::empty();
        Self {
            store,
            path: options.path,
            read_only: options.read_only,
            revision: 1,
            page: None,
            ids: Ids(options.seed),
            text: ParleyText::new(fonts.clone()),
            fonts,
            resolver: Resolver::new(),
            layout: TaffyLayout::new(),
            view: None,
        }
    }

    /// An engine over a `.vellum` file's bytes, on its own (no daemon to replicate).
    pub fn open(bytes: &[u8], options: Options) -> Result<Self, String> {
        let doc = vellum_codec_json::decode(bytes).map_err(|e| e.to_string())?;
        if options.peer == 0 || options.peer >= 1 << 63 {
            return Err("A sync peer is between 1 and 2^63".into());
        }
        Ok(Self::with_store(
            LoroDocStore::new(&doc, options.peer),
            options,
        ))
    }

    /// A replica of the daemon's engine: the document it was opened on, then everything since (ADR-033).
    pub fn replica(genesis: &[u8], updates: &[u8], options: Options) -> Result<Self, String> {
        let mut engine = Self::open(genesis, options)?;
        engine.import(updates)?;
        Ok(engine)
    }

    /// Adds a font face the text engine shapes with and the renderer draws with (Inter, emoji, document fonts).
    /// Returns the handles frames name it by; the host keeps the bytes under them, so frames need not copy them.
    pub fn register_font(&mut self, bytes: Vec<u8>) -> Result<Vec<u64>, String> {
        let handles = self
            .fonts
            .register(FontTier::Bundled, bytes)
            .map_err(|e| format!("{e:?}"))?;
        self.changed();
        Ok(handles.into_iter().map(|h| h.0).collect())
    }

    /// The engine-client facade's synchronous calls, as one JSON request (`{"call": name, ...}`).
    pub fn call(&mut self, request: &Value) -> Result<Value, String> {
        let text = |key: &str| {
            request[key]
                .as_str()
                .ok_or_else(|| format!("{key} is missing"))
        };
        let at = |key: &str| point(&request[key]).ok_or_else(|| format!("{key} is not a point"));
        let deepest = || request["depth"].as_str() == Some("deepest");
        Ok(match request["call"].as_str().unwrap_or_default() {
            "snapshot" => self.snapshot(),
            "apply" => self.apply(&request["transaction"]),
            "undo" => self.undo(&request["actor"]),
            "redo" => self.redo(&request["actor"]),
            "setReadOnly" => {
                self.set_read_only(request["readOnly"].as_bool().ok_or("readOnly is missing")?);
                Value::Null
            }
            "setPage" => {
                self.set_page(text("page")?);
                Value::Null
            }
            "nextId" => json!(self.next_id()),
            "get" => self.scene_get(text("address")?),
            "roots" => self.scene_roots(),
            "hitPoint" => self.hit_point(at("point")?, deepest()),
            "hitRect" => self.hit_rect(
                &request["box"],
                deepest(),
                request["mode"].as_str() == Some("contained"),
            ),
            "snap" => self.snap(request),
            "toParent" => {
                let p = self.to_parent(&request["target"], at("point")?);
                json!({"x": p.x, "y": p.y})
            }
            "toLocal" => {
                let p = self.to_local(text("address")?, at("point")?);
                json!({"x": p.x, "y": p.y})
            }
            "path" => self.path(text("address")?),
            other => return Err(format!("Unknown engine call {other}")),
        })
    }

    fn changed(&mut self) {
        self.revision += 1;
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn document(&self) -> Document {
        self.store.snapshot()
    }

    /// `EngineSnapshot`.
    pub fn snapshot(&self) -> Value {
        json!({
            "revision": self.revision,
            "document": self.store.snapshot(),
            "readOnly": self.read_only,
        })
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        if self.read_only != read_only {
            self.read_only = read_only;
            self.changed();
        }
    }

    /// A fresh node id, for the editor's tools.
    pub fn next_id(&mut self) -> String {
        self.ids.node_id().to_string()
    }

    /// The canonical bytes of the document as it is now: what Save writes.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        vellum_codec_json::encode(&self.store.snapshot()).map_err(|e| e.to_string())
    }

    /// Applies a person's `ToolTransaction`; answers an `EngineCommit`.
    pub fn apply(&mut self, request: &Value) -> Value {
        if self.read_only {
            return rejected("read-only");
        }
        let Ok(tx) = tool_ops::transaction(request) else {
            return rejected("invalid");
        };
        let mut inputs = Inputs::default();
        if let Some(view) = self.current() {
            // Measured boxes of top-level occurrences, for ops whose result depends on laid-out size (scale).
            inputs.bounds = view
                .layout
                .boxes
                .iter()
                .filter(|(a, _)| a.len() == 1)
                .map(|(a, b)| (a.first().clone(), *b))
                .collect();
        }
        match ops::apply_with_inputs(&mut self.store, &mut self.ids, tx, &inputs) {
            Ok(_) => {
                self.changed();
                json!({"status": "applied", "revision": self.revision})
            }
            Err(_) => rejected("invalid"),
        }
    }

    fn history(&mut self, actor: &Value, redo: bool) -> Value {
        if self.read_only {
            return rejected("read-only");
        }
        let Ok(actor) = serde_json::from_value::<Actor>(actor.clone()) else {
            return rejected("invalid");
        };
        let done = if redo {
            self.store.redo(&actor)
        } else {
            self.store.undo(&actor)
        };
        match done {
            Some(_) => {
                self.changed();
                json!({"status": "applied", "revision": self.revision})
            }
            None => rejected("unavailable"),
        }
    }

    /// The person's own undo (Invariant 3).
    pub fn undo(&mut self, actor: &Value) -> Value {
        self.history(actor, false)
    }

    pub fn redo(&mut self, actor: &Value) -> Value {
        self.history(actor, true)
    }

    /// How much of the document this replica has seen.
    pub fn cursor(&self) -> Vec<u8> {
        self.store.cursor().0
    }

    /// Everything this replica has that a peer at `cursor` lacks: the person's edits, for the daemon.
    pub fn updates_since(&self, cursor: &[u8]) -> Vec<u8> {
        self.store
            .export_updates(&SyncCursor(cursor.to_vec()))
            .bytes
    }

    /// Updates from the daemon. Returns whether anything changed.
    pub fn import(&mut self, bytes: &[u8]) -> Result<bool, String> {
        if bytes.is_empty() {
            return Ok(false);
        }
        let changes = self
            .store
            .import_updates(&SyncPayload {
                adapter: self.store.adapter_id().into(),
                bytes: bytes.to_vec(),
            })
            .map_err(|e| e.to_string())?;
        if changes.is_empty() {
            return Ok(false);
        }
        self.changed();
        Ok(true)
    }

    /// The page the person is on; the scene answers for it.
    pub fn set_page(&mut self, page: &str) {
        if let Ok(id) = PageId::new(page)
            && self.page.as_ref() != Some(&id)
        {
            self.page = Some(id);
            self.view = None;
        }
    }

    fn current(&mut self) -> Option<&mut View> {
        let stale = self.view.as_ref().is_none_or(|v| {
            v.revision != self.revision || self.page.as_ref().is_some_and(|p| *p != v.page)
        });
        if stale {
            self.view = self.build();
        }
        self.view.as_mut()
    }

    fn build(&mut self) -> Option<View> {
        let doc = self.store.snapshot();
        let page = self
            .page
            .as_ref()
            .and_then(|id| doc.pages.iter().find(|p| p.id == *id))
            .or_else(|| doc.pages.first())?;
        let roots =
            self.resolver
                .resolve_page(&doc, &self.path, page, &mut Libraries::new(&NoLibraries));
        // A root that cannot be laid out (a missing component) is left out, not the whole page.
        let mut boxes = LayoutResult::default();
        let mut laid = Vec::new();
        for root in &roots {
            let Ok(tree) = layout::from_resolved(root) else {
                continue;
            };
            let Ok(result) = self.layout.try_layout(&tree, &DirtySet::all(), &self.text) else {
                continue;
            };
            boxes.boxes.extend(result.boxes);
            boxes.problems.extend(result.problems);
            laid.push(root.clone());
        }
        let nodes = scene::from_resolved(&laid, &boxes).ok()?;
        let mut index = Scene::new(RstarIndex::new());
        index.sync(&nodes).ok()?;
        let mut items = BTreeMap::new();
        fn walk(
            r: &Arc<ResolvedNode>,
            instance: Option<(&NodeId, &NodeIdPath)>,
            items: &mut BTreeMap<NodeIdPath, Item>,
        ) {
            let inside = instance.map(|(id, root)| {
                let rest = r.address.segments().skip(root.len()).cloned();
                let path = rest.fold(None::<NodeIdPath>, |acc, s| {
                    Some(match acc {
                        None => NodeIdPath::from(s),
                        Some(p) => p.join(s),
                    })
                });
                (id.clone(), path)
            });
            items.insert(
                r.address.clone(),
                Item {
                    resolved: r.clone(),
                    instance: inside.and_then(|(id, path)| path.map(|p| (id, p))),
                },
            );
            let own = r.instance.as_ref().map(|_| (r.value.node.id(), &r.address));
            for child in &r.children {
                walk(child, own.or(instance), items);
            }
        }
        for root in &laid {
            walk(root, None, &mut items);
        }
        Some(View {
            revision: self.revision,
            page: page.id.clone(),
            roots: laid,
            layout: boxes,
            scene: index,
            items,
        })
    }

    /// `ToolScene.get`: a `SceneItem` or null.
    pub fn scene_get(&mut self, at: &str) -> Value {
        let Some(a) = address(at) else {
            return Value::Null;
        };
        let Some(view) = self.current() else {
            return Value::Null;
        };
        let (Some(item), Some(o)) = (view.items.get(&a), view.scene.occurrence(&a)) else {
            return Value::Null;
        };
        let mut node = item.resolved.value.node.clone();
        // A SceneItem's node is the occurrence without its children; the scene lists them as addresses.
        if let Ok(bare) = serde_json::to_value(node.without_children()) {
            node = serde_json::from_value(bare).unwrap_or(node);
        }
        json!({
            "node": node,
            "bounds": box_json(o.bounds),
            "children": view.scene.children(&a).iter().map(ToString::to_string).collect::<Vec<_>>(),
            "parent": o.parent.as_ref().map(ToString::to_string),
            "instance": item.instance.as_ref().map(|(id, path)| json!({"id": id, "path": path.to_string()})),
            "selectable": o.visible && !o.locked && o.selectable,
        })
    }

    pub fn scene_roots(&mut self) -> Value {
        let roots = self.current().map(|v| v.scene.roots()).unwrap_or_default();
        json!(roots.iter().map(ToString::to_string).collect::<Vec<_>>())
    }

    pub fn hit_point(&mut self, p: Point, deepest: bool) -> Value {
        let depth = if deepest {
            HitDepth::Deepest
        } else {
            HitDepth::TopLevel
        };
        self.current()
            .and_then(|v| v.scene.hit_point(p, depth))
            .map_or(Value::Null, |a| json!(a.to_string()))
    }

    pub fn hit_rect(&mut self, r: &Value, deepest: bool, contained: bool) -> Value {
        let Some(r) = rect(r) else {
            return json!([]);
        };
        let depth = if deepest {
            HitDepth::Deepest
        } else {
            HitDepth::TopLevel
        };
        let mode = if contained {
            Marquee::Contained
        } else {
            Marquee::Intersects
        };
        let found = self
            .current()
            .map(|v| v.scene.hit_rect(r, depth, mode))
            .unwrap_or_default();
        json!(found.iter().map(ToString::to_string).collect::<Vec<_>>())
    }

    /// `ToolScene.snap(box, exclude, options)`.
    pub fn snap(&mut self, request: &Value) -> Value {
        let none = json!({"offset": {"x": 0, "y": 0}, "guides": []});
        let (Some(moving), Some(o)) = (rect(&request["box"]), request.get("options")) else {
            return none;
        };
        let Some(search) = rect(&o["search"]) else {
            return none;
        };
        let flag = |k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
        let options = SnapOptions {
            search,
            tolerance: o["tolerance"].as_f64().unwrap_or(0.),
            zoom: o["zoom"].as_f64().unwrap_or(1.),
            edges: flag("edges"),
            centres: flag("centres"),
            equal_spacing: flag("equalSpacing"),
            grid: o["grid"].as_f64(),
        };
        let exclude = request["exclude"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().and_then(address))
                    .collect()
            })
            .unwrap_or_default();
        let Some(view) = self.current() else {
            return none;
        };
        let Ok(result) = view.scene.snap(moving, &exclude, options) else {
            return none;
        };
        json!({
            "offset": {"x": result.offset.x, "y": result.offset.y},
            "guides": result.guides.iter().map(|g| json!({
                "axis": match g.axis { SnapAxis::X => "x", SnapAxis::Y => "y" },
                "kind": match g.kind {
                    SnapKind::Edge => "edge",
                    SnapKind::Centre => "centre",
                    SnapKind::EqualSpacing => "equalSpacing",
                    SnapKind::Grid => "grid",
                },
                "coordinate": g.coordinate,
                "targets": g.targets.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "gap": g.gap,
            })).collect::<Vec<_>>(),
        })
    }

    /// `ToolScene.toParent(address | place, point)`: a world point in the coordinates a node's position is
    /// written in, under its parent (or under a place's parent).
    pub fn to_parent(&mut self, target: &Value, p: Point) -> Point {
        let parent = if let Some(at) = target.as_str() {
            let Some(view) = self.current() else {
                return p;
            };
            address(at).and_then(|a| view.scene.occurrence(&a)?.parent)
        } else {
            match target["parent"]["type"].as_str() {
                Some("node") => target["parent"]["id"].as_str().and_then(address),
                _ => None,
            }
        };
        let Some(parent) = parent else {
            return p;
        };
        self.current()
            .and_then(|v| v.scene.to_local(&parent, p))
            .unwrap_or(p)
    }

    /// `ToolScene.toLocal(address, point)`: a world point in a path's viewBox (the node's box for other kinds).
    pub fn to_local(&mut self, at: &str, p: Point) -> Point {
        let Some(a) = address(at) else {
            return p;
        };
        let Some(view) = self.current() else {
            return p;
        };
        let Some(local) = view.scene.to_local(&a, p) else {
            return p;
        };
        let (Some(item), Some(size)) = (view.items.get(&a), view.layout.boxes.get(&a)) else {
            return local;
        };
        match &item.resolved.value.node {
            vellum_types::Node::Vector(v) if size.width > 0. && size.height > 0. => Point {
                x: v.view_box.x + local.x * v.view_box.width / size.width,
                y: v.view_box.y + local.y * v.view_box.height / size.height,
            },
            _ => local,
        }
    }

    /// `ToolScene.path(address)`: the editable outline, or null.
    pub fn path(&mut self, at: &str) -> Value {
        let Some(a) = address(at) else {
            return Value::Null;
        };
        let Some(view) = self.current() else {
            return Value::Null;
        };
        let (Some(item), Some(size)) = (view.items.get(&a), view.layout.boxes.get(&a)) else {
            return Value::Null;
        };
        paths::editable(&item.resolved.value.node, size.width, size.height).unwrap_or(Value::Null)
    }

    /// The display list for a viewport of `width`×`height` CSS pixels at `dpr`, panned and zoomed as the
    /// editor's viewport says (`world = (screen - {x, y}) / zoom`).
    pub fn frame(&mut self, request: &Value) -> Result<Frame, String> {
        let number = |v: &Value| v.as_f64().filter(|n| n.is_finite());
        let (Some(width), Some(height), Some(dpr)) = (
            number(&request["width"]),
            number(&request["height"]),
            number(&request["dpr"]),
        ) else {
            return Err("A frame needs a width, height and device pixel ratio".into());
        };
        let v = &request["viewport"];
        let (Some(x), Some(y), Some(zoom)) = (number(&v["x"]), number(&v["y"]), number(&v["zoom"]))
        else {
            return Err("A frame needs a viewport".into());
        };
        if zoom <= 0. || width <= 0. || height <= 0. || dpr <= 0. {
            return Err("A frame needs a positive size and zoom".into());
        }
        let viewport = Viewport {
            bounds: Rect::new(-x / zoom, -y / zoom, width / zoom, height / zoom),
            scale: zoom * dpr,
        };
        self.current().ok_or("The document has no page to draw")?;
        let Self {
            view, text, fonts, ..
        } = self;
        let view = view.as_mut().expect("just built");
        let output = display_list::build(
            &view.roots,
            &view.layout,
            text,
            fonts,
            &mut view.scene,
            viewport,
        )
        .map_err(|e| format!("{e:?}"))?;
        Ok(Frame {
            list: output.list.to_bytes().map_err(|e| format!("{e:?}"))?,
            fonts: output.fonts,
        })
    }
}

#[cfg(test)]
mod tests;
