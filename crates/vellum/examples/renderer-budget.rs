//! TEST DATA: release-only native pipelines. Warm OS cache; includes decode/resolve/layout/list/raster.
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, time::Instant};
use vellum_core::{
    display_list,
    ops::{self, Op, Parent, Place, Transaction},
    ports::*,
    resolver::{
        components::Resolver,
        libraries::{Libraries, LibrarySource},
    },
    scene::Scene,
};
use vellum_render_skia::{Page, Renderer, Resources};
use vellum_types::{Actor, AgentId, Document, Origin};
struct NoLibraries;
impl LibrarySource for NoLibraries {
    fn load(&self, _: &str) -> Result<Option<Document>, String> {
        Ok(None)
    }
}
fn source(label_len: usize) -> String {
    let label = format!("TEST DATA {}", "x".repeat(label_len));
    let children=(0..5000).map(|i|{
       let kind=if i%10==0 {r#""type":"text","content":[{"text":"TEST DATA","font":"Inter","size":8}]"#}else{r#""type":"rect""#};
       format!(r##"{{"id":"n{i:09}",{kind},"name":"{label}","width":8,"height":8,"position":{{"type":"absolute","x":{},"y":{}}},"fills":[{{"type":"solid","color":"#2878C8"}}]}}"##,i%100*10,i/100*10)
    }).collect::<Vec<_>>().join(",");
    format!(
        r#"{{"meta":{{"formatVersion":1,"name":"TEST DATA","createdWith":"TEST DATA"}},"pages":[{{"id":"page000001","name":"TEST DATA","children":[{{"id":"root000001","type":"frame","name":"TEST DATA","width":1000,"height":600,"children":[{children}]}}]}}]}}"#
    )
}
fn stats(mut v: Vec<f64>) -> Value {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    json!({"samples":n,"p50":v[n.div_ceil(2)-1],"p95":v[(n*95).div_ceil(100)-1],"max":v[n-1]})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("Performance budgets require --release".into());
    }
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/performance/native.json".into()),
    );
    fs::create_dir_all(out.parent().ok_or("report parent")?)?;
    let base = source(0);
    let size = 2 * 1024 * 1024;
    let bytes = source((size - base.len()) / 5000);
    let bytes = format!("{bytes}{}", " ".repeat(size - bytes.len()));
    let fixture = out.parent().unwrap().join("open-2mb.vellum");
    fs::write(&fixture, &bytes)?;
    let fonts = vellum_text_parley::ParleyFonts::new();
    let text = vellum_text_parley::ParleyText::new(fonts.clone());
    let renderer = Renderer::new()?;
    let paint = |doc: &Document,
                 pan: f64,
                 scale: f64,
                 basic: bool|
     -> Result<(), Box<dyn std::error::Error>> {
        let roots = Resolver::new().resolve_page(
            doc,
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
                bounds: Rect::new(pan, 0., 1000., 600.),
                scale,
            },
        )
        .map_err(|e| format!("{e:?}"))?;
        let resources = Resources {
            fonts: output.fonts,
            ..Default::default()
        };
        let buffer = output.list.to_bytes()?;
        let scene = if basic {
            renderer.prepare_basic(&buffer, &resources)?
        } else {
            renderer.prepare_buffer(&buffer, &resources)?
        };
        let rgba = renderer.rgba(&Page {
            width: 1000,
            height: 600,
            scene: &scene,
        })?;
        if !rgba.iter().any(|b| *b > 0) {
            return Err("empty paint".into());
        }
        std::hint::black_box(rgba);
        Ok(())
    };
    let doc = vellum_codec_json::decode(bytes.as_bytes())?;
    let disk = vellum_assets_fs::StdFs::new(out.parent().unwrap());
    let codec = vellum_codec_json::JsonCodec::default();
    let dir = vellum_codec_json_dir::DirCodec::new(&codec);
    dir.write(&disk, "open-dir.vellum", &doc)?;
    let mut metrics = BTreeMap::new();
    for basic in [false, true] {
        let prefix = if basic { "basic-native." } else { "" };
        for metric in ["open2mb", "panZoom", "drag", "batch200"] {
            let mut samples = vec![];
            for i in 0..23 {
                let mut store = vellum_doc_loro::LoroDocStore::new(&doc, 1);
                let nodes = &doc.pages[0].children[0].children();
                let mut tx = Transaction::new(
                    Origin::new(Actor::Agent {
                        agent: AgentId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV")?,
                    }),
                    (0..if metric == "drag" { 1 } else { 200 })
                        .map(|j| {
                            let mut node = nodes[j].clone();
                            let id = vellum_types::NodeId::new(format!("b{j:09}")).unwrap();
                            match &mut node {
                                vellum_types::Node::Rect(r) => r.id = id,
                                vellum_types::Node::Text(t) => t.id = id,
                                _ => unreachable!("fixture kinds"),
                            };
                            Op::Insert {
                                node: Box::new(node),
                                to: Place {
                                    parent: Parent::Node(doc.pages[0].children[0].id().clone()),
                                    index: 5000 + j,
                                },
                            }
                        })
                        .collect(),
                );
                if metric == "drag" {
                    tx.ops = vec![Op::Update {
                        id: nodes[0].id().clone(),
                        patch: serde_json::from_value(
                            json!({"position":{"type":"absolute","x":i,"y":0}}),
                        )?,
                    }];
                }
                let start = Instant::now();
                match metric {
                    "open2mb" => {
                        let loaded = vellum_codec_json::decode(&fs::read(&fixture)?)?;
                        let loaded = vellum_doc_loro::LoroDocStore::new(&loaded, 1).snapshot();
                        paint(&loaded, 0., 1., basic)?;
                    }
                    "panZoom" => {
                        paint(&doc, f64::from(i) * 0.3, 0.4 + f64::from(i) * 0.001, basic)?
                    }
                    _ => {
                        ops::apply(
                            &mut store,
                            &mut vellum_contracts::fakes::SequentialIds::new(),
                            tx,
                        )?;
                        paint(&store.snapshot(), 0., 1., basic)?;
                    }
                }
                if i >= 3 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
            metrics.insert(format!("{prefix}{metric}"), stats(samples));
        }
    }
    let mut samples = vec![];
    for _ in 0..20 {
        let start = Instant::now();
        let loaded = dir.read(&disk, "open-dir.vellum")?;
        paint(&loaded.document, 0., 1., false)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.);
    }
    metrics.insert("codec-json-dir.open2mb".into(), stats(samples));
    let machine = std::env::var("VELLUM_PERF_MACHINE")
        .unwrap_or_else(|_| format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH));
    let report = json!({"machine":machine,"lane":"native","adapters":["assets-fs","codec-json","codec-json-dir","doc-loro","layout-taffy","text-parley","index-rstar","render-skia"],"fixture":{"bytes":size,"drawableNodes":5000,"nodes":5001,"textNodes":500,"width":1000,"height":600,"warmup":3},"metrics":metrics});
    fs::write(&out, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
