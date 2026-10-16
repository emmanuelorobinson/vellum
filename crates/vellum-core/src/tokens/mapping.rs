//! Which selector each theme value is written under (DESIGN.md C20: "Light → `:root`, Dark → `.dark`").
//!
//! A mapping lists every value of every theme axis, in the axis's order. The first value of an axis is its default
//! and is written under the root selector, shared by all axes; every other value has a selector of its own.
//! [`ThemeMapping::derive`] suggests one from a document's themes; the person edits it by naming the rows to change
//! ([`ThemeMapping::with`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The selector the default value of every axis is written under.
pub const ROOT: &str = ":root";

/// One theme value and where it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeSelector {
    /// The theme axis, such as `Mode`.
    pub axis: String,
    /// The axis value, such as `Dark`.
    pub mode: String,
    /// The CSS selector: `.dark`, `[data-theme="high-contrast"]`, `[data-device="phone"]`.
    pub selector: String,
}

impl ModeSelector {
    /// A row.
    pub fn new(
        axis: impl Into<String>,
        mode: impl Into<String>,
        selector: impl Into<String>,
    ) -> Self {
        Self {
            axis: axis.into(),
            mode: mode.into(),
            selector: selector.into(),
        }
    }
}

/// Why a mapping was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingError(String);

impl fmt::Display for MappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MappingError {}

/// The theme values and their selectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeMapping {
    rows: Vec<ModeSelector>,
}

/// An attribute value or name as the default selectors spell it: lower case, words joined by hyphens.
fn slug(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
}

/// The selector a non-default value gets unless the person says otherwise: `Mode: Dark` is `.dark`, other `Mode`
/// values `[data-theme="…"]`, and any other axis `[data-<axis>="…"]` (`Device` gives `[data-device="phone"]`).
fn default_selector(axis: &str, mode: &str) -> String {
    let axis_slug = slug(axis);
    if axis_slug == "mode" {
        if slug(mode) == "dark" {
            ".dark".to_owned()
        } else {
            format!("[data-theme=\"{}\"]", slug(mode))
        }
    } else {
        format!("[data-{axis_slug}=\"{}\"]", slug(mode))
    }
}

/// A selector as compared: quotes are double, no space around `=` or inside the brackets, words separated by one
/// space.
pub fn normalize(selector: &str) -> String {
    let text = selector.replace('\'', "\"");
    let mut out = String::new();
    let mut quoted = false;
    for c in text.trim().chars() {
        if c == '"' {
            quoted = !quoted;
        }
        if !quoted && c.is_whitespace() {
            if !out.ends_with([' ', '=', '[']) {
                out.push(' ');
            }
            continue;
        }
        if !quoted && matches!(c, '=' | ']') && out.ends_with(' ') {
            out.pop();
        }
        out.push(c);
    }
    out
}

impl ThemeMapping {
    /// The selector the default value of every axis is written under.
    pub const ROOT: &'static str = ROOT;

    /// `Mode` with `Light` and `Dark`: what a document with no theme axes gets, so a file's `.dark` block can be
    /// imported into it.
    pub fn standard() -> Self {
        Self::derive(&BTreeMap::from([(
            "Mode".to_owned(),
            vec!["Light".to_owned(), "Dark".to_owned()],
        )]))
    }

    /// The mapping for a document's theme axes (`standard` when it has none).
    pub fn derive(themes: &BTreeMap<String, Vec<String>>) -> Self {
        if themes.values().all(Vec::is_empty) {
            return Self::standard();
        }
        let mut rows = Vec::new();
        for (axis, modes) in themes {
            for (i, mode) in modes.iter().enumerate() {
                let selector = if i == 0 {
                    ROOT.to_owned()
                } else {
                    default_selector(axis, mode)
                };
                rows.push(ModeSelector::new(axis, mode, selector));
            }
        }
        Self { rows }
    }

    /// Every row: axes in name order, values in the axis's order, defaults first.
    pub fn rows(&self) -> &[ModeSelector] {
        &self.rows
    }

    /// The selector `mode` of `axis` is written under.
    pub fn selector(&self, axis: &str, mode: &str) -> Option<&str> {
        self.rows
            .iter()
            .find(|row| row.axis == axis && row.mode == mode)
            .map(|row| row.selector.as_str())
    }

    /// Whether `mode` is the default of `axis` (its first value).
    pub fn is_default(&self, axis: &str, mode: &str) -> bool {
        self.rows
            .iter()
            .find(|row| row.axis == axis)
            .is_some_and(|row| row.mode == mode)
    }

    /// The non-default theme value written under `selector`, whatever its spelling of quotes and spaces. The
    /// root selector is no theme value; nor is a selector the mapping lacks.
    pub fn lookup(&self, selector: &str) -> Option<(&str, &str)> {
        let wanted = normalize(selector);
        self.rows
            .iter()
            .filter(|row| !self.is_default(&row.axis, &row.mode))
            .find(|row| normalize(&row.selector) == wanted)
            .map(|row| (row.axis.as_str(), row.mode.as_str()))
    }

    /// Whether `selector` is the root selector.
    pub fn is_root(selector: &str) -> bool {
        normalize(selector) == ROOT
    }

    /// The axes and values the mapping names, defaults first.
    pub fn themes(&self) -> BTreeMap<String, Vec<String>> {
        let mut themes: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for row in &self.rows {
            themes
                .entry(row.axis.clone())
                .or_default()
                .push(row.mode.clone());
        }
        themes
    }

    /// This mapping with `edits` applied. A row for a value the mapping has changes its selector; a row for one it
    /// lacks adds the value (as the axis's default, which must then take the root selector, when the axis is new).
    pub fn with(mut self, edits: &[ModeSelector]) -> Result<Self, MappingError> {
        for edit in edits {
            let edit = &ModeSelector {
                selector: normalize(&edit.selector),
                ..edit.clone()
            };
            match self
                .rows
                .iter_mut()
                .find(|row| row.axis == edit.axis && row.mode == edit.mode)
            {
                Some(row) => row.selector = edit.selector.clone(),
                None => self.rows.push(edit.clone()),
            }
        }
        self.rows.sort_by_key(|row| row.axis.clone());
        self.validate()?;
        Ok(self)
    }

    /// Checks the rules a mapping keeps: safe selectors, defaults on the root and nothing else there, no selector
    /// used twice.
    pub fn validate(&self) -> Result<(), MappingError> {
        let mut seen = BTreeSet::new();
        for row in &self.rows {
            let selector = row.selector.trim();
            let at = format!("{}: {}", row.axis, row.mode);
            if selector.is_empty()
                || selector.len() > 200
                || selector.contains(['{', '}', ';', '\n', '\r', '\0'])
                || selector.contains("/*")
            {
                return Err(MappingError(format!(
                    "{at}: {:?} is not a selector that can be written in a stylesheet.",
                    row.selector
                )));
            }
            let root = Self::is_root(selector);
            if self.is_default(&row.axis, &row.mode) {
                if !root {
                    return Err(MappingError(format!(
                        "{at}: the default value of an axis is written under {ROOT}, not {selector}."
                    )));
                }
            } else {
                if root {
                    return Err(MappingError(format!(
                        "{at}: {ROOT} is where default values go; give this value a selector of its own."
                    )));
                }
                if !seen.insert(normalize(selector)) {
                    return Err(MappingError(format!(
                        "{at}: {selector} is already the selector of another theme value."
                    )));
                }
            }
        }
        Ok(())
    }
}
