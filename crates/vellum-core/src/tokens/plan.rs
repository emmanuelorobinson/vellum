//! Comparing a [`TokenSet`] with a document: the diff a person previews (DESIGN.md C20), and the variables and
//! themes that make the document match the set.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use vellum_types::{
    AliasEntry, Document, LiteralEntry, Variable, VariableBody, VariableId, VariableType,
    VariableValue,
};

use super::{ModeValue, Token, TokenSet, TokenValue, collect};

/// One token that differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The token's name.
    pub name: String,
    /// What differs: `type`, `value`, or a theme value such as `Mode: Dark`.
    pub what: Vec<String>,
}

/// Added, changed and removed tokens, by name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TokenDiff {
    /// In the file, not in the document.
    pub added: Vec<String>,
    /// In both, different.
    pub changed: Vec<Change>,
    /// In the document, not in the file. Only removed when the caller asks.
    pub removed: Vec<String>,
    /// In both, the same.
    pub unchanged: Vec<String>,
}

impl TokenDiff {
    /// Whether applying it would change nothing but possibly removals.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }
}

/// What importing a set would do.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportPlan {
    /// The comparison.
    pub diff: TokenDiff,
    /// The document's theme axes after the import, when they change (a file may name a mode the document lacks).
    pub themes: Option<BTreeMap<String, Vec<String>>>,
    /// The variables to add or replace: the added and changed tokens.
    pub set: BTreeMap<VariableId, Variable>,
    /// The variables to remove: those the file lacks, when removal was asked for.
    pub remove: BTreeSet<VariableId>,
    /// The id of each token in the set, by name.
    pub ids: BTreeMap<String, VariableId>,
}

/// Why a set cannot become variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError {
    /// The token at fault, when one is.
    pub token: Option<String>,
    /// What is wrong, in a sentence.
    pub message: String,
}

