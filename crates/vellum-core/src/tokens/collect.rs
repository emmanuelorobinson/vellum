//! A document's variables as a [`TokenSet`].

use std::collections::BTreeMap;

use vellum_types::{Document, Variable, VariableBody, VariableId, VariableValue};

use super::{ModeValue, Token, TokenSet, TokenValue, TokenWarning};
use crate::resolver::variables::Theme;

type Entries<'a> = Vec<(TokenValue, &'a BTreeMap<String, String>)>;

/// The entries of one variable body in order, aliases by name. `Err` names an alias whose target is gone.
fn entries_of<'a, T: Clone>(
    body: &'a VariableBody<T>,
    wrap: fn(T) -> TokenValue,
    doc: &Document,
) -> Result<Entries<'a>, VariableId> {
    body.values
        .iter()
        .map(|entry| match entry {
            VariableValue::Literal(literal) => Ok((wrap(literal.value.clone()), &literal.when)),
            VariableValue::Alias(alias) => doc
                .variables
                .get(&alias.alias)
                .map(|target| (TokenValue::Alias(target.name().to_owned()), &alias.when))
                .ok_or_else(|| alias.alias.clone()),
        })
        .collect()
}

fn entries<'a>(variable: &'a Variable, doc: &Document) -> Result<Entries<'a>, VariableId> {
    match variable {
        Variable::Color(body) => entries_of(body, TokenValue::Color, doc),
        Variable::Number(body) => entries_of(body, TokenValue::Number, doc),
        Variable::String(body) => entries_of(body, TokenValue::String, doc),
        Variable::Boolean(body) => entries_of(body, TokenValue::Boolean, doc),
    }
}

/// The entry that applies under `theme`: the last one whose condition the theme satisfies.
fn select<'a>(entries: &'a Entries<'_>, theme: &Theme) -> Option<&'a TokenValue> {
    entries
        .iter()
        .rev()
        .find(|(_, when)| theme.matches(when))
        .map(|(value, _)| value)
}

/// The document's variables as tokens, by name, with a warning for each thing that could not be kept: a variable
/// with no value by default, an alias to a variable that is gone, a name used twice, a value that depends on
/// several theme axes at once.
pub fn collect(doc: &Document) -> (TokenSet, Vec<TokenWarning>) {
    let mut warnings = Vec::new();
    let defaults = Theme::defaults(doc);
    let mut tokens: BTreeMap<String, Token> = BTreeMap::new();
    for variable in doc.variables.values() {
        let name = variable.name().to_owned();
        let entries = match entries(variable, doc) {
            Ok(entries) => entries,
            Err(missing) => {
                warnings.push(TokenWarning::token(
                    &name,
                    format!("it refers to {missing}, which is not a variable; left out."),
                ));
                continue;
            }
        };
        let Some(value) = select(&entries, &defaults).cloned() else {
            warnings.push(TokenWarning::token(
                &name,
                "it has no value under the default theme; left out.",
            ));
            continue;
        };
        let mut modes = Vec::new();
        for (axis, values) in &doc.themes {
            for mode in values.iter().skip(1) {
                let choice = BTreeMap::from([(axis.clone(), mode.clone())]);
                let Ok(theme) = defaults.with(doc, &choice) else {
                    continue;
                };
                if let Some(found) = select(&entries, &theme)
                    && *found != value
                {
                    modes.push(ModeValue {
                        axis: axis.clone(),
                        mode: mode.clone(),
                        value: found.clone(),
                    });
                }
            }
        }
        if entries.iter().any(|(_, when)| {
            when.len() > 1
                || when
                    .iter()
                    .any(|(axis, mode)| doc.themes.get(axis).is_none_or(|v| !v.contains(mode)))
        }) {
            warnings.push(TokenWarning::token(
                &name,
                "a value depends on more than one theme axis, or on one the document does not have; \
                 only the value under each single axis is written.",
            ));
        }
        let token = Token {
            name: name.clone(),
            kind: variable.variable_type(),
            value,
            modes,
        };
        if tokens.insert(name.clone(), token).is_some() {
            warnings.push(TokenWarning::token(
                &name,
                "two variables have this name; the later one is used.",
            ));
        }
    }
    (
        TokenSet {
            themes: doc.themes.clone(),
            tokens: tokens.into_values().collect(),
        },
        warnings,
    )
}
