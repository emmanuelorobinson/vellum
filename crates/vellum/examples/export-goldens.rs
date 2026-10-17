//! TEST DATA. Generate offline export fixtures and verify the native golden before the tagged Chrome lane.
use std::{fs, path::PathBuf, sync::Arc};
use vellum_core::{export::*, ports::*};
struct Host(std::rc::Rc<dyn vellum_core::doc_api::Services>);
impl ExportRenderer for Host {
    fn render(&self, frames: &[ExportFrame], format: &str) -> Result<Vec<u8>, ExportError> {
        self.0.render_export(frames, format)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let check = std::env::args().any(|a| a == "--check");
    let mut manifest = vec![];
    for name in ["geometry", "layout", "text", "gradient"] {
        let input = format!("fixtures/exports/{name}.vellum");
        let doc = vellum_codec_json::JsonCodec::new().decode(&fs::read(root.join(&input))?)?;
        let services = vellum::daemon::production::export_services(
            &doc,
            Arc::new(vellum_assets_fs::StdFs::new(&root)),
            &input,
        )?;
        let scene = services.prepare_export(&doc, &["frame00001".into()])?;
        let host = Host(services);
        let png = host.render(&scene.frames, "png")?;
        let golden = root.join(format!("fixtures/exports/{name}.png"));
        if check {
            assert_eq!(png, fs::read(golden)?, "native golden changed: {name}");
        } else {
            fs::write(golden, png)?;
        }
        for format in ["html-css", "html-tailwind"] {
            for scaffold in [true, false] {
                for assets in [AssetMode::Inline, AssetMode::Files] {
                    let case = format!(
                        "{name}-{format}-{}-{}",
                        if scaffold { "page" } else { "fragment" },
                        if assets == AssetMode::Inline {
                            "inline"
                        } else {
                            "files"
                        }
                    );
                    let dir = root.join("target/export-goldens").join(&case);
                    fs::create_dir_all(&dir)?;
                    let bundle = vellum_export_html::Html::new(format).unwrap().export(
                        &scene,
                        &CodeOptions { scaffold, assets },
                        &host,
                    )?;
                    for f in bundle.files {
                        let path = dir.join(f.name);
                        fs::create_dir_all(path.parent().unwrap())?;
                        fs::write(path, f.bytes)?;
                    }
                    manifest.push(
                        serde_json::json!({"case":case,"fixture":name,"width":160,"height":100}),
                    );
                }
            }
        }
    }
    fs::write(
        root.join("target/export-goldens/manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "{} export cases generated; native goldens {}",
        manifest.len(),
        if check { "matched" } else { "written" }
    );
    Ok(())
}
