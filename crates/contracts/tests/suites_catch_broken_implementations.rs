//! Every suite rejects an implementation that breaks one of its rules. Without this, a suite that checks
//! nothing would pass every fake.

use std::cell::Cell;

use vellum_contracts::fakes::{
    FakeJsonCodec, FixedMetricText, IdentityPathOps, InMemoryDocStore, NaiveSpatialIndex,
    SequentialIds, StackLayout,
};
use vellum_contracts::{samples, suites};
use vellum_core::ports::*;
use vellum_types::{BoolOp, Document, FontStyle, NodeId};

struct BackwardsClock(Cell<i64>);
impl Clock for BackwardsClock {
    fn now(&self) -> Timestamp {
        self.0.set(self.0.get() - 1);
        Timestamp(self.0.get())
    }
}

#[test]
#[should_panic(expected = "the time went back")]
fn clock_that_goes_back() {
    suites::clock::run_clock(&BackwardsClock(Cell::new(0)));
}

struct RepeatingIds(SequentialIds, u32);
impl IdGen for RepeatingIds {
    fn node_id(&mut self) -> NodeId {
        self.1 += 1;
        if self.1.is_multiple_of(7) {
            NodeId::new("id00000001").unwrap()
        } else {
            self.0.node_id()
        }
    }
}

#[test]
#[should_panic(expected = "minted twice")]
fn ids_that_repeat() {
    suites::clock::run_id_gen(&mut RepeatingIds(SequentialIds::new(), 0));
}

struct AcceptsAnything;
impl Codec for AcceptsAnything {
    fn id(&self) -> &'static str {
        "lax"
    }
    fn decode(&self, _bytes: &[u8]) -> Result<Document, CodecError> {
        Ok(samples::document())
    }
    fn encode(&self, doc: &Document) -> Result<Vec<u8>, CodecError> {
        FakeJsonCodec.encode(doc)
    }
}

#[test]
#[should_panic(expected = "was accepted")]
fn codec_that_accepts_garbage() {
    suites::codec::run(&AcceptsAnything, &[samples::document()]);
}

struct Unstable(Cell<u32>);
impl Codec for Unstable {
    fn id(&self) -> &'static str {
        "unstable"
    }
    fn decode(&self, bytes: &[u8]) -> Result<Document, CodecError> {
        FakeJsonCodec.decode(bytes)
    }
    fn encode(&self, doc: &Document) -> Result<Vec<u8>, CodecError> {
        self.0.set(self.0.get() + 1);
        let mut bytes = FakeJsonCodec.encode(doc)?;
        bytes.extend(std::iter::repeat_n(b' ', self.0.get() as usize));
        Ok(bytes)
    }
}

#[test]
#[should_panic(expected = "different bytes twice")]
fn codec_that_is_not_deterministic() {
    suites::codec::run(&Unstable(Cell::new(0)), &[samples::document()]);
}

/// Every actor shares one undo history.
struct SharedUndo(InMemoryDocStore);
impl DocStore for SharedUndo {
    fn adapter_id(&self) -> &'static str {
        self.0.adapter_id()
    }
    fn snapshot(&self) -> Document {
        self.0.snapshot()
    }
    fn apply(&mut self, mut tx: Transaction) -> Result<ChangeSet, OpError> {
        let origin = tx.origin.clone();
        tx.origin.actor = ActorId::new("everyone");
        self.0.apply(tx).map(|mut changes| {
            changes.origin = origin;
            changes
        })
    }
    fn undo(&mut self, actor: &ActorId) -> Option<ChangeSet> {
        self.0.undo(&ActorId::new("everyone")).map(|mut changes| {
            changes.origin.actor = actor.clone();
            changes
        })
    }
    fn redo(&mut self, actor: &ActorId) -> Option<ChangeSet> {
        self.0.redo(&ActorId::new("everyone")).map(|mut changes| {
            changes.origin.actor = actor.clone();
            changes
        })
    }
    fn cursor(&self) -> SyncCursor {
        self.0.cursor()
    }
    fn export_updates(&self, since: &SyncCursor) -> SyncPayload {
        self.0.export_updates(since)
    }
    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError> {
        self.0.import_updates(payload)
    }
}

#[test]
#[should_panic(expected = "touched Bob's work")]
fn store_whose_undo_is_not_per_actor() {
    suites::doc_store::run(|doc, peer| SharedUndo(InMemoryDocStore::new(doc, peer)));
}

