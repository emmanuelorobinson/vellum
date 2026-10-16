//! CSS custom properties, plain (`css-vars`) and as a Tailwind v4 `@theme` block (`tailwind-v4`).
//!
//! The default values are written under the root selector (`:root`, or `@theme` for Tailwind) and each non-default
//! theme value under the selector the mapping gives it, holding only what differs. An alias is `var(--other)`.
//! Reading is forgiving, because real stylesheets hold much more than tokens: only custom properties are read, in
//! blocks whose selector is the root or one the mapping knows; everything else is skipped, and said so when it
//! held a custom property.

use std::collections::{BTreeMap, BTreeSet};

use vellum_core::ports::{TokenError, TokenExport, TokenFormat, TokenImport};
use vellum_core::tokens::{
    ModeValue, ThemeMapping, Token, TokenOptions, TokenSet, TokenValue, TokenWarning,
};
use vellum_types::VariableType;

use super::names::{self, HINT_KEY};
use super::value::{self, Parsed};

/// Plain CSS variables.
pub struct CssVars;

/// A Tailwind v4 stylesheet: `@theme { … }` for the defaults, plain blocks for the other theme values.
pub struct TailwindV4;

impl TokenFormat for CssVars {
    fn id(&self) -> &'static str {
        "css-vars"
    }
    fn label(&self) -> &'static str {
        "CSS variables"
    }
    fn file_name(&self) -> &'static str {
        "tokens.css"
    }
    fn export(&self, set: &TokenSet, options: &TokenOptions) -> Result<TokenExport, TokenError> {
        write(set, options, ThemeMapping::ROOT)
    }
    fn import(&self, text: &str, options: &TokenOptions) -> Result<TokenImport, TokenError> {
        read(text, options)
    }
}

