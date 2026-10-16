//! Prompt packs: the built-in engine's instructions are versioned files (ARCHITECTURE §3.4), so they can be tuned
//! without a release. The shipped pack is `packages/prompt-packs/packs/builtin.md`; a host may load another.
//!
//! A pack is a header (`id`, `version`) between `---` lines, then three sections: `# system` (which must hold the
//! `{{skills}}` slot), `# self-check` and `# summarize`.
use std::fmt;
use vellum_core::ports::SkillMeta;

const BUILTIN: &str = include_str!("../../../../packages/prompt-packs/packs/builtin.md");
const SKILLS_SLOT: &str = "{{skills}}";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptPack {
    id: String,
    version: u32,
    system: String,
    self_check: String,
    summarize: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackError(String);
impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "prompt pack: {}", self.0)
    }
}
impl std::error::Error for PackError {}

impl PromptPack {
    /// The pack Vellum ships.
    pub fn builtin() -> Self {
        Self::parse(BUILTIN).expect("the shipped prompt pack parses")
    }

    pub fn parse(text: &str) -> Result<Self, PackError> {
        let error = |message: &str| PackError(message.to_owned());
        let rest = text
            .strip_prefix("---\n")
            .ok_or_else(|| error("starts with a --- header"))?;
        let (header, body) = rest
            .split_once("\n---\n")
            .ok_or_else(|| error("the header ends with ---"))?;
        let (mut id, mut version) = (None, None);
        for line in header.lines() {
            match line.split_once(':').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("id", value)) if !value.is_empty() => id = Some(value.to_owned()),
                Some(("version", value)) => {
                    version = Some(value.parse().map_err(|_| error("version is a number"))?);
                }
                _ => return Err(error("the header holds id and version only")),
            }
        }
        let mut sections: Vec<(&str, String)> = vec![];
        for line in body.lines() {
            if let Some(name) = line.strip_prefix("# ") {
                sections.push((name.trim(), String::new()));
            } else if let Some((_, text)) = sections.last_mut() {
                text.push_str(line);
                text.push('\n');
            } else if !line.trim().is_empty() {
                return Err(error("text before the first section"));
            }
        }
        let mut take = |name: &str| {
            let position = sections.iter().position(|(n, _)| *n == name);
            position
                .map(|at| sections.remove(at).1.trim().to_owned())
                .filter(|text| !text.is_empty())
                .ok_or_else(|| PackError(format!("needs a non-empty # {name} section")))
        };
        let pack = Self {
            id: id.ok_or_else(|| error("needs an id"))?,
            version: version.ok_or_else(|| error("needs a version"))?,
            system: take("system")?,
            self_check: take("self-check")?,
            summarize: take("summarize")?,
        };
        if !sections.is_empty() {
            return Err(error("has a section it does not know"));
        }
        if pack.system.matches(SKILLS_SLOT).count() != 1 {
            return Err(error("the system section holds {{skills}} once"));
        }
        Ok(pack)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    /// The system prompt's text before the skills list.
    pub fn system_prefix(&self) -> &str {
        self.system.split(SKILLS_SLOT).next().unwrap_or_default()
    }

    /// The system prompt with the skills the agent can read listed in place.
    pub fn system(&self, skills: &[SkillMeta]) -> String {
        let list = if skills.is_empty() {
            "(none)".to_owned()
        } else {
            skills
                .iter()
                .map(|s| format!("- {} ({}): {}", s.path, s.name, s.description))
                .collect::<Vec<_>>()
                .join("\n")
        };
        self.system.replacen(SKILLS_SLOT, &list, 1)
    }

    /// What the engine says when a model finishes after writing without looking at the result.
    pub fn self_check(&self) -> &str {
        &self.self_check
    }

    /// The system prompt of a summary request.
    pub fn summarize(&self) -> &str {
        &self.summarize
    }
}
