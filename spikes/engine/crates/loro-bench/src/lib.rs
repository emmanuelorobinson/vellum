//! Spike B (guide Prompt 5): the renderer spike's generated document as a Loro movable tree.
//! Spike quality; DocStore on Loro is Prompt 14.
//!
//! Model: one `LoroTree` named `nodes`, fractional indexes on (children are ordered, as layers are),
//! one tree node per scene node, its properties in the node's metadata map. Paints are stored as one
//! compact string per paint, which is roughly how much structure Prompt 7's types will carry.
//!
//! TEST DATA: documents come from `spike_scene::generate`.

use std::collections::BTreeMap;

use loro::{ExportMode, LoroDoc, LoroError, LoroMap, LoroTree, LoroValue, TreeID, TreeParentId};
use spike_scene::{Color, Document, Node, Paint};

/// Nodes one batch touches.
pub const BATCH_SIZE: usize = 200;
const TREE: &str = "nodes";

/// Builds the movable tree for `scene` in one transaction, as peer 1 (deterministic bytes).
pub fn from_document(scene: &Document) -> LoroDoc {
    let doc = LoroDoc::new();
    doc.set_peer_id(1).expect("peer id");
    let tree = doc.get_tree(TREE);
    tree.enable_fractional_index(0);
    for node in &scene.nodes {
        insert(&tree, TreeParentId::Root, node);
    }
    doc.commit();
    doc
}

fn insert(tree: &LoroTree, parent: TreeParentId, node: &Node) {
    let id = tree.create(parent).expect("create");
    let meta = tree.get_meta(id).expect("meta");
    let set = |k: &str, v: LoroValue| {
        meta.insert(k, v).expect("insert");
    };
    let num = |v: f32| LoroValue::Double(f64::from(v));
    match node {
        Node::Frame {
            x,
            y,
            w,
            h,
            radius,
            fill,
            clip,
            opacity,
            children,
        } => {
            set("type", "frame".into());
            for (k, v) in [("x", x), ("y", y), ("w", w), ("h", h), ("radius", radius)] {
                set(k, num(*v));
            }
            set("fill", paint(fill).into());
            set("clip", (*clip).into());
            set("opacity", num(*opacity));
            for child in children {
                insert(tree, TreeParentId::Node(id), child);
            }
        }
        Node::Rect {
            x,
            y,
            w,
            h,
            radius,
            rotation,
            fill,
            stroke,
            blend,
        } => {
            set("type", "rect".into());
            for (k, v) in [
                ("x", x),
                ("y", y),
                ("w", w),
                ("h", h),
                ("radius", radius),
                ("rotation", rotation),
            ] {
                set(k, num(*v));
            }
            if let Some(fill) = fill {
                set("fill", paint(fill).into());
            }
            if let Some(s) = stroke {
                set("stroke", format!("{} {}", hex(s.color), s.width).into());
            }
            set("blend", i64::from(*blend).into());
        }
        Node::Ellipse {
            x,
            y,
            w,
            h,
            fill,
            stroke,
            blend,
        } => {
            set("type", "ellipse".into());
            for (k, v) in [("x", x), ("y", y), ("w", w), ("h", h)] {
                set(k, num(*v));
            }
            if let Some(fill) = fill {
                set("fill", paint(fill).into());
            }
            if let Some(s) = stroke {
                set("stroke", format!("{} {}", hex(s.color), s.width).into());
            }
            set("blend", i64::from(*blend).into());
        }
        Node::Polygon { points, fill } => {
            set("type", "polygon".into());
            let pts: Vec<String> = points.iter().map(|(x, y)| format!("{x},{y}")).collect();
            set("points", pts.join(" ").into());
            set("fill", paint(fill).into());
        }
        Node::Text {
            x,
            y,
            size,
            color,
            text,
        } => {
            set("type", "text".into());
            for (k, v) in [("x", x), ("y", y), ("size", size)] {
                set(k, num(*v));
            }
            set("color", hex(*color).into());
            set("text", text.as_str().into());
        }
        Node::Image {
            x,
            y,
            w,
            h,
            image,
            smooth,
        } => {
            set("type", "image".into());
            for (k, v) in [("x", x), ("y", y), ("w", w), ("h", h)] {
                set(k, num(*v));
            }
            set("asset", format!("image-{image}").into());
            set("smooth", (*smooth).into());
        }
        Node::Glyphs { .. } => unreachable!("generated documents have no laid-out glyphs"),
    }
}

