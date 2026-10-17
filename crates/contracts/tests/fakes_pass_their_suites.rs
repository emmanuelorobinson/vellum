//! Every fake passes its own suite (guide Prompt 10).

use vellum_contracts::fakes::{
    FakeJsonCodec, FixedMetricText, IdentityPathOps, InMemoryAssets, InMemoryDocStore, ManualClock,
    NaiveSpatialIndex, NoopScriptRuntime, PassthroughShaderCompiler, SequentialIds, StackLayout,
    StaticFonts,
};
use vellum_contracts::{samples, suites};
use vellum_core::ports::TokenFormat;

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
    suites::text::rich(&FixedMetricText::new());
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
    suites::assets::collect(&InMemoryAssets::new());
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

#[test]
fn document_target() {
    suites::target::run(vellum_contracts::fakes::InMemoryTarget::new);
}

#[test]
fn file_watcher() {
    let watcher = vellum_contracts::fakes::ManualWatcher::new();
    suites::watch::run(&mut watcher.clone(), &mut suites::watch::ToldDisk(watcher));
}

#[test]
fn tokens_and_spec() {
    let set = samples::token_set();
    let fake = vellum_contracts::fakes::MemoryTokens::default();
    suites::tokens::format(&fake, &set, &fake.default_options(&set.themes));
    suites::spec::format(
        &vellum_contracts::fakes::MemorySpec,
        &samples::document(),
        &samples::node_id("frame00001"),
    );
}

#[test]
fn import_and_asset_ports() {
    use vellum_contracts::fakes::{MemoryFetch, MemoryIcons, MemoryImages, MemoryImporter};
    use vellum_core::ports::IconStyle;
    suites::import::importer(&MemoryImporter, b"memory:Mark");
    suites::import::importer(&MemoryImporter, b"memory:Photo:image");
    for (style, fill) in [
        (IconStyle::Stroke, false),
        (IconStyle::Fill, true),
        (IconStyle::Fill, false),
    ] {
        suites::import::icon_library(
            &MemoryIcons::new("memory-icons", style, fill),
            "house",
            "home",
        );
    }
    suites::import::image_source(&MemoryImages, "TEST DATA kitchen");
    suites::import::http_fetch(
        &MemoryFetch::default().with("https://example.com/a.png", "image/png", &[1, 2, 3]),
        "https://example.com/a.png",
    );
}