impl TokenFormat for TailwindV4 {
    fn id(&self) -> &'static str {
        "tailwind-v4"
    }
    fn label(&self) -> &'static str {
        "Tailwind v4"
    }
    fn file_name(&self) -> &'static str {
        "tokens.css"
    }
    fn default_options(&self, themes: &BTreeMap<String, Vec<String>>) -> TokenOptions {
        let mut options = TokenOptions::for_themes(themes);
        options
            .namespaces
            .insert("space".to_owned(), "spacing".to_owned());
        options
    }
    fn export(&self, set: &TokenSet, options: &TokenOptions) -> Result<TokenExport, TokenError> {
        write(set, options, "@theme")
    }
    fn import(&self, text: &str, options: &TokenOptions) -> Result<TokenImport, TokenError> {
        read(text, options)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Writing

struct Declaration {
    property: String,
    value: String,
    hint: Option<String>,
}

fn text_of(
    token: &Token,
    value: &TokenValue,
    options: &TokenOptions,
    properties: &BTreeMap<&str, String>,
) -> Result<String, String> {
    match value {
        TokenValue::Color(color) => Ok(color.to_string()),
        TokenValue::Number(n) if !n.is_finite() => Err("its number is not finite.".to_owned()),
        TokenValue::Number(n) => {
            let text = value::format_number(*n);
            Ok(if options.is_length(&token.name) {
                format!("{text}px")
            } else {
                text
            })
        }
        TokenValue::String(text) => Ok(value::format_string(text)),
        TokenValue::Boolean(b) => Ok(b.to_string()),
        TokenValue::Alias(target) => properties
            .get(target.as_str())
            .map(|property| format!("var(--{property})"))
            .ok_or_else(|| format!("it refers to {target}, which is not written.")),
    }
}

fn write(set: &TokenSet, options: &TokenOptions, root: &str) -> Result<TokenExport, TokenError> {
    let mut warnings = Vec::new();

    // Which tokens have a property name of their own.
    let mut properties: BTreeMap<&str, String> = BTreeMap::new();
    let mut taken = BTreeSet::new();
    for token in &set.tokens {
        match names::property(&token.name, options) {
            None => warnings.push(TokenWarning::token(
                &token.name,
                "a stylesheet cannot carry this name; left out.",
            )),
            Some(property) if !taken.insert(property.clone()) => warnings.push(TokenWarning::token(
                &token.name,
                format!("it would be written as --{property}, which another token already is; left out."),
            )),
            Some(property) => {
                properties.insert(&token.name, property);
            }
        }
    }

    let mut blocks: Vec<(String, Vec<Declaration>)> = vec![(root.to_owned(), Vec::new())];
    for row in options.mapping.rows() {
        if !options.mapping.is_default(&row.axis, &row.mode) {
            blocks.push((row.selector.clone(), Vec::new()));
        }
    }
    let mut written = BTreeSet::new();
    for token in &set.tokens {
        let Some(property) = properties.get(token.name.as_str()) else {
            continue;
        };
        let default = match text_of(token, &token.value, options, &properties) {
            Ok(text) => text,
            Err(why) => {
                warnings.push(TokenWarning::token(&token.name, format!("{why} Left out.")));
                continue;
            }
        };
        written.insert(token.name.as_str());
        let hint = names::needs_hint(&token.name, property, options).then(|| token.name.clone());
        blocks[0].1.push(Declaration {
            property: property.clone(),
            value: default,
            hint,
        });
        for mode in &token.modes {
            let Some(selector) = options.mapping.selector(&mode.axis, &mode.mode) else {
                warnings.push(TokenWarning::token(
                    &token.name,
                    format!(
                        "its value for {}: {} has no selector in the mapping; left out.",
                        mode.axis, mode.mode
                    ),
                ));
                continue;
            };
            match text_of(token, &mode.value, options, &properties) {
                Ok(text) => {
                    if let Some((_, declarations)) =
                        blocks.iter_mut().skip(1).find(|(s, _)| s == selector)
                    {
                        declarations.push(Declaration {
                            property: property.clone(),
                            value: text,
                            hint: None,
                        });
                    }
                }
                Err(why) => warnings.push(TokenWarning::token(
                    &token.name,
                    format!(
                        "{why} Its value for {}: {} is left out.",
                        mode.axis, mode.mode
                    ),
                )),
            }
        }
    }

    let mut text = String::new();
    for (selector, declarations) in blocks.iter().filter(|(_, d)| !d.is_empty()) {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(selector);
        text.push_str(" {\n");
        for d in declarations {
            text.push_str(&format!("  --{}: {};", d.property, d.value));
            if let Some(hint) = &d.hint {
                text.push_str(&format!(" /* {HINT_KEY} {hint} */"));
            }
            text.push('\n');
        }
        text.push_str("}\n");
    }
    Ok(TokenExport { text, warnings })
}

// ---------------------------------------------------------------------------------------------------------
// Parsing

#[derive(Debug)]
struct Decl {
    property: String,
    value: String,
    hint: Option<String>,
}

#[derive(Debug)]
struct Rule {
    selectors: Vec<String>,
    /// An `@theme` block: always the root.
    theme: bool,
    declarations: Vec<Decl>,
}

struct Parser {
    chars: Vec<char>,
    at: usize,
    line: u32,
    rules: Vec<Rule>,
    warnings: Vec<TokenWarning>,
}

/// What ended a run of text.
#[derive(PartialEq, Clone, Copy)]
enum End {
    Open,
    Semi,
    Close,
    Eof,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        if c == '\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn comment(&mut self) -> Result<String, TokenError> {
        let line = self.line;
        self.next();
        self.next();
        let mut text = String::new();
        loop {
            match self.next() {
                None => return Err(TokenError::new("a comment is never closed.").at_line(line)),
                Some('*') if self.peek() == Some('/') => {
                    self.next();
                    return Ok(text);
                }
                Some(c) => text.push(c),
            }
        }
    }

    fn skip_space(&mut self) -> Result<(), TokenError> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.next();
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    self.comment()?;
                }
                _ => return Ok(()),
            }
        }
    }

    fn string(&mut self, out: &mut String) -> Result<(), TokenError> {
        let line = self.line;
        let quote = self.next().expect("a quote");
        out.push(quote);
        loop {
            match self.next() {
                None | Some('\n') => {
                    return Err(TokenError::new("a string is never closed.").at_line(line));
                }
                Some('\\') => {
                    out.push('\\');
                    if let Some(c) = self.next() {
                        out.push(c);
                    }
                }
                Some(c) => {
                    out.push(c);
                    if c == quote {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Text up to a `{`, `;` or `}` outside brackets, strings and comments. The `}` is left unread.
    fn run(&mut self) -> Result<(String, End), TokenError> {
        let mut out = String::new();
        let mut depth = 0usize;
        loop {
            match self.peek() {
                None => return Ok((out.trim().to_owned(), End::Eof)),
                Some('/') if self.peek_at(1) == Some('*') => {
                    self.comment()?;
                }
                Some('"' | '\'') => self.string(&mut out)?,
                Some(c @ ('(' | '[')) => {
                    depth += 1;
                    out.push(c);
                    self.next();
                }
                Some(c @ (')' | ']')) => {
                    depth = depth.saturating_sub(1);
                    out.push(c);
                    self.next();
                }
                Some('{') if depth == 0 => {
                    self.next();
                    return Ok((out.trim().to_owned(), End::Open));
                }
                Some(';') if depth == 0 => {
                    self.next();
                    return Ok((out.trim().to_owned(), End::Semi));
                }
                Some('}') if depth == 0 => return Ok((out.trim().to_owned(), End::Close)),
                Some(c) => {
                    out.push(c);
                    self.next();
                }
            }
        }
    }

    /// Skips the rest of a block whose `{` was read at `line`.
    fn skip_block(&mut self, line: u32) -> Result<(), TokenError> {
        let mut depth = 1usize;
        loop {
            match self.peek() {
                None => return Err(never_closed(line)),
                Some('/') if self.peek_at(1) == Some('*') => {
                    self.comment()?;
                }
                Some('"' | '\'') => self.string(&mut String::new())?,
                Some('{') => {
                    depth += 1;
                    self.next();
                }
                Some('}') => {
                    depth -= 1;
                    self.next();
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Some(_) => {
                    self.next();
                }
            }
        }
    }

    /// The comment right after a declaration on its line, when it is a `name:` hint.
    fn hint(&mut self) -> Result<Option<String>, TokenError> {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.next();
        }
        if self.peek() == Some('/') && self.peek_at(1) == Some('*') {
            let text = self.comment()?;
            return Ok(text
                .trim()
                .strip_prefix(HINT_KEY)
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty()));
        }
        Ok(None)
    }

    fn declarations(&mut self, line: u32) -> Result<Vec<Decl>, TokenError> {
        let mut out = Vec::new();
        loop {
            self.skip_space()?;
            if self.peek().is_none() {
                return Err(never_closed(line));
            }
            if self.peek() == Some('}') {
                self.next();
                return Ok(out);
            }
            let started = self.line;
            let (text, end) = self.run()?;
            match end {
                End::Open => {
                    // Nested CSS: not ours.
                    self.skip_block(started)?;
                    continue;
                }
                End::Eof => return Err(never_closed(line)),
                End::Semi | End::Close => {}
            }
            let hint = if end == End::Semi { self.hint()? } else { None };
            if let Some((property, value)) = text.split_once(':')
                && property.trim().starts_with("--")
            {
                let mut value = value.trim();
                if let Some(at) = value.to_ascii_lowercase().rfind('!')
                    && value[at + 1..].trim().eq_ignore_ascii_case("important")
                {
                    value = value[..at].trim();
                }
                if !value.is_empty() {
                    out.push(Decl {
                        property: property.trim()[2..].to_owned(),
                        value: value.to_owned(),
                        hint,
                    });
                }
            }
            if end == End::Close {
                self.next();
                return Ok(out);
            }
        }
    }

    fn rules(&mut self, open: Option<u32>) -> Result<(), TokenError> {
        loop {
            self.skip_space()?;
            let line = self.line;
            match self.peek() {
                None => return open.map_or(Ok(()), |l| Err(never_closed(l))),
                Some('}') => {
                    self.next();
                    if open.is_some() {
                        return Ok(());
                    }
                    continue;
                }
                _ => {}
            }
            let (prelude, end) = self.run()?;
            match end {
                End::Semi | End::Close | End::Eof => {
                    if end == End::Eof && open.is_some() {
                        return Err(never_closed(open.unwrap_or(line)));
                    }
                }
                End::Open if prelude.starts_with("@theme") => {
                    let declarations = self.declarations(line)?;
                    self.rules.push(Rule {
                        selectors: vec![ThemeMapping::ROOT.to_owned()],
                        theme: true,
                        declarations,
                    });
                }
                End::Open if prelude.starts_with("@layer") => self.rules(Some(line))?,
                End::Open if prelude.starts_with('@') => {
                    let name = prelude.split_whitespace().next().unwrap_or("@").to_owned();
                    self.warnings.push(TokenWarning::general(format!(
                        "ignored `{prelude}` on line {line}: tokens inside {name} are not read."
                    )));
                    self.skip_block(line)?;
                }
                End::Open => {
                    let declarations = self.declarations(line)?;
                    self.rules.push(Rule {
                        selectors: split_selectors(&prelude),
                        theme: false,
                        declarations,
                    });
                }
            }
        }
    }
}

fn never_closed(line: u32) -> TokenError {
    TokenError::new("a block is never closed.").at_line(line)
}

/// A selector list at its top-level commas.
fn split_selectors(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let (mut depth, mut quoted, mut start) = (0usize, None::<char>, 0usize);
    for (i, c) in text.char_indices() {
        match (quoted, c) {
            (Some(q), c) if c == q => quoted = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quoted = Some(c),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => depth = depth.saturating_sub(1),
            (None, ',') if depth == 0 => {
                parts.push(text[start..i].trim().to_owned());
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim().to_owned());
    parts.retain(|p| !p.is_empty());
    parts
}

// ---------------------------------------------------------------------------------------------------------
// From declarations to tokens

#[derive(Clone)]
struct Raw {
    value: String,
    hint: Option<String>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Bucket {
    Root,
    Mode(String, String),
}

struct Reader<'a> {
    options: &'a TokenOptions,
    root: &'a BTreeMap<String, Raw>,
    names: &'a BTreeMap<String, String>,
}

impl Reader<'_> {
    /// The token a `var(--property)` points at: one of the file's, or one the document has.
    fn target(&self, property: &str) -> Result<String, String> {
        if let Some(name) = self.names.get(property) {
            return Ok(name.clone());
        }
        self.options
            .known
            .keys()
            .find(|name| names::property(name, self.options).as_deref() == Some(property))
            .cloned()
            .ok_or_else(|| format!("it refers to `--{property}`, which is not defined."))
    }

    /// The type of the token called `name`, following aliases through the file.
    fn kind_of(&self, name: &str, path: &mut Vec<String>) -> Result<VariableType, String> {
        if path.iter().any(|n| n == name) {
            path.push(name.to_owned());
            return Err(format!(
                "its aliases lead back to it ({}).",
                path.join(" → ")
            ));
        }
        let Some((property, _)) = self.names.iter().find(|(_, n)| n.as_str() == name) else {
            return self
                .options
                .known
                .get(name)
                .copied()
                .ok_or_else(|| format!("it refers to {name}, which is not defined."));
        };
        let raw = self
            .root
            .get(property)
            .ok_or_else(|| format!("{name} has no default value."))?;
        match value::classify(&raw.value) {
            Parsed::Var(target) => {
                path.push(name.to_owned());
                let target = self.target(&target)?;
                self.kind_of(&target, path)
            }
            parsed => Ok(kind(&parsed)),
        }
    }

    fn value(&self, text: &str) -> Result<TokenValue, String> {
        Ok(match value::classify(text) {
            Parsed::Color(c) => TokenValue::Color(c),
            Parsed::Number(n) => TokenValue::Number(n),
            Parsed::Boolean(b) => TokenValue::Boolean(b),
            Parsed::Text(t) => TokenValue::String(t),
            Parsed::Var(property) => TokenValue::Alias(self.target(&property)?),
        })
    }
}

fn kind(parsed: &Parsed) -> VariableType {
    match parsed {
        Parsed::Color(_) => VariableType::Color,
        Parsed::Number(_) => VariableType::Number,
        Parsed::Boolean(_) => VariableType::Boolean,
        Parsed::Text(_) | Parsed::Var(_) => VariableType::String,
    }
}

fn read(text: &str, options: &TokenOptions) -> Result<TokenImport, TokenError> {
    let mut parser = Parser {
        chars: text.chars().collect(),
        at: 0,
        line: 1,
        rules: Vec::new(),
        warnings: Vec::new(),
    };
    parser.rules(None)?;
    let Parser {
        rules,
        mut warnings,
        ..
    } = parser;

    // Sort every custom property into the root or a theme value.
    let mut buckets: BTreeMap<Bucket, BTreeMap<String, Raw>> = BTreeMap::new();
    let mut unknown = BTreeSet::new();
    for rule in &rules {
        if rule.declarations.is_empty() {
            continue;
        }
        for selector in &rule.selectors {
            let bucket = if rule.theme || ThemeMapping::is_root(selector) {
                Bucket::Root
            } else if let Some((axis, mode)) = options.mapping.lookup(selector) {
                Bucket::Mode(axis.to_owned(), mode.to_owned())
            } else {
                if unknown.insert(selector.clone()) {
                    warnings.push(TokenWarning::general(format!(
                        "ignored `{selector}`: no theme value is written under that selector."
                    )));
                }
                continue;
            };
            let entries = buckets.entry(bucket).or_default();
            for d in &rule.declarations {
                if d.property
                    .contains(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_')))
                {
                    continue;
                }
                entries.insert(
                    d.property.clone(),
                    Raw {
                        value: d.value.clone(),
                        hint: d.hint.clone(),
                    },
                );
            }
        }
    }
    let root = buckets.remove(&Bucket::Root).unwrap_or_default();

    // Names: a comment's, else the document's, else the property's own.
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for property in root.keys().chain(buckets.values().flat_map(BTreeMap::keys)) {
        let hint = root.get(property).and_then(|r| r.hint.clone()).or_else(|| {
            buckets
                .values()
                .find_map(|b| b.get(property).and_then(|r| r.hint.clone()))
        });
        names
            .entry(property.clone())
            .or_insert_with(|| hint.unwrap_or_else(|| names::name_of(property, options)));
    }

    let reader = Reader {
        options,
        root: &root,
        names: &names,
    };
    let mut tokens: BTreeMap<String, Token> = BTreeMap::new();
    for (property, raw) in &root {
        let name = &names[property];
        let result = reader
            .kind_of(name, &mut Vec::new())
            .and_then(|kind| Ok((kind, reader.value(&raw.value)?)));
        match result {
            Ok((kind, value)) => {
                tokens.insert(
                    name.clone(),
                    Token {
                        name: name.clone(),
                        kind,
                        value,
                        modes: Vec::new(),
                    },
                );
            }
            Err(why) => warnings.push(TokenWarning::token(name, format!("{why} Left out."))),
        }
    }
    let mut used_axes = BTreeSet::new();
    for (bucket, entries) in &buckets {
        let Bucket::Mode(axis, mode) = bucket else {
            continue;
        };
        for (property, raw) in entries {
            let name = &names[property];
            let Some(token) = tokens.get_mut(name) else {
                if !root.contains_key(property) {
                    warnings.push(TokenWarning::token(
                        name,
                        format!(
                            "it is only given under {axis}: {mode}, with no default; left out."
                        ),
                    ));
                }
                continue;
            };
            let kind_ok = match value::classify(&raw.value) {
                Parsed::Var(target) => reader
                    .target(&target)
                    .and_then(|t| reader.kind_of(&t, &mut Vec::new()))
                    .map(|k| k == token.kind),
                parsed => Ok(kind(&parsed) == token.kind),
            };
            match kind_ok {
                Ok(true) => {}
                Ok(false) => {
                    warnings.push(TokenWarning::token(
                        name,
                        format!(
                            "the value under {axis}: {mode} is not a {}; left out.",
                            token.kind.as_str()
                        ),
                    ));
                    continue;
                }
                Err(why) => {
                    warnings.push(TokenWarning::token(
                        name,
                        format!("under {axis}: {mode}, {why} Left out."),
                    ));
                    continue;
                }
            }
            match reader.value(&raw.value) {
                Ok(value) if value == token.value => {}
                Ok(value) => {
                    used_axes.insert(axis.clone());
                    token.modes.push(ModeValue {
                        axis: axis.clone(),
                        mode: mode.clone(),
                        value,
                    });
                }
                Err(why) => warnings.push(TokenWarning::token(name, format!("{why} Left out."))),
            }
        }
    }

    // A reference to a token that was left out leads nowhere.
    loop {
        let present: BTreeSet<String> = tokens.keys().cloned().collect();
        let gone = |value: &TokenValue| {
            matches!(value, TokenValue::Alias(target)
                if !present.contains(target) && !options.known.contains_key(target))
        };
        let dangling: Vec<String> = tokens
            .values()
            .filter(|t| gone(&t.value))
            .map(|t| t.name.clone())
            .collect();
        for name in &dangling {
            tokens.remove(name);
            warnings.push(TokenWarning::token(
                name,
                "it refers to a token that was left out; left out.",
            ));
        }
        let mut changed = !dangling.is_empty();
        for token in tokens.values_mut() {
            let before = token.modes.len();
            token.modes.retain(|m| !gone(&m.value));
            changed |= token.modes.len() != before;
        }
        if !changed {
            break;
        }
    }

    let all = options.mapping.themes();
    let themes = used_axes
        .into_iter()
        .filter_map(|axis| all.get(&axis).map(|values| (axis.clone(), values.clone())))
        .collect();
    let mut tokens: Vec<Token> = tokens.into_values().collect();
    for token in &mut tokens {
        let order = |m: &ModeValue| {
            all.get(&m.axis)
                .and_then(|values| values.iter().position(|v| *v == m.mode))
                .unwrap_or(usize::MAX)
        };
        token.modes.sort_by_key(|m| (m.axis.clone(), order(m)));
    }
    Ok(TokenImport {
        set: TokenSet { themes, tokens },
        warnings,
    })
}