fn hex(c: Color) -> String {
    let [r, g, b, a] = [c.r, c.g, c.b, c.a].map(|v| (v * 255.0).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}

fn paint(p: &Paint) -> String {
    let stops = |s: &[(f32, Color)]| {
        s.iter()
            .map(|(p, c)| format!("{} {p}", hex(*c)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    match p {
        Paint::Solid(c) => hex(*c),
        Paint::Linear { from, to, stops: s } => format!(
            "linear({},{} {},{}; {})",
            from.0,
            from.1,
            to.0,
            to.1,
            stops(s)
        ),
        Paint::Radial {
            center,
            radius,
            stops: s,
        } => format!("radial({},{} {radius}; {})", center.0, center.1, stops(s)),
    }
}

/// A full snapshot (history and state).
pub fn save(doc: &LoroDoc) -> Vec<u8> {
    doc.export(ExportMode::Snapshot).expect("snapshot")
}

pub fn load(bytes: &[u8]) -> Result<LoroDoc, LoroError> {
    LoroDoc::from_snapshot(bytes)
}

pub fn node_count(doc: &LoroDoc) -> usize {
    doc.get_tree(TREE).nodes().len()
}

/// The document's state as JSON (the deep value), a stand-in for the `.vellum` file's size.
pub fn to_json(doc: &LoroDoc) -> String {
    serde_json::to_string(&doc.get_deep_value()).expect("json")
}

/// The state as 2-space-indented JSON, closer to the `.vellum` file's layout (ARCHITECTURE §4.3).
pub fn to_json_pretty(doc: &LoroDoc) -> String {
    serde_json::to_string_pretty(&doc.get_deep_value()).expect("json")
}

/// FNV-1a over bytes, to compare native and WASM output without shipping it around.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Every node's parent and metadata, keyed by id, to count what a batch changed.
pub fn node_states(doc: &LoroDoc) -> BTreeMap<String, (Option<TreeID>, LoroValue)> {
    let tree = doc.get_tree(TREE);
    tree.nodes()
        .into_iter()
        .map(|id| {
            let parent = match tree.parent(id) {
                Some(TreeParentId::Node(p)) => Some(p),
                _ => None,
            };
            let meta: LoroMap = tree.get_meta(id).expect("meta");
            (id.to_string(), (parent, meta.get_deep_value()))
        })
        .collect()
}

/// One edit of a batch: a node, its new position, and the frame it moves into (if any).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchEdit {
    pub node: TreeID,
    pub x: f64,
    pub y: f64,
    pub move_to: Option<TreeID>,
}

/// Picks `BATCH_SIZE` nodes deterministically from `seed`: every node gets a new position, and every
/// fourth leaf is also moved under another top-level frame (as dragging a selection into another frame
/// would). Picking walks the whole tree, so it is kept out of the timed apply.
pub fn pick_batch(doc: &LoroDoc, seed: u64) -> Vec<BatchEdit> {
    let tree = doc.get_tree(TREE);
    let roots = tree.roots();
    let mut nodes = tree.nodes();
    nodes.sort_by_key(|id| (id.peer, id.counter));
    let leaves: Vec<TreeID> = nodes
        .into_iter()
        .filter(|id| tree.children_num(*id).unwrap_or(0) == 0 && !roots.contains(id))
        .collect();
    assert!(leaves.len() >= BATCH_SIZE, "document too small for a batch");
    let step = leaves.len() / BATCH_SIZE;
    (0..BATCH_SIZE)
        .map(|k| {
            let node = leaves[(k * step + seed as usize) % leaves.len()];
            let offset = (seed as f64) + k as f64;
            let move_to = (k % 4 == 0)
                .then(|| roots[(k / 4 + seed as usize) % roots.len()])
                .filter(|target| tree.parent(node) != Some(TreeParentId::Node(*target)));
            BatchEdit {
                node,
                x: 10.0 + offset,
                y: 20.0 + offset,
                move_to,
            }
        })
        .collect()
}

/// Applies a picked batch as one transaction.
pub fn apply_edits(doc: &LoroDoc, edits: &[BatchEdit]) {
    let tree = doc.get_tree(TREE);
    for edit in edits {
        let meta = tree.get_meta(edit.node).expect("meta");
        meta.insert("x", edit.x).expect("x");
        meta.insert("y", edit.y).expect("y");
        if let Some(target) = edit.move_to {
            tree.mov(edit.node, target).expect("move");
        }
    }
    doc.commit();
}

/// `pick_batch` then `apply_edits`.
pub fn apply_batch(doc: &LoroDoc, seed: u64) {
    let edits = pick_batch(doc, seed);
    apply_edits(doc, &edits);
}
