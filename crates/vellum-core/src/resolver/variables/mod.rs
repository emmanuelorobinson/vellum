//! Resolver I (Prompt 16): variables, theme axes and aliases.
//!
//! A document says how its variables vary (`themes`: axes with ordered values, the first being the default;
//! each variable's `values`: entries with an optional `when`). A frame can pick a value on an axis for its
//! subtree (`theme`). The resolver turns that into one answer per place:
//!
//! - [`Theme`] is the choice of one value on every axis at some point in the tree. It starts at each axis's
//!   first value, and a frame's `theme` replaces the axes it names (and only those) for itself and below.
//! - [`resolve_variable`] picks a variable's value under a theme. The **last** entry whose `when` the theme
//!   satisfies wins, whatever its position in the file or its specificity; an entry without `when` always
//!   satisfies it. An alias entry hands the question to the other variable under the same theme.
//! - [`resolve_value`] does the same for a property that holds a literal or a `$var`, and checks the type
//!   the property takes. [`check_node_bindings`] and friends are the same check made at bind time, without
//!   a theme, which is how the op layer refuses a variable of the wrong type.
//! - [`Resolver`] resolves a tree into [`ResolvedNode`]s, each holding its node with every reference
//!   replaced by its value, and caches the result per subtree so an edit recomputes only what it reaches.
//!
//! Aliases are checked when they are set, but a file read from disk, a merge or an older version can hold one
//! that the op layer would have refused, so resolution reports a cycle, a missing target or a type clash as a
//! typed error rather than assuming the document is valid. A cycle is only an error when the theme selects
//! it: a variable whose alias entry applies in Dark only is fine in Light.

mod bind;
mod tree;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use vellum_types::{
    Color, Document, Node, Scalar, Uniform, Value, Variable, VariableBody, VariableId,
    VariableType, VariableValue,
};

pub use bind::{
    BindingProblem, BindingProblemKind, check_component_bindings, check_document_bindings,
    check_node_bindings, check_own_bindings, check_style_bindings, check_subtree_bindings,
    check_variants_bindings, first_misuse_of,
};
pub use tree::{Problem, ResolveError, ResolvedNode, Resolver};

#[cfg(test)]
mod tests;

/// One value chosen on every theme axis: `{"Mode": "Dark", "Device": "Phone"}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Theme(BTreeMap<String, String>);

/// Why a theme choice is not valid for a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeError {
    /// The document has no axis with this name.
    UnknownAxis {
        /// The axis named.
        axis: String,
    },
    /// The axis has no such value.
    UnknownValue {
        /// The axis.
        axis: String,
        /// The value named.
        value: String,
    },
}

impl ThemeError {
    /// The axis the error is about.
    pub fn axis(&self) -> &str {
        match self {
            Self::UnknownAxis { axis } | Self::UnknownValue { axis, .. } => axis,
        }
    }
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAxis { axis } => write!(f, "there is no theme axis {axis:?}."),
            Self::UnknownValue { axis, value } => {
                write!(f, "the theme axis {axis:?} has no value {value:?}.")
            }
        }
    }
}

impl std::error::Error for ThemeError {}

impl Theme {
    /// The document's default theme: the first value of every axis. An axis with no values has no entry.
    pub fn defaults(doc: &Document) -> Self {
        Self(
            doc.themes
                .iter()
                .filter_map(|(axis, values)| Some((axis.clone(), values.first()?.clone())))
                .collect(),
        )
    }

    /// The value chosen on `axis`.
    pub fn get(&self, axis: &str) -> Option<&str> {
        self.0.get(axis).map(String::as_str)
    }

