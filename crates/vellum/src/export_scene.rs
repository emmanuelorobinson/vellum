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
    for frame in &mut scene.frames {
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
