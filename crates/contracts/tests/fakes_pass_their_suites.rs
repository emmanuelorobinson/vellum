//! Every fake passes its own suite (guide Prompt 10).

use vellum_contracts::fakes::{
    FakeJsonCodec, FixedMetricText, IdentityPathOps, InMemoryAssets, InMemoryDocStore, ManualClock,
    NaiveSpatialIndex, NoopScriptRuntime, PassthroughShaderCompiler, SequentialIds, StackLayout,
    StaticFonts,
};
use vellum_contracts::{samples, suites};

#[test]
fn clock() {
    suites::clock::run_clock(&ManualClock::new(1_700_000_000_000));
    let clock = ManualClock::new(5);
    clock.advance(10);
    suites::clock::run_clock(&clock);
}

#[test]
fn id_gen() {
    suites::clock::run_id_gen(&mut SequentialIds::new());
}

#[test]
fn codec() {
    suites::codec::run(
        &FakeJsonCodec,
        &[
            samples::document(),
            vellum_types::Document::new("TEST DATA: empty"),
        ],
    );
}

#[test]
fn doc_store() {
    suites::doc_store::run(InMemoryDocStore::new);
    suites::doc_store::undo_is_yours(InMemoryDocStore::new, 0..200);
}

#[test]
fn layout() {
    suites::layout::run(StackLayout::new, &FixedMetricText::new());
}

#[test]
fn text() {
    suites::text::run(&FixedMetricText::new());
}

#[test]
fn fonts() {
    suites::fonts::run(
        &StaticFonts::new(&["TEST DATA Sans", "TEST DATA Serif"]),
        "TEST DATA Sans",
        "TEST DATA Serif",
        "No Such Family",
    );
}

#[test]
fn spatial_index() {
    suites::spatial::run(NaiveSpatialIndex::new);
}

#[test]
fn path_ops() {
    suites::path::run(&IdentityPathOps);
}

#[test]
fn assets() {
    suites::assets::run(&InMemoryAssets::new());
}

#[test]
fn script_runtime() {
    suites::script::run(&NoopScriptRuntime);
}

#[test]
fn shader_compiler() {
    suites::shader::run(&PassthroughShaderCompiler);
}

/// Cold-path ports are used as trait objects, hot-path ones through generics; both must compile.
#[test]
fn ports_are_object_safe() {
    use vellum_core::ports::{
        AssetStore, Clock, Codec, DocStore, FontSource, IdGen, LayoutEngine, PathOps,
        ScriptRuntime, ShaderCompiler, SpatialIndex, TextEngine,
    };
    let _: &dyn Clock = &ManualClock::new(0);
    let _: Box<dyn IdGen> = Box::new(SequentialIds::new());
    let _: &dyn Codec = &FakeJsonCodec;
    let _: Box<dyn DocStore> = Box::new(InMemoryDocStore::new(&samples::document(), 1));
    let _: Box<dyn LayoutEngine> = Box::new(StackLayout::new());
    let _: &dyn TextEngine = &FixedMetricText::new();
    let _: &dyn FontSource = &StaticFonts::new(&[]);
    let _: Box<dyn SpatialIndex> = Box::new(NaiveSpatialIndex::new());
    let _: &dyn PathOps = &IdentityPathOps;
    let _: &dyn AssetStore = &InMemoryAssets::new();
    let _: &dyn ScriptRuntime = &NoopScriptRuntime;
    let _: &dyn ShaderCompiler = &PassthroughShaderCompiler;
}