    /// The choices, by axis.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(axis, value)| (axis.as_str(), value.as_str()))
    }

    /// This theme with `choices` applied: each axis they name takes the value given, the others stay. Fails
    /// on the first axis or value the document does not have.
    pub fn with(
        &self,
        doc: &Document,
        choices: &BTreeMap<String, String>,
    ) -> Result<Self, ThemeError> {
        let mut errors = Vec::new();
        let next = self.with_lenient(doc, choices, &mut errors);
        match errors.into_iter().next() {
            Some(error) => Err(error),
            None => Ok(next),
        }
    }

    /// As [`Theme::with`], applying every valid choice and collecting the others in `errors`.
    pub(crate) fn with_lenient(
        &self,
        doc: &Document,
        choices: &BTreeMap<String, String>,
        errors: &mut Vec<ThemeError>,
    ) -> Self {
        let mut next = self.clone();
        for (axis, value) in choices {
            match doc.themes.get(axis) {
                None => errors.push(ThemeError::UnknownAxis { axis: axis.clone() }),
                Some(values) if !values.contains(value) => errors.push(ThemeError::UnknownValue {
                    axis: axis.clone(),
                    value: value.clone(),
                }),
                Some(_) => {
                    next.0.insert(axis.clone(), value.clone());
                }
            }
        }
        next
    }

    /// Whether every axis in `when` has the value it names here. An axis this theme lacks never matches.
    pub fn matches(&self, when: &BTreeMap<String, String>) -> bool {
        when.iter()
            .all(|(axis, value)| self.get(axis) == Some(value.as_str()))
    }
}

/// The value of a variable under a theme.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedValue {
    /// A colour variable's value.
    Color(Color),
    /// A number variable's value.
    Number(f64),
    /// A string variable's value.
    String(String),
    /// A boolean variable's value.
    Boolean(bool),
}

impl ResolvedValue {
    /// The type of variable this came from.
    pub fn variable_type(&self) -> VariableType {
        match self {
            Self::Color(_) => VariableType::Color,
            Self::Number(_) => VariableType::Number,
            Self::String(_) => VariableType::String,
            Self::Boolean(_) => VariableType::Boolean,
        }
    }
}

/// Why a variable could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableError {
    /// The document has no variable with this id.
    Unknown(VariableId),
    /// No entry of the variable applies under the theme.
    NoValue {
        /// The variable.
        variable: VariableId,
        /// The theme that selected nothing.
        theme: Theme,
    },
    /// An alias entry names a variable of another type.
    AliasType {
        /// The variable holding the alias.
        variable: VariableId,
        /// The variable it names.
        alias: VariableId,
        /// The type of the variable holding the alias.
        expected: VariableType,
        /// The type of the variable it names.
        found: VariableType,
    },
    /// Aliases lead back to a variable already on the way. The chain runs from its first appearance to its
    /// second, so the first and last entries are the same variable.
    Cycle(Vec<VariableId>),
    /// A property was bound to a variable of a type it does not take.
    TypeMismatch {
        /// The variable.
        variable: VariableId,
        /// The types the property takes.
        expected: &'static [VariableType],
        /// The variable's type.
        found: VariableType,
    },
}

pub(super) fn type_list(types: &[VariableType]) -> String {
    let names: Vec<&str> = types.iter().map(|t| t.as_str()).collect();
    match names.as_slice() {
        [] => "no variable".into(),
        [one] => format!("a {one} variable"),
        [init @ .., last] => format!("a {} or {last} variable", init.join(", ")),
    }
}

impl fmt::Display for VariableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "there is no variable {id}."),
            Self::NoValue { variable, theme } => {
                let choices: Vec<String> = theme
                    .iter()
                    .map(|(axis, value)| format!("{axis}: {value}"))
                    .collect();
                write!(
                    f,
                    "variable {variable} has no value for {}.",
                    if choices.is_empty() {
                        "the default theme".into()
                    } else {
                        choices.join(", ")
                    }
                )
            }
            Self::AliasType {
                variable,
                alias,
                expected,
                found,
            } => write!(
                f,
                "variable {variable} is a {} variable but aliases {alias}, a {} one.",
                expected.as_str(),
                found.as_str()
            ),
            Self::Cycle(chain) => {
                let ids: Vec<&str> = chain.iter().map(VariableId::as_str).collect();
                write!(
                    f,
                    "variables alias each other in a loop: {}.",
                    ids.join(" → ")
                )
            }
            Self::TypeMismatch {
                variable,
                expected,
                found,
            } => write!(
                f,
                "variable {variable} is a {} variable; this takes {}.",
                found.as_str(),
                type_list(expected)
            ),
        }
    }
}

