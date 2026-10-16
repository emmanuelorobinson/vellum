use std::rc::Rc;
use std::sync::Arc;

use vellum_core::ports::{ContentError, FileSystem, SkillGroup, SkillMeta, SkillSource};

use super::{path, read};

struct Builtin {
    path: &'static str,
    name: &'static str,
    description: &'static str,
    body: &'static str,
}

macro_rules! skill {
    ($path:literal, $name:literal, $description:literal) => {
        Builtin {
            path: $path,
            name: $name,
            description: $description,
            body: include_str!(concat!("../../../../packages/skills/builtin/", $path)),
        }
    };
}

const BUILTINS: &[Builtin] = &[
    skill!(
        "format.md",
        "Document format",
        "Node specs, variables, components and paths."
    ),
    skill!(
        "execute.md",
        "Execute API",
        "Atomic scripts, limits, checks and error recovery."
    ),
    Builtin {
        path: "vellum-doc.d.ts",
        name: "Doc API declarations",
        description: "Generated declarations for doc API 1.0.",
        body: include_str!("../../../../packages/skills/src/generated/vellum-doc.d.ts"),
    },
    skill!(
        "workflows/design-a-screen.md",
        "Design a screen",
        "Build a screen from a brief and verify it."
    ),
    skill!(
        "workflows/build-a-design-system.md",
        "Build a design system",
        "Create variables and reusable components with guidelines."
    ),
    skill!(
        "workflows/recreate-from-code.md",
        "Recreate from code",
        "Translate supplied code and tokens into editable layers."
    ),
    skill!(
        "workflows/make-a-chart-with-a-script.md",
        "Make a chart with a script",
        "Draw a deterministic chart from explicit data."
    ),
];

/// Compiled from the versioned content files. Does not require an open document or a network.
pub struct BuiltinSkills;

impl SkillSource for BuiltinSkills {
    fn list(&self) -> Result<Vec<SkillMeta>, ContentError> {
        Ok(BUILTINS
            .iter()
            .map(|s| SkillMeta {
                path: s.path.into(),
                name: s.name.into(),
                description: s.description.into(),
                group: SkillGroup::Builtin,
            })
            .collect())
    }
    fn read(&self, path: &str) -> Result<String, ContentError> {
        BUILTINS
            .iter()
            .find(|s| s.path == path)
            .map(|s| s.body.to_owned())
            .ok_or_else(|| ContentError::NotFound(path.into()))
    }
}

/// The ordered sources. Later sources own duplicate paths; their errors never fall back to an older body.
pub struct StackedSkills(Vec<Rc<dyn SkillSource>>);

impl StackedSkills {
    pub fn new(sources: Vec<Rc<dyn SkillSource>>) -> Self {
        Self(sources)
    }
}

impl SkillSource for StackedSkills {
    fn list(&self) -> Result<Vec<SkillMeta>, ContentError> {
        let mut result: Vec<SkillMeta> = Vec::new();
        for source in &self.0 {
            for meta in source.list()? {
                result.retain(|old| old.path != meta.path);
                result.push(meta);
            }
        }
        Ok(result)
    }
    fn read(&self, path: &str) -> Result<String, ContentError> {
        for source in self.0.iter().rev() {
            if source.list()?.iter().any(|m| m.path == path) {
                return source.read(path);
            }
        }
        Err(ContentError::NotFound(path.into()))
    }
}

/// A fixed set of user-selected SKILL.md folders or design-system guideline folders, under an injected root.
/// Reads only these files; metadata and bodies are refreshed each call. No recursion or arbitrary read path.
pub struct FileSkills {
    fs: Arc<dyn FileSystem>,
    files: Vec<String>,
    group: SkillGroup,
}