/// A store that forgets nothing it should but lets a half-applied transaction through.
struct NotAtomic(InMemoryDocStore);
impl DocStore for NotAtomic {
    fn adapter_id(&self) -> &'static str {
        self.0.adapter_id()
    }
    fn snapshot(&self) -> Document {
        self.0.snapshot()
    }
    fn apply(&mut self, tx: Transaction) -> Result<ChangeSet, OpError> {
        let first = Transaction {
            origin: tx.origin.clone(),
            ops: tx.ops[..1.min(tx.ops.len())].to_vec(),
        };
        let result = self.0.apply(tx);
        if result.is_err() {
            let _ = self.0.apply(first);
        }
        result
    }
    fn undo(&mut self, actor: &ActorId) -> Option<ChangeSet> {
        self.0.undo(actor)
    }
    fn redo(&mut self, actor: &ActorId) -> Option<ChangeSet> {
        self.0.redo(actor)
    }
    fn cursor(&self) -> SyncCursor {
        self.0.cursor()
    }
    fn export_updates(&self, since: &SyncCursor) -> SyncPayload {
        self.0.export_updates(since)
    }
    fn import_updates(&mut self, payload: &SyncPayload) -> Result<ChangeSet, SyncError> {
        self.0.import_updates(payload)
    }
}

#[test]
#[should_panic(expected = "refused transaction changed the document")]
fn store_that_is_not_atomic() {
    suites::doc_store::run(|doc, peer| NotAtomic(InMemoryDocStore::new(doc, peer)));
}

struct Shifted(StackLayout);
impl LayoutEngine for Shifted {
    fn layout(
        &mut self,
        tree: &LayoutTree,
        dirty: &DirtySet,
        text: &dyn TextEngine,
    ) -> LayoutResult {
        let mut result = self.0.layout(tree, dirty, text);
        result.boxes.values_mut().for_each(|b| b.x += 1.0);
        result
    }
}

#[test]
#[should_panic(expected = "LayoutEngine")]
fn layout_that_is_off_by_one() {
    suites::layout::run(|| Shifted(StackLayout::new()), &FixedMetricText::new());
}

/// Incremental layout that trusts the dirty set too much: it reuses its first result for anything not named.
struct Stale(StackLayout, Option<LayoutResult>);
impl LayoutEngine for Stale {
    fn layout(
        &mut self,
        tree: &LayoutTree,
        dirty: &DirtySet,
        text: &dyn TextEngine,
    ) -> LayoutResult {
        let fresh = self.0.layout(tree, dirty, text);
        let Some(old) = &self.1 else {
            self.1 = Some(fresh.clone());
            return fresh;
        };
        let mut merged = fresh.clone();
        for (id, rect) in &old.boxes {
            if !dirty.all && !dirty.nodes.contains(id) {
                merged.boxes.insert(id.clone(), *rect);
            }
        }
        merged
    }
}

#[test]
#[should_panic(expected = "stale box")]
fn layout_that_reuses_too_much() {
    suites::layout::run(|| Stale(StackLayout::new(), None), &FixedMetricText::new());
}

struct WiderThanItSays(FixedMetricText);
impl TextEngine for WiderThanItSays {
    fn measure(&self, text: &RichText, constraints: Extent) -> TextMetrics {
        let mut m = self.0.measure(text, constraints);
        m.width += 1.0;
        m
    }
    fn layout(&self, text: &RichText, width: f64) -> TextLayout {
        self.0.layout(text, width)
    }
}

#[test]
#[should_panic(expected = "measure and layout differ in width")]
fn text_whose_measure_disagrees_with_layout() {
    suites::text::run(&WiderThanItSays(FixedMetricText::new()));
}

struct NeverWraps(FixedMetricText);
impl TextEngine for NeverWraps {
    fn measure(&self, text: &RichText, _c: Extent) -> TextMetrics {
        self.0.measure(text, Extent::UNBOUNDED)
    }
    fn layout(&self, text: &RichText, _width: f64) -> TextLayout {
        self.0.layout(text, f64::INFINITY)
    }
}

#[test]
#[should_panic(expected = "TextEngine")]
fn text_that_never_wraps() {
    suites::text::run(&NeverWraps(FixedMetricText::new()));
}

struct OneFont;
impl FontSource for OneFont {
    fn resolve(&self, family: &str, _w: u16, _s: FontStyle) -> Option<FontHandle> {
        (family != "No Such Family").then_some(FontHandle(1))
    }
}

