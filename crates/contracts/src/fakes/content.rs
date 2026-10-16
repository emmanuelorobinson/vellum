use vellum_core::ports::{
    ContentError, SkillMeta, SkillSource, StyleLibrary, StyleMeta, VisualStyle,
};

/// Explicit in-memory instructions; tests choose metadata and bodies.
pub struct MemorySkills(pub Vec<(SkillMeta, String)>);
impl SkillSource for MemorySkills {
    fn list(&self) -> Result<Vec<SkillMeta>, ContentError> {
        Ok(self.0.iter().map(|(m, _)| m.clone()).collect())
    }
    fn read(&self, path: &str) -> Result<String, ContentError> {
        self.0
            .iter()
            .find(|(m, _)| m.path == path)
            .map(|(_, s)| s.clone())
            .ok_or_else(|| ContentError::NotFound(path.into()))
    }
}

/// Explicit in-memory recipes; no filesystem or font installation.
pub struct MemoryStyles(pub Vec<VisualStyle>);
impl StyleLibrary for MemoryStyles {
    fn list(&self) -> Result<Vec<StyleMeta>, ContentError> {
        Ok(self
            .0
            .iter()
            .map(|s| StyleMeta {
                name: s.name.clone(),
                description: s.description.clone(),
            })
            .collect())
    }
    fn read(&self, name: &str) -> Result<VisualStyle, ContentError> {
        self.0
            .iter()
            .find(|s| s.name == name)
            .cloned()
            .ok_or_else(|| ContentError::NotFound(name.into()))
    }
}