impl FileSkills {
    pub fn folders(fs: Arc<dyn FileSystem>, folders: Vec<String>) -> Result<Self, ContentError> {
        Self::new(fs, folders, "SKILL.md", SkillGroup::User)
    }
    pub fn guidelines(fs: Arc<dyn FileSystem>, folders: Vec<String>) -> Result<Self, ContentError> {
        Self::new(fs, folders, "guidelines.md", SkillGroup::DesignSystem)
    }
    fn new(
        fs: Arc<dyn FileSystem>,
        folders: Vec<String>,
        leaf: &str,
        group: SkillGroup,
    ) -> Result<Self, ContentError> {
        let mut files = Vec::new();
        for folder in folders {
            path(&folder)?;
            let file = format!("{folder}/{leaf}");
            if files.contains(&file) {
                return Err(ContentError::Invalid(format!("duplicate folder {folder}")));
            }
            files.push(file);
        }
        Ok(Self { fs, files, group })
    }
}

impl SkillSource for FileSkills {
    fn list(&self) -> Result<Vec<SkillMeta>, ContentError> {
        self.files
            .iter()
            .map(|file| {
                let text = read(&*self.fs, file)?;
                let (name, description) = if self.group == SkillGroup::User {
                    metadata(&text)
                        .map_err(|why| ContentError::Invalid(format!("{file}: {why}")))?
                } else {
                    let name = text
                        .lines()
                        .find_map(|l| l.strip_prefix("# "))
                        .unwrap_or(file)
                        .trim()
                        .to_owned();
                    let description = text
                        .lines()
                        .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
                        .unwrap_or("Design-system guidelines.")
                        .trim()
                        .to_owned();
                    (name, description)
                };
                Ok(SkillMeta {
                    path: file.clone(),
                    name,
                    description,
                    group: self.group,
                })
            })
            .collect()
    }
    fn read(&self, file: &str) -> Result<String, ContentError> {
        path(file)?;
        if !self.files.iter().any(|p| p == file) {
            return Err(ContentError::NotFound(file.into()));
        }
        let text = read(&*self.fs, file)?;
        if self.group == SkillGroup::User {
            metadata(&text).map_err(|why| ContentError::Invalid(format!("{file}: {why}")))?;
        }
        Ok(text)
    }
}

/// A deliberately bounded frontmatter subset: name/description as plain, quoted or block scalars. Other
/// fields are ignored, never interpreted as tool permissions. Unsupported scalar syntax is an error.
fn metadata(text: &str) -> Result<(String, String), String> {
    let text = text.replace("\r\n", "\n");
    let Some(rest) = text.strip_prefix("---\n") else {
        return Err("SKILL.md needs name and description frontmatter".into());
    };
    let Some((header, body)) = rest.split_once("\n---\n") else {
        return Err("frontmatter is not closed".into());
    };
    if body.trim().is_empty() {
        return Err("skill body is empty".into());
    }
    let lines: Vec<_> = header.lines().collect();
    let mut name = None;
    let mut description = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') || line.starts_with(char::is_whitespace)
        {
            continue;
        }
        let Some((key, raw)) = line.split_once(':') else {
            return Err("expected frontmatter key: value".into());
        };
        let slot = match key {
            "name" => &mut name,
            "description" => &mut description,
            _ => continue,
        };
        if slot.is_some() {
            return Err(format!("duplicate {key}"));
        }
        let raw = raw.trim();
        let value = if matches!(raw, "|" | ">" | "|-" | ">-") {
            lines[i + 1..]
                .iter()
                .take_while(|l| l.starts_with([' ', '\t']) || l.is_empty())
                .map(|l| l.trim())
                .collect::<Vec<_>>()
                .join(if raw.starts_with('>') { " " } else { "\n" })
        } else if raw.starts_with('"') {
            serde_json::from_str::<String>(raw).map_err(|_| format!("invalid quoted {key}"))?
        } else if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
            raw[1..raw.len() - 1].replace("''", "'")
        } else if raw.starts_with(['[', '{', '&', '*', '!', '|', '>', '\'']) {
            return Err(format!("{key} must be a string"));
        } else {
            raw.into()
        };
        if value.trim().is_empty() || value.len() > 1024 {
            return Err(format!("{key} must contain 1–1024 bytes"));
        }
        *slot = Some(value);
    }
    Ok((
        name.ok_or("missing name")?,
        description.ok_or("missing description")?,
    ))
}