#[test]
#[should_panic(expected = "share a handle")]
fn fonts_that_share_a_handle() {
    suites::fonts::run(
        &OneFont,
        "TEST DATA Sans",
        "TEST DATA Serif",
        "No Such Family",
    );
}

struct AlwaysFound;
impl FontSource for AlwaysFound {
    fn resolve(&self, family: &str, _w: u16, _s: FontStyle) -> Option<FontHandle> {
        Some(FontHandle(family.len() as u64))
    }
}

#[test]
#[should_panic(expected = "resolved at weight")]
fn fonts_that_find_everything() {
    suites::fonts::run(
        &AlwaysFound,
        "TEST DATA Sans",
        "TEST DATA Serif",
        "No Such Family",
    );
}

struct StrictEdges(NaiveSpatialIndex);
impl SpatialIndex for StrictEdges {
    fn rebuild(&mut self, items: &[(NodeId, Rect)]) {
        self.0.rebuild(items);
    }
    fn update(&mut self, changed: &[(NodeId, Rect)], removed: &[NodeId]) {
        self.0.update(changed, removed);
    }
    fn query_rect(&self, r: Rect) -> Vec<NodeId> {
        self.0.query_rect(Rect::new(
            r.x + 1e-3,
            r.y + 1e-3,
            r.width - 2e-3,
            r.height - 2e-3,
        ))
    }
    fn query_point(&self, x: f64, y: f64) -> Vec<NodeId> {
        self.0.query_point(x, y)
    }
}

#[test]
#[should_panic(expected = "SpatialIndex")]
fn spatial_index_that_excludes_edges() {
    suites::spatial::run(|| StrictEdges(NaiveSpatialIndex::new()));
}

struct IgnoresStroke;
impl PathOps for IgnoresStroke {
    fn boolean(&self, a: &Path, b: &Path, op: BoolOp) -> Path {
        IdentityPathOps.boolean(a, b, op)
    }
    fn bounds(&self, path: &Path, _stroke: Option<&StrokeStyle>) -> Option<Rect> {
        IdentityPathOps.bounds(path, None)
    }
}

#[test]
#[should_panic(expected = "stroke")]
fn path_ops_that_ignore_the_stroke() {
    suites::path::run(&IgnoresStroke);
}

struct OverwritesByLength(std::sync::Mutex<std::collections::BTreeMap<usize, Vec<u8>>>);
impl AssetStore for OverwritesByLength {
    fn get(&self, hash: &AssetHash) -> Result<Vec<u8>, AssetError> {
        let len = usize::from_str_radix(hash.as_str(), 16).unwrap_or(usize::MAX);
        self.0
            .lock()
            .unwrap()
            .get(&len)
            .cloned()
            .ok_or_else(|| AssetError::NotFound(hash.clone()))
    }
    fn put(&self, bytes: &[u8], _mime: &str) -> Result<AssetHash, AssetError> {
        self.0.lock().unwrap().insert(bytes.len(), bytes.to_vec());
        Ok(AssetHash::new(format!("{:x}", bytes.len())).unwrap())
    }
}

#[test]
#[should_panic(expected = "share a hash")]
fn assets_hashed_by_length() {
    suites::assets::run(&OverwritesByLength(Default::default()));
}

struct Flaky(Cell<u32>);
impl ScriptRuntime for Flaky {
    fn run(
        &self,
        _src: &str,
        _api: &dyn ScriptApi,
        _limits: ScriptLimits,
    ) -> Result<ScriptOutput, ScriptError> {
        self.0.set(self.0.get() + 1);
        Ok(ScriptOutput {
            value: ScriptValue::Number(f64::from(self.0.get())),
            logs: vec![],
        })
    }
}

#[test]
#[should_panic(expected = "Invariant 8")]
fn script_runtime_that_is_not_deterministic() {
    suites::script::run(&Flaky(Cell::new(0)));
}

struct CompilesNothing;
impl ShaderCompiler for CompilesNothing {
    fn compile(&self, src: &str, target: ShaderTarget) -> Result<CompiledShader, ShaderError> {
        Ok(CompiledShader {
            target,
            code: src.to_owned(),
        })
    }
}

#[test]
#[should_panic(expected = "compiled for")]
fn shader_compiler_that_accepts_empty_source() {
    suites::shader::run(&CompilesNothing);
}
