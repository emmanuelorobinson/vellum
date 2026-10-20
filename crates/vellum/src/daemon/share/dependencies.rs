//! File reads come only from typed model fields, never free-form metadata, props or source text.
use std::collections::{BTreeMap, BTreeSet};
use vellum_types::{
    Document, Effect, Node, NodeId, NodeIdPath, NodePatch, Override, Paint, Stroke, Style,
};
struct Reads<'a> {
    path: &'a str,
    files: BTreeSet<String>,
}
impl Reads<'_> {
    fn asset(&mut self, asset: &str) {
        self.files.insert(format!(
            "{}/{}",
            vellum_assets_fs::assets_dir_for(self.path),
            asset
        ));
    }
    fn source(&mut self, source: &str) -> Result<(), String> {
        self.files.insert(super::relative(self.path, source)?);
        Ok(())
    }
    fn paints(&mut self, paints: &[Paint]) -> Result<(), String> {
        for paint in paints {
            match paint {
                Paint::Image(image) => self.asset(image.asset.as_str()),
                Paint::Shader(shader) => self.source(&shader.src)?,
                _ => {}
            }
        }
        Ok(())
    }
    fn effects(&mut self, effects: &[Effect]) -> Result<(), String> {
        for effect in effects {
            if let Effect::Shader(shader) = effect {
                self.source(&shader.src)?;
            }
        }
        Ok(())
    }
    fn nodes(&mut self, nodes: &[Node]) -> Result<(), String> {
        for node in nodes {
            self.node(node)?;
        }
        Ok(())
    }
    fn node(&mut self, node: &Node) -> Result<(), String> {
        match node {
            Node::Image(image) => self.asset(image.asset.as_str()),
            Node::Script(script) => self.source(&script.src)?,
            _ => {}
        }
        self.paints(node.fills())?;
        if let Some(stroke) = node.stroke() {
            self.paints(&stroke.paints)?;
        }
        self.effects(node.effects())?;
        self.nodes(node.children())?;
        if let Node::Instance(instance) = node {
            for nodes in instance.slot_content.values() {
                self.nodes(nodes)?;
            }
            for value in instance.overrides.values() {
                self.override_value(value)?;
            }
        }
        Ok(())
    }
    fn override_value(&mut self, value: &Override) -> Result<(), String> {
        match value {
            Override::Replace(node) => self.node(node),
            Override::Children(nodes) => self.nodes(nodes),
            Override::Props(patch) => self.patch(patch),
        }
    }
    fn patch(&mut self, patch: &NodePatch) -> Result<(), String> {
        let properties = patch.properties();
        if let Some(source) = properties.get("src").and_then(serde_json::Value::as_str) {
            self.source(source)?;
        }
        if let Some(asset) = properties.get("asset").and_then(serde_json::Value::as_str) {
            self.asset(asset);
        }
        if let Some(value) = properties.get("fills").filter(|v| !v.is_null()) {
            let paints: Vec<Paint> = serde_json::from_value(value.clone())
                .map_err(|_| "Invalid snapshot paint override")?;
            self.paints(&paints)?;
        }
        if let Some(value) = properties.get("stroke").filter(|v| !v.is_null()) {
            let stroke: Stroke = serde_json::from_value(value.clone())
                .map_err(|_| "Invalid snapshot stroke override")?;
            self.paints(&stroke.paints)?;
        }
        if let Some(value) = properties.get("effects").filter(|v| !v.is_null()) {
            let effects: Vec<Effect> = serde_json::from_value(value.clone())
                .map_err(|_| "Invalid snapshot effect override")?;
            self.effects(&effects)?;
        }
        if let Some(value) = properties.get("slotContent").filter(|v| !v.is_null()) {
            let slots: BTreeMap<NodeId, Vec<Node>> = serde_json::from_value(value.clone())
                .map_err(|_| "Invalid snapshot slot override")?;
            for nodes in slots.values() {
                self.nodes(nodes)?;
            }
        }
        if let Some(value) = properties.get("overrides").filter(|v| !v.is_null()) {
            let overrides: BTreeMap<NodeIdPath, Override> =
                serde_json::from_value(value.clone())
                    .map_err(|_| "Invalid snapshot nested override")?;
            for value in overrides.values() {
                self.override_value(value)?;
            }
        }
        Ok(())
    }
}
pub(super) fn collect(doc: &Document, path: &str) -> Result<BTreeSet<String>, String> {
    let mut reads = Reads {
        path,
        files: BTreeSet::new(),
    };
    for page in &doc.pages {
        reads.nodes(&page.children)?;
    }
    for component in doc.components.values() {
        reads.node(&component.root)?;
        for variant in &component.variants {
            for value in variant.overrides.values() {
                reads.override_value(value)?;
            }
        }
    }
    for style in doc.styles.values() {
        if let Style::Effect(style) = style {
            reads.effects(&style.effects)?;
        }
    }
    for hash in doc.fonts.keys() {
        reads.asset(hash.as_str());
    }
    Ok(reads.files)
}
