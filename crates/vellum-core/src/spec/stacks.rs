//! Stack profiles: hints for writing a spec's design in one stack, as data.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

/// How one stack says what a spec says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackProfile {
    /// The name tools use: `shadcn`, `react-native`, `swiftui`.
    pub id: String,
    /// The name a person sees.
    pub label: String,
    /// The language code is written in, such as `tsx`.
    pub language: String,
    /// One or two sentences on what the stack is and how its code is organised.
    pub summary: String,
    /// What each kind of layer becomes: `frame`, `text`, `image`, `icon`, `instance`, …
    pub elements: BTreeMap<String, String>,
    /// What each layout word becomes: `row`, `column`, `grid`, `none`, `gap`, `padding`, `justify`, `align`,
    /// `fill`, `hug`, `fixed`.
    pub layout: BTreeMap<String, String>,
    /// How tokens are used: `reference` (a token in code), `naming` (a token's name as the stack spells it),
    /// `modes` (how theme values are handled).
    pub tokens: BTreeMap<String, String>,
    /// How component instances become the stack's components.
    pub components: String,
    /// Rules to follow, in order of importance.
    pub rules: Vec<String>,
}

fn text(object: &Map<String, Value>, key: &str) -> Result<String, String> {
    match object.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.clone()),
        Some(_) => Err(format!("`{key}` must be a non-empty string.")),
        None => Err(format!("`{key}` is missing.")),
    }
}

fn table(object: &Map<String, Value>, key: &str) -> Result<BTreeMap<String, String>, String> {
    let Some(Value::Object(entries)) = object.get(key) else {
        return Err(format!("`{key}` is missing or is not an object."));
    };
    entries
        .iter()
        .map(|(name, value)| match value {
            Value::String(s) if !s.trim().is_empty() => Ok((name.clone(), s.clone())),
            _ => Err(format!("`{key}.{name}` must be a non-empty string.")),
        })
        .collect()
}

impl StackProfile {
    /// A profile from its JSON, or what is wrong with it.
    pub fn from_json(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("a stack profile is a JSON object.")?;
        let rules = match object.get("rules") {
            Some(Value::Array(items)) if !items.is_empty() => items
                .iter()
                .map(|r| match r {
                    Value::String(s) if !s.trim().is_empty() => Ok(s.clone()),
                    _ => Err("`rules` holds only non-empty strings.".to_owned()),
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err("`rules` must be a non-empty list of strings.".to_owned()),
            None => return Err("`rules` is missing.".to_owned()),
        };
        Ok(Self {
            id: text(object, "id")?,
            label: text(object, "label")?,
            language: text(object, "language")?,
            summary: text(object, "summary")?,
            elements: table(object, "elements")?,
            layout: table(object, "layout")?,
            tokens: table(object, "tokens")?,
            components: text(object, "components")?,
            rules,
        })
    }

    /// The profile as JSON, in the shape [`StackProfile::from_json`] reads.
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "language": self.language,
            "summary": self.summary,
            "elements": self.elements,
            "layout": self.layout,
            "tokens": self.tokens,
            "components": self.components,
            "rules": self.rules,
        })
    }
}

const BUILTIN: [&str; 3] = [
    include_str!("stacks/shadcn.json"),
    include_str!("stacks/react-native.json"),
    include_str!("stacks/swiftui.json"),
];

/// The stacks `get_spec` knows, in the order a person sees them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackProfiles {
    profiles: Vec<StackProfile>,
}

impl StackProfiles {
    /// shadcn, React Native and SwiftUI.
    ///
    /// # Panics
    /// If a shipped profile is invalid, which a test rules out.
    pub fn builtin() -> Self {
        let profiles = BUILTIN
            .iter()
            .map(|source| {
                let value: Value = serde_json::from_str(source).expect("a shipped profile is JSON");
                StackProfile::from_json(&value).expect("a shipped profile is complete")
            })
            .collect();
        Self { profiles }
    }

    /// These stacks and `profile`: it replaces the one with its id, or goes last.
    #[must_use]
    pub fn with(mut self, profile: StackProfile) -> Self {
        match self.profiles.iter_mut().find(|p| p.id == profile.id) {
            Some(existing) => *existing = profile,
            None => self.profiles.push(profile),
        }
        self
    }

    /// The stacks, in order.
    pub fn iter(&self) -> impl Iterator<Item = &StackProfile> {
        self.profiles.iter()
    }

    /// The stack called `id`.
    pub fn get(&self, id: &str) -> Option<&StackProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// The stack called `id`, or a sentence naming the ones there are.
    pub fn require(&self, id: &str) -> Result<&StackProfile, String> {
        self.get(id).ok_or_else(|| {
            format!(
                "there is no stack called {id:?}. The stacks are: {}.",
                self.iter()
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }
}
