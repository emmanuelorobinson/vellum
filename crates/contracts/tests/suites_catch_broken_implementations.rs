//! Every suite rejects an implementation that breaks one of its rules. Without this, a suite that checks
//! nothing would pass every fake.

use std::cell::Cell;

use vellum_contracts::fakes::{
    FakeJsonCodec, FixedMetricText, IdentityPathOps, InMemoryDocStore, NaiveSpatialIndex,
    SequentialIds, StackLayout,
};
use vellum_contracts::{samples, suites};
use vellum_core::ops::apply_edits;
use vellum_core::ports::*;
use vellum_types::{Actor, BoolOp, Document, FontStyle, NodeId, TurnId};

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

/// Every actor shares one undo history: undo reverts whoever changed something last.
struct SharedUndo(InMemoryDocStore, Vec<Actor>);
impl DocStore for SharedUndo {
    fn adapter_id(&self) -> &'static str {
        self.0.adapter_id()
    }
    fn snapshot(&self) -> Document {
        self.0.snapshot()
    }
    fn apply(&mut self, commit: Commit) -> ChangeSet {
        self.1.push(commit.origin().actor.clone());
        self.0.apply(commit)
    }
    fn undo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        let last = self.1.pop()?;
        self.0.undo(&last).map(|mut changes| {
            changes.origin.actor = actor.clone();
            changes
        })
    }
    fn redo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        let changes = self.0.redo(actor)?;
        self.1.push(actor.clone());
        Some(changes)
    }
    fn undo_turn(&mut self, turn: &TurnId, by: &Actor) -> UndoTurn {
        self.0.undo_turn(turn, by)
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
    suites::doc_store::run(|doc, peer| SharedUndo(InMemoryDocStore::new(doc, peer), Vec::new()));
}

/// Undo per actor, but each undo puts back whole nodes as they were, over whatever others did since.
struct WholeNodeUndo {
    doc: Document,
    undo: Vec<(Actor, Vec<vellum_core::ops::Edit>)>,
}
impl DocStore for WholeNodeUndo {
    fn adapter_id(&self) -> &'static str {
        "whole-node-undo"
    }
    fn snapshot(&self) -> Document {
        self.doc.clone()
    }
    fn apply(&mut self, commit: Commit) -> ChangeSet {
        let (origin, edits, inverse, changes) = commit.into_parts();
        apply_edits(&mut self.doc, &edits, origin.clone()).expect("a commit applies");
        self.undo.push((origin.actor, inverse));
        changes
    }
    fn undo(&mut self, actor: &Actor) -> Option<ChangeSet> {
        let at = self.undo.iter().rposition(|(a, _)| a == actor)?;
        let (_, inverse) = self.undo.remove(at);
        let origin = vellum_types::Origin::new(actor.clone());
        apply_edits(&mut self.doc, &inverse, origin)
            .ok()
            .map(|(_, changes)| changes)
    }
    fn redo(&mut self, _: &Actor) -> Option<ChangeSet> {
        None
    }
    fn undo_turn(&mut self, _: &TurnId, _: &Actor) -> UndoTurn {
        UndoTurn::Nothing
    }
    fn cursor(&self) -> SyncCursor {
        SyncCursor::empty()
    }
    fn export_updates(&self, _: &SyncCursor) -> SyncPayload {
        SyncPayload {
            adapter: self.adapter_id().into(),
            bytes: vec![],
        }
    }
    fn import_updates(&mut self, _: &SyncPayload) -> Result<ChangeSet, SyncError> {
        Err(SyncError::Malformed("no sync".into()))
    }
}

#[test]
#[should_panic(expected = "Invariant 3")]
fn store_whose_undo_overwrites_later_work() {
    suites::doc_store::undo_is_yours(
        |doc, _| WholeNodeUndo {
            doc: doc.clone(),
            undo: Vec::new(),
        },
        0..50,
    );
}

/// A store that applies only the first edit of each commit. It cannot cut the commit down and pass it on (a
/// commit cannot be built outside the op layer), so it keeps its own document.
struct FirstEditOnly(Document);
impl DocStore for FirstEditOnly {
    fn adapter_id(&self) -> &'static str {
        "first-edit-only"
    }
    fn snapshot(&self) -> Document {
        self.0.clone()
    }
    fn apply(&mut self, commit: Commit) -> ChangeSet {
        let first = &commit.edits()[..1];
        let _ = apply_edits(&mut self.0, first, commit.origin().clone());
        commit.changes().clone()
    }
    fn undo(&mut self, _: &Actor) -> Option<ChangeSet> {
        None
    }
    fn redo(&mut self, _: &Actor) -> Option<ChangeSet> {
        None
    }
    fn undo_turn(&mut self, _: &TurnId, _: &Actor) -> UndoTurn {
        UndoTurn::Nothing
    }
    fn cursor(&self) -> SyncCursor {
        SyncCursor::empty()
    }
    fn export_updates(&self, _: &SyncCursor) -> SyncPayload {
        SyncPayload {
            adapter: self.adapter_id().into(),
            bytes: vec![],
        }
    }
    fn import_updates(&mut self, _: &SyncPayload) -> Result<ChangeSet, SyncError> {
        Err(SyncError::Malformed("no sync".into()))
    }
}

#[test]
#[should_panic(expected = "ops did not run in order")]
fn store_that_applies_part_of_a_commit() {
    suites::doc_store::run(|doc, _| FirstEditOnly(doc.clone()));
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

struct DropsRichStyle(FixedMetricText);
impl TextEngine for DropsRichStyle {
    fn measure(&self, text: &RichText, constraints: Extent) -> TextMetrics {
        self.0.measure(text, constraints)
    }
    fn layout(&self, text: &RichText, width: f64) -> TextLayout {
        let mut l = self.0.layout(text, width);
        for line in &mut l.lines {
            for r in &mut line.glyph_runs {
                r.link = None;
                r.underline = None;
                r.strikethrough = None;
            }
        }
        l
    }
}
#[test]
#[should_panic(expected = "TextEngine: rich style was dropped")]
fn text_that_drops_links_and_decorations() {
    suites::text::rich(&DropsRichStyle(FixedMetricText::new()));
}