impl std::error::Error for VariableError {}

/// A literal type a property can hold, and the variable types that can stand for it.
pub trait Bindable: Sized {
    /// The types of variable this property takes.
    const ACCEPTS: &'static [VariableType];

    /// The value, if `resolved` is a kind this type takes.
    fn from_resolved(resolved: &ResolvedValue) -> Option<Self>;
}

impl Bindable for f64 {
    const ACCEPTS: &'static [VariableType] = &[VariableType::Number];
    fn from_resolved(resolved: &ResolvedValue) -> Option<Self> {
        match resolved {
            ResolvedValue::Number(n) => Some(*n),
            _ => None,
        }
    }
}

impl Bindable for bool {
    const ACCEPTS: &'static [VariableType] = &[VariableType::Boolean];
    fn from_resolved(resolved: &ResolvedValue) -> Option<Self> {
        match resolved {
            ResolvedValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

impl Bindable for Color {
    const ACCEPTS: &'static [VariableType] = &[VariableType::Color];
    fn from_resolved(resolved: &ResolvedValue) -> Option<Self> {
        match resolved {
            ResolvedValue::Color(c) => Some(*c),
            _ => None,
        }
    }
}

impl Bindable for String {
    const ACCEPTS: &'static [VariableType] = &[VariableType::String];
    fn from_resolved(resolved: &ResolvedValue) -> Option<Self> {
        match resolved {
            ResolvedValue::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl Bindable for Scalar {
    /// A script input takes any variable. A colour arrives as the text a literal colour would be written as
    /// (`#RRGGBB`), because a `Scalar` is a boolean, a number or text and a script reads a colour as text.
    const ACCEPTS: &'static [VariableType] = &[
        VariableType::Boolean,
        VariableType::Number,
        VariableType::String,
        VariableType::Color,
    ];
    fn from_resolved(resolved: &ResolvedValue) -> Option<Self> {
        Some(match resolved {
            ResolvedValue::Boolean(b) => Self::Bool(*b),
            ResolvedValue::Number(n) => Self::Number(*n),
            ResolvedValue::String(s) => Self::Text(s.clone()),
            ResolvedValue::Color(c) => Self::Text(c.to_string()),
        })
    }
}

impl Bindable for Uniform {
    const ACCEPTS: &'static [VariableType] = &[
        VariableType::Boolean,
        VariableType::Number,
        VariableType::Color,
    ];
    fn from_resolved(resolved: &ResolvedValue) -> Option<Self> {
        match resolved {
            ResolvedValue::Boolean(b) => Some(Self::Bool(*b)),
            ResolvedValue::Number(n) => Some(Self::Number(*n)),
            ResolvedValue::Color(c) => Some(Self::Color(*c)),
            ResolvedValue::String(_) => None,
        }
    }
}

/// What picking an entry of a variable under a theme gives.
enum Picked<'a> {
    /// A literal.
    Value(ResolvedValue),
    /// An alias to follow.
    Alias(&'a VariableId),
    /// No entry applies.
    Nothing,
}

fn pick_entry<'a, T: Clone>(
    body: &'a VariableBody<T>,
    theme: &Theme,
    wrap: fn(T) -> ResolvedValue,
) -> Picked<'a> {
    let applies = |entry: &&VariableValue<T>| match entry {
        VariableValue::Literal(l) => theme.matches(&l.when),
        VariableValue::Alias(a) => theme.matches(&a.when),
    };
    match body.values.iter().rev().find(applies) {
        Some(VariableValue::Literal(l)) => Picked::Value(wrap(l.value.clone())),
        Some(VariableValue::Alias(a)) => Picked::Alias(&a.alias),
        None => Picked::Nothing,
    }
}

fn pick<'a>(variable: &'a Variable, theme: &Theme) -> Picked<'a> {
    match variable {
        Variable::Color(b) => pick_entry(b, theme, ResolvedValue::Color),
        Variable::Number(b) => pick_entry(b, theme, ResolvedValue::Number),
        Variable::String(b) => pick_entry(b, theme, ResolvedValue::String),
        Variable::Boolean(b) => pick_entry(b, theme, ResolvedValue::Boolean),
    }
}

