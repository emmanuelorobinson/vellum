//! Shared export resources for both current documents and immutable Git snapshots.
use std::{collections::BTreeMap, sync::Arc};
use vellum_core::{
    doc_api::Services,
    ports::{AssetHash, AssetStore, ExportError, FileSystem},
};
use vellum_types::{
    Document,
    display::{RenderPaint, Resource},
};

pub fn prepare(
    doc: &Document,
    fs: Arc<dyn FileSystem>,
    path: &str,
    services: &dyn Services,
    targets: &[String],
) -> Result<vellum_core::export::ExportScene, ExportError> {
    let mut scene = services
        .prepare_export(doc, targets)
        .map_err(ExportError::new)?;
    fn sources(
        r: &vellum_core::resolver::ResolvedNode,
        out: &mut BTreeMap<vellum_types::AssetId, String>,
    ) {
        if let vellum_types::Node::Image(n) = &r.value.node {
            out.insert(n.asset.clone(), r.source.clone());
        }
        for p in r.value.node.fills().iter().chain(
            r.value
                .node
                .stroke()
                .into_iter()
                .flat_map(|s| s.paints.iter()),
        ) {
            if let vellum_types::Paint::Image(p) = p {
                out.insert(p.asset.clone(), r.source.clone());
            }
        }
        for c in &r.children {
            sources(c, out);
        }
    }
    fn shader_origins(
        r: &vellum_core::resolver::ResolvedNode,
        out: &mut BTreeMap<String, std::collections::BTreeSet<String>>,
    ) {
        if !r.value.visible {
            return;
        }
        for src in vellum_core::shaders::sources(&r.value.node) {
            out.entry(src).or_default().insert(r.source.clone());
        }
        for c in &r.children {
            shader_origins(c, out);
        }
    }
    for frame in &mut scene.frames {
        let mut shader_files = BTreeMap::new();
        for root in &frame.roots {
            shader_origins(root, &mut shader_files);
        }
        if shader_files.len() > 64 {
            return Err(ExportError::new(
                "At most 64 shader sources per export frame",
            ));
        }
        for (src, origins) in shader_files {
            if origins.len() != 1 {
                return Err(ExportError::new(
                    "Shader paths conflict across libraries; use distinct relative paths",
                ));
            }
            vellum_core::ports::check_path(&src, false).map_err(ExportError::new)?;
            if src.contains(':') {
                return Err(ExportError::new("Shader sources must be relative files"));
            }
            let owner = origins
                .first()
                .filter(|s| !s.is_empty())
                .map_or(path, String::as_str);
            let dir = owner.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let file = if dir.is_empty() {
                src.clone()
            } else {
                format!("{dir}/{src}")
            };
            let bytes = fs.read(&file).map_err(ExportError::new)?;
            if bytes.len() > 16 * 1024 {
                return Err(ExportError::new("Shader source exceeds 16 KiB"));
            }
            let code = String::from_utf8(bytes).map_err(ExportError::new)?;
            #[cfg(feature = "scene")]
            {
                use vellum_core::ports::{ShaderCompiler, ShaderTarget};
                vellum_shader_sksl::SkslCompiler
                    .compile(&code, ShaderTarget::Sksl)
                    .map_err(ExportError::new)?;
            }
            frame.shaders.insert(src, code.into());
        }
        let mut origins = BTreeMap::new();
        for root in &frame.roots {
            sources(root, &mut origins);
        }
        for resource in &frame.output.list.resources {
            let image = match resource {
                Resource::Image(i) | Resource::Paint(RenderPaint::Image(i)) => i,
                _ => continue,
            };
            if frame.images.contains_key(&image.asset) {
                continue;
            }
            let source = origins
                .get(&image.asset)
                .filter(|p| !p.is_empty())
                .map_or(path, String::as_str);
            let store = vellum_assets_fs::FsAssets::new(
                fs.clone(),
                vellum_assets_fs::assets_dir_for(source),
            )
            .map_err(ExportError::new)?;
            let hash = AssetHash::new(image.asset.as_str()).map_err(ExportError::new)?;
            let bytes = store.get(&hash).map_err(ExportError::new)?;
            frame.images.insert(image.asset.clone(), bytes.into());
        }
    }
    Ok(scene)
}
