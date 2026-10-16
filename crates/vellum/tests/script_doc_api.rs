//! TEST DATA. Real QuickJS → Ops → resolver → Taffy → shared Skia; no GPU or files required.
use std::{cell::RefCell, rc::Rc};
use vellum_contracts::{
    fakes::{InMemoryDocStore, SequentialIds},
    samples,
};
use vellum_core::{
    display_list,
    doc_api::{
        self, ExecuteContext, ExecuteRequest, RenderRead, Rendered, SceneConfig, SceneServices,
    },
    guard::GuardState,
    ports::{Rect, ScriptError, ScriptValue, Timestamp},
    resolver::libraries::LibrarySource,
};
use vellum_types::Document;
struct Libraries;
impl LibrarySource for Libraries {
    fn load(&self, _: &str) -> Result<Option<Document>, String> {
        Ok(None)
    }
}
struct Skia(vellum_render_skia::Renderer);
impl RenderRead for Skia {
    fn render(
        &self,
        output: &display_list::Output,
        bounds: Rect,
        scale: f64,
        format: &str,
    ) -> Result<Rendered, ScriptError> {
        let resources = vellum_render_skia::Resources {
            fonts: output.fonts.clone(),
            ..Default::default()
        };
        let scene = self
            .0
            .prepare(&output.list, &resources)
            .map_err(doc_api::error)?;
        let page = vellum_render_skia::Page {
            width: (bounds.width * scale).ceil() as u32,
            height: (bounds.height * scale).ceil() as u32,
            scene: &scene,
        };
        let (mime, bytes) = match format {
            "png" => ("image/png", self.0.png(&page).map_err(doc_api::error)?),
            "svg" => (
                "image/svg+xml",
                self.0.svg(&page).map_err(doc_api::error)?.into_bytes(),
            ),
            "pdf" => (
                "application/pdf",
                self.0.pdf(&[page]).map_err(doc_api::error)?,
            ),
            _ => return Err(doc_api::error("Unsupported format")),
        };
        Ok(Rendered {
            mime: mime.into(),
            bytes,
        })
    }
}
#[test]
fn doc_api_screenshot_and_exports_use_production_scene_and_renderer() {
    let fonts = vellum_text_parley::ParleyFonts::new();
    let text = vellum_text_parley::ParleyText::new(fonts.clone());
    let services = SceneServices::new(
        SceneConfig {
            script_outputs: Default::default(),
            path: "TEST-DATA.vellum".into(),
            libraries: Rc::new(Libraries),
            layout: RefCell::new(Box::new(vellum_layout_taffy::TaffyLayout::new())),
            text: Rc::new(text),
            fonts: Rc::new(fonts),
            renderer: Rc::new(Skia(vellum_render_skia::Renderer::new().unwrap())),
        },
        vellum_index_rstar::RstarIndex::new,
    );
    let mut store = InMemoryDocStore::new(&samples::document(), 1);
    let out=doc_api::execute(&vellum_script_quickjs::QuickJsRuntime::new(||0,1),&mut store,ExecuteRequest {source:"const n=doc.insert('page00002',{type:'rect',width:20,height:20,fills:[{type:'solid',color:'#123456'}]});return [doc.screenshot(n),doc.export(n,{format:'svg'}),doc.export(n,{format:'pdf'})];",origin:vellum_types::Origin::new(samples::alice()),validate_only:false,limits:doc_api::EXECUTE_LIMITS},ExecuteContext {ids:Rc::new(RefCell::new(SequentialIds::new())),services:Rc::new(services),guard:&mut GuardState::default(),now:Timestamp(0)}).unwrap();
    let ScriptValue::List(images) = out.value else {
        panic!("images")
    };
    let expected = [
        ("image/png", "iVBORw0KGgo"),
        ("image/svg+xml", "PHN2Zy"),
        ("application/pdf", "JVBER"),
    ];
    for (image, (mime, prefix)) in images.iter().zip(expected) {
        let ScriptValue::Map(data) = image else {
            panic!("image data")
        };
        assert_eq!(data.get("mime"), Some(&ScriptValue::Text(mime.into())));
        let Some(ScriptValue::Text(encoded)) = data.get("base64") else {
            panic!("base64")
        };
        assert!(
            encoded.starts_with(prefix),
            "{mime}: {}",
            &encoded[..encoded.len().min(30)]
        );
    }
}
