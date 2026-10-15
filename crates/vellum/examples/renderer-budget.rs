//! TEST DATA: warm native engine open-to-pixels measurement, without editor/host startup or font discovery.
use std::{fs, time::Instant};
use vellum_core::{
    display_list,
    ports::*,
    resolver::{
        components::Resolver,
        libraries::{Libraries, LibrarySource},
    },
    scene::Scene,
};
use vellum_render_skia::{Page, Renderer, Resources};
use vellum_types::Document;
struct NoLibraries;
impl LibrarySource for NoLibraries {
    fn load(&self, _: &str) -> Result<Option<Document>, String> {
        Ok(None)
    }
}
fn source(label_len: usize) -> String {
    let label = format!("TEST DATA {}", "x".repeat(label_len));
    let children=(0..5000).map(|i|format!(r##"{{"id":"n{i:09}","type":"rect","name":"{label}","width":8,"height":8,"position":{{"type":"absolute","x":{},"y":{}}},"fills":[{{"type":"solid","color":"#2878C8"}}]}}"##,i%100*10,i/100*10)).collect::<Vec<_>>().join(",");
    format!(
        r#"{{"meta":{{"formatVersion":1,"name":"TEST DATA","createdWith":"TEST DATA"}},"pages":[{{"id":"page000001","name":"TEST DATA","children":[{{"id":"root000001","type":"frame","name":"TEST DATA","width":1000,"height":600,"children":[{children}]}}]}}]}}"#
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = source(0);
    let size = 2 * 1024 * 1024;
    let bytes = source((size - base.len()) / 5000);
    let bytes = format!("{bytes}{}", " ".repeat(size - bytes.len()));
    fs::create_dir_all("target/renderer-goldens")?;
    let path = "target/renderer-goldens/open-2mb.vellum";
    fs::write(path, &bytes)?;
    let fonts = vellum_text_parley::ParleyFonts::new();
    let text = vellum_text_parley::ParleyText::new(fonts.clone());
    let mut samples = vec![];
    for _ in 0..20 {
        let start = Instant::now();
        let bytes = fs::read(path)?;
        let doc = vellum_codec_json::decode(&bytes)?;
        let roots = Resolver::new().resolve_page(
            &doc,
            "test.vellum",
            &doc.pages[0],
            &mut Libraries::new(&NoLibraries),
        );
        let tree = vellum_core::layout::from_resolved(&roots[0])?;
        let layout =
            vellum_layout_taffy::TaffyLayout::new().try_layout(&tree, &DirtySet::all(), &text)?;
        let output = display_list::build(
            &roots,
            &layout,
            &text,
            &fonts,
            &mut Scene::new(vellum_index_rstar::RstarIndex::new()),
            display_list::Viewport {
                bounds: Rect::new(0., 0., 1000., 600.),
                scale: 1.,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
        let r = Renderer::new()?;
        let scene = r.prepare_buffer(
            &output.list.to_bytes()?,
            &Resources {
                fonts: output.fonts,
                ..Default::default()
            },
        )?;
        let rgba = r.rgba(&Page {
            width: 1000,
            height: 600,
            scene: &scene,
        })?;
        assert!(rgba.iter().any(|n| *n > 0));
        samples.push(start.elapsed().as_secs_f64() * 1000.);
    }
    samples.sort_by(f64::total_cmp);
    let report = format!(
        "{{\"bytes\":{size},\"nodes\":5000,\"samples\":20,\"p50Ms\":{},\"p95Ms\":{},\"maxMs\":{}}}\n",
        samples[10], samples[19], samples[19]
    );
    fs::write("target/renderer-goldens/native-budget.json", &report)?;
    print!("{report}");
    Ok(())
}