/// The value of variable `id` under `theme`: the last entry that applies, following aliases.
pub fn resolve_variable(
    doc: &Document,
    id: &VariableId,
    theme: &Theme,
) -> Result<ResolvedValue, VariableError> {
    resolve_tracked(doc, id, theme, &mut BTreeSet::new())
}

/// As [`resolve_variable`], adding to `read` every variable the answer depended on: the variable itself, each
/// alias target on the chosen path, and the variable that was missing if one was. When one of them changes,
/// the answer may too; when anything else does, it cannot.
pub(crate) fn resolve_tracked(
    doc: &Document,
    id: &VariableId,
    theme: &Theme,
    read: &mut BTreeSet<VariableId>,
) -> Result<ResolvedValue, VariableError> {
    let mut chain: Vec<&VariableId> = Vec::new();
    let mut current = id;
    loop {
        read.insert(current.clone());
        if let Some(at) = chain.iter().position(|seen| *seen == current) {
            let mut cycle: Vec<VariableId> = chain[at..].iter().map(|v| (*v).clone()).collect();
            cycle.push(current.clone());
            return Err(VariableError::Cycle(cycle));
        }
        let variable = doc
            .variables
            .get(current)
            .ok_or_else(|| VariableError::Unknown(current.clone()))?;
        chain.push(current);
        match pick(variable, theme) {
            Picked::Value(value) => return Ok(value),
            Picked::Nothing => {
                return Err(VariableError::NoValue {
                    variable: current.clone(),
                    theme: theme.clone(),
                });
            }
            Picked::Alias(target) => {
                if let Some(found) = doc.variables.get(target).map(Variable::variable_type)
                    && found != variable.variable_type()
                {
                    return Err(VariableError::AliasType {
                        variable: current.clone(),
                        alias: target.clone(),
                        expected: variable.variable_type(),
                        found,
                    });
                }
                current = target;
            }
        }
    }
}

/// The literal a property holds under `theme`: itself, or the value of the variable it refers to. A reference
/// to a variable of a type the property does not take is a [`VariableError::TypeMismatch`], reported before
/// the variable is resolved.
pub fn resolve_value<T: Bindable + Clone>(
    doc: &Document,
    value: &Value<T>,
    theme: &Theme,
) -> Result<T, VariableError> {
    resolve_value_tracked(doc, value, theme, &mut BTreeSet::new())
}

pub(crate) fn resolve_value_tracked<T: Bindable + Clone>(
    doc: &Document,
    value: &Value<T>,
    theme: &Theme,
    read: &mut BTreeSet<VariableId>,
) -> Result<T, VariableError> {
    let reference = match value {
        Value::Literal(literal) => return Ok(literal.clone()),
        Value::Var(reference) => reference,
    };
    let id = reference.id();
    read.insert(id.clone());
    let declared = doc
        .variables
        .get(id)
        .ok_or_else(|| VariableError::Unknown(id.clone()))?
        .variable_type();
    if !T::ACCEPTS.contains(&declared) {
        return Err(VariableError::TypeMismatch {
            variable: id.clone(),
            expected: T::ACCEPTS,
            found: declared,
        });
    }
    let resolved = resolve_tracked(doc, id, theme, read)?;
    T::from_resolved(&resolved).ok_or(VariableError::TypeMismatch {
        variable: id.clone(),
        expected: T::ACCEPTS,
        found: resolved.variable_type(),
    })
}

/// Whether `node` is shown under `theme`: its `visible`, which a boolean variable can decide.
pub fn resolve_visible(doc: &Document, node: &Node, theme: &Theme) -> Result<bool, VariableError> {
    resolve_value(doc, node.visible(), theme)
}