impl PlanError {
    fn token(name: &str, message: impl Into<String>) -> Self {
        Self {
            token: Some(name.to_owned()),
            message: message.into(),
        }
    }
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.token {
            Some(token) => write!(f, "{token}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for PlanError {}

fn mode_label(mode: &ModeValue) -> String {
    format!("{}: {}", mode.axis, mode.mode)
}

fn check_value(
    token: &Token,
    value: &TokenValue,
    at: &str,
    kinds: &BTreeMap<&str, VariableType>,
) -> Result<(), PlanError> {
    match value {
        TokenValue::Alias(target) => match kinds.get(target.as_str()) {
            None => Err(PlanError::token(
                &token.name,
                format!(
                    "{at} refers to {target}, which is not a token in the file or the document."
                ),
            )),
            Some(kind) if *kind != token.kind => Err(PlanError::token(
                &token.name,
                format!(
                    "{at} refers to {target}, a {} token; this one is a {}.",
                    kind.as_str(),
                    token.kind.as_str()
                ),
            )),
            Some(_) => Ok(()),
        },
        literal if literal.kind() != Some(token.kind) => Err(PlanError::token(
            &token.name,
            format!("{at} is not a {}.", token.kind.as_str()),
        )),
        _ => Ok(()),
    }
}

fn body<T>(
    name: &str,
    entries: &[(&TokenValue, BTreeMap<String, String>)],
    pick: impl Fn(&TokenValue) -> Option<T>,
    id_of: &dyn Fn(&str) -> Option<VariableId>,
) -> VariableBody<T> {
    VariableBody {
        name: name.to_owned(),
        values: entries
            .iter()
            .filter_map(|(value, when)| match value {
                TokenValue::Alias(target) => Some(VariableValue::Alias(AliasEntry {
                    alias: id_of(target)?,
                    when: when.clone(),
                })),
                literal => Some(VariableValue::Literal(LiteralEntry {
                    value: pick(literal)?,
                    when: when.clone(),
                })),
            })
            .collect(),
    }
}

fn variable(
    token: &Token,
    themes: &BTreeMap<String, Vec<String>>,
    id_of: &dyn Fn(&str) -> Option<VariableId>,
) -> Variable {
    let position = |mode: &ModeValue| {
        themes
            .get(&mode.axis)
            .and_then(|values| values.iter().position(|v| *v == mode.mode))
            .unwrap_or(usize::MAX)
    };
    let mut modes: Vec<&ModeValue> = token.modes.iter().collect();
    modes.sort_by_key(|mode| (mode.axis.clone(), position(mode)));
    let mut entries = vec![(&token.value, BTreeMap::new())];
    entries.extend(modes.into_iter().map(|mode| {
        (
            &mode.value,
            BTreeMap::from([(mode.axis.clone(), mode.mode.clone())]),
        )
    }));
    let name = &token.name;
    match token.kind {
        VariableType::Color => Variable::Color(body(
            name,
            &entries,
            |v| {
                if let TokenValue::Color(c) = v {
                    Some(*c)
                } else {
                    None
                }
            },
            id_of,
        )),
        VariableType::Number => Variable::Number(body(
            name,
            &entries,
            |v| {
                if let TokenValue::Number(n) = v {
                    Some(*n)
                } else {
                    None
                }
            },
            id_of,
        )),
        VariableType::String => Variable::String(body(
            name,
            &entries,
            |v| {
                if let TokenValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            },
            id_of,
        )),
        VariableType::Boolean => Variable::Boolean(body(
            name,
            &entries,
            |v| {
                if let TokenValue::Boolean(b) = v {
                    Some(*b)
                } else {
                    None
                }
            },
            id_of,
        )),
    }
}

/// What differs between two tokens of the same name.
fn differences(old: &Token, new: &Token) -> Vec<String> {
    let mut what = Vec::new();
    if old.kind != new.kind {
        what.push("type".to_owned());
    }
    if old.value != new.value {
        what.push("value".to_owned());
    }
    let key = |m: &ModeValue| (m.axis.clone(), m.mode.clone());
    let old_modes: BTreeMap<_, _> = old.modes.iter().map(|m| (key(m), m)).collect();
    let new_modes: BTreeMap<_, _> = new.modes.iter().map(|m| (key(m), m)).collect();
    let keys: BTreeSet<_> = old_modes.keys().chain(new_modes.keys()).collect();
    for k in keys {
        let (a, b) = (old_modes.get(k), new_modes.get(k));
        if a.map(|m| &m.value) != b.map(|m| &m.value) {
            what.push(mode_label(a.or(b).expect("one side has it")));
        }
    }
    what
}

/// Compares `incoming` with `doc`. `new_id` makes the id of each token the document lacks; a token the document
/// has keeps its id. With `remove_missing`, variables the set lacks are listed in `remove` as well as in the diff.
pub fn plan(
    doc: &Document,
    incoming: &TokenSet,
    remove_missing: bool,
    new_id: &mut dyn FnMut() -> VariableId,
) -> Result<ImportPlan, PlanError> {
    // Names must be usable, and each used once.
    let mut names = BTreeSet::new();
    for token in &incoming.tokens {
        if token.name.is_empty()
            || token.name != token.name.trim()
            || token.name.chars().any(char::is_control)
        {
            return Err(PlanError::token(
                &token.name,
                "a token name cannot be empty, start or end with a space, or hold control characters.",
            ));
        }
        if !names.insert(token.name.as_str()) {
            return Err(PlanError::token(&token.name, "the file defines it twice."));
        }
    }

    // The ids of the document's variables by name; the later of two with one name, as `collect` uses.
    let mut existing: BTreeMap<&str, (&VariableId, &Variable)> = BTreeMap::new();
    for (id, variable) in &doc.variables {
        existing.insert(variable.name(), (id, variable));
    }
    let removed: Vec<String> = existing
        .keys()
        .filter(|name| !names.contains(*name))
        .map(|name| (*name).to_owned())
        .collect();

    // Names an alias can point to, and their types.
    let mut kinds: BTreeMap<&str, VariableType> = BTreeMap::new();
    for (name, (_, variable)) in &existing {
        if !(remove_missing && !names.contains(name)) {
            kinds.insert(name, variable.variable_type());
        }
    }
    for token in &incoming.tokens {
        kinds.insert(&token.name, token.kind);
    }

    // Themes: the axes and values the file needs.
    let mut themes = doc.themes.clone();
    for token in &incoming.tokens {
        let mut seen = BTreeSet::new();
        for mode in &token.modes {
            if !seen.insert((&mode.axis, &mode.mode)) {
                return Err(PlanError::token(
                    &token.name,
                    format!("it has two values for {}.", mode_label(mode)),
                ));
            }
            if !themes.contains_key(&mode.axis) {
                match incoming.themes.get(&mode.axis) {
                    Some(values) if !values.is_empty() => {
                        themes.insert(mode.axis.clone(), values.clone());
                    }
                    _ => {
                        return Err(PlanError::token(
                            &token.name,
                            format!(
                                "the document has no theme axis {:?} and the file does not say what its default is.",
                                mode.axis
                            ),
                        ));
                    }
                }
            }
            let values = themes.get_mut(&mode.axis).expect("just ensured");
            if values.first() == Some(&mode.mode) {
                return Err(PlanError::token(
                    &token.name,
                    format!(
                        "{} is the default of {:?}; its value belongs in the token's own value.",
                        mode.mode, mode.axis
                    ),
                ));
            }
            if !values.contains(&mode.mode) {
                values.push(mode.mode.clone());
            }
        }
    }

    // Values are of the token's type, and aliases lead somewhere of the same type.
    for token in &incoming.tokens {
        check_value(token, &token.value, "its value", &kinds)?;
        for mode in &token.modes {
            check_value(token, &mode.value, &mode_label(mode), &kinds)?;
        }
    }
    reject_cycles(incoming)?;

    // Ids, then the variables.
    let mut ids: BTreeMap<String, VariableId> = BTreeMap::new();
    for token in &incoming.tokens {
        let id = match existing.get(token.name.as_str()) {
            Some((id, _)) => (*id).clone(),
            None => new_id(),
        };
        ids.insert(token.name.clone(), id);
    }
    let id_of = |name: &str| -> Option<VariableId> {
        ids.get(name)
            .cloned()
            .or_else(|| existing.get(name).map(|(id, _)| (*id).clone()))
    };

    let (current, _) = collect(doc);
    let mut diff = TokenDiff {
        removed,
        ..TokenDiff::default()
    };
    let mut set = BTreeMap::new();
    for token in &incoming.tokens {
        let id = ids[&token.name].clone();
        if existing.contains_key(token.name.as_str()) {
            let what = match current.get(&token.name) {
                Some(old) => differences(old, token),
                None => vec!["value".to_owned()],
            };
            if what.is_empty() {
                diff.unchanged.push(token.name.clone());
                continue;
            }
            diff.changed.push(Change {
                name: token.name.clone(),
                what,
            });
        } else {
            diff.added.push(token.name.clone());
        }
        set.insert(id, variable(token, &themes, &id_of));
    }
    let remove = if remove_missing {
        diff.removed
            .iter()
            .filter_map(|name| existing.get(name.as_str()).map(|(id, _)| (*id).clone()))
            .collect()
    } else {
        BTreeSet::new()
    };
    Ok(ImportPlan {
        diff,
        themes: (themes != doc.themes).then_some(themes),
        set,
        remove,
        ids,
    })
}

/// Aliases among the file's own tokens must not lead back to themselves, under any theme value.
fn reject_cycles(incoming: &TokenSet) -> Result<(), PlanError> {
    let edges: BTreeMap<&str, BTreeSet<&str>> = incoming
        .tokens
        .iter()
        .map(|token| {
            let targets = std::iter::once(&token.value)
                .chain(token.modes.iter().map(|m| &m.value))
                .filter_map(|value| match value {
                    TokenValue::Alias(target) => Some(target.as_str()),
                    _ => None,
                })
                .collect();
            (token.name.as_str(), targets)
        })
        .collect();
    fn visit<'a>(
        at: &'a str,
        edges: &BTreeMap<&'a str, BTreeSet<&'a str>>,
        path: &mut Vec<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Option<Vec<&'a str>> {
        if let Some(start) = path.iter().position(|n| *n == at) {
            let mut cycle = path[start..].to_vec();
            cycle.push(at);
            return Some(cycle);
        }
        if done.contains(at) {
            return None;
        }
        path.push(at);
        for next in edges.get(at).into_iter().flatten() {
            if let Some(cycle) = visit(next, edges, path, done) {
                return Some(cycle);
            }
        }
        path.pop();
        done.insert(at);
        None
    }
    let mut done = BTreeSet::new();
    for name in edges.keys() {
        if let Some(cycle) = visit(name, &edges, &mut Vec::new(), &mut done) {
            return Err(PlanError::token(
                cycle[0],
                format!("its aliases lead back to it ({}).", cycle.join(" → ")),
            ));
        }
    }
    Ok(())
}
