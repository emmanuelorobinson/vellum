//! W3C Design Tokens (DTCG) JSON: `{"color": {"primary": {"$type": "color", "$value": "#2563EB"}}}`.
//!
//! The format has no themes, so values under a non-default theme value, and the axes themselves, go in a
//! `$extensions` entry named for the product (`modes`, `themes`), where tools that do not know it pass them
//! through. Vellum's strings and booleans are not DTCG types; they are written with the types `string` and
//! `boolean`, which a tool that does not know them ignores or keeps. A name a reference cannot spell (a `.`, a
//! brace, a leading `$`) is written with `_` in its place and keeps its real name in the extension.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Number, Value, json};
use vellum_core::ports::{TokenError, TokenExport, TokenFormat, TokenImport};
use vellum_core::tokens::{ModeValue, Token, TokenOptions, TokenSet, TokenValue, TokenWarning};
use vellum_types::{VariableType, brand};

use super::value::{self, Parsed};

/// W3C Design Tokens JSON.
pub struct DtcgJson;

/// The key tool-specific data goes under.
fn extension_key() -> String {
    brand::NAME.to_lowercase()
}

impl TokenFormat for DtcgJson {
    fn id(&self) -> &'static str {
        "dtcg-json"
    }
    fn label(&self) -> &'static str {
        "Design Tokens JSON"
    }
    fn file_name(&self) -> &'static str {
        "tokens.json"
    }
    fn export(&self, set: &TokenSet, options: &TokenOptions) -> Result<TokenExport, TokenError> {
        write(set, options)
    }
    fn import(&self, text: &str, options: &TokenOptions) -> Result<TokenImport, TokenError> {
        read(text, options)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Writing

fn segment(text: &str) -> String {
    if text.is_empty() {
        return "_".to_owned();
    }
    let text: String = text
        .chars()
        .map(|c| if matches!(c, '.' | '{' | '}') { '_' } else { c })
        .collect();
    match text.strip_prefix('$') {
        Some(rest) => format!("_{rest}"),
        None => text,
    }
}

fn path_of(name: &str) -> Vec<String> {
    name.split('/').map(segment).collect()
}

fn number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        Value::Number(Number::from(n as i64))
    } else {
        Number::from_f64(n).map_or(Value::Null, Value::Number)
    }
}

/// A value in `$value` notation, and the DTCG type of the token it belongs to.
fn encode(
    token: &Token,
    value: &TokenValue,
    options: &TokenOptions,
    paths: &BTreeMap<&str, String>,
) -> Result<Value, String> {
    Ok(match value {
        TokenValue::Color(color) => json!(color.to_string()),
        TokenValue::Number(n) if !n.is_finite() => return Err("its number is not finite.".into()),
        TokenValue::Number(n) if options.is_length(&token.name) => {
            json!(format!("{}px", value::format_number(*n)))
        }
        TokenValue::Number(n) => number(*n),
        TokenValue::String(text) => json!(text),
        TokenValue::Boolean(b) => json!(b),
        TokenValue::Alias(target) => {
            let path = paths
                .get(target.as_str())
                .ok_or_else(|| format!("it refers to {target}, which is not written."))?;
            json!(format!("{{{path}}}"))
        }
    })
}

fn type_of(token: &Token, options: &TokenOptions) -> &'static str {
    match token.kind {
        VariableType::Color => "color",
        VariableType::Number if options.is_length(&token.name) => "dimension",
        VariableType::Number => "number",
        VariableType::String => "string",
        VariableType::Boolean => "boolean",
    }
}

/// Puts `leaf` at `path`, or says why it cannot go there.
fn insert(root: &mut Map<String, Value>, path: &[String], leaf: Value) -> Result<(), &'static str> {
    let (last, parents) = path.split_last().expect("a path has a segment");
    let mut at = root;
    for part in parents {
        let entry = at
            .entry(part.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        let Some(group) = entry.as_object_mut() else {
            return Err("a name inside a token");
        };
        if group.contains_key("$value") {
            return Err("a token cannot also be a group");
        }
        at = group;
    }
    if at.contains_key(last) {
        return Err("a token cannot share its name with a group or another token");
    }
    at.insert(last.clone(), leaf);
    Ok(())
}

fn write(set: &TokenSet, options: &TokenOptions) -> Result<TokenExport, TokenError> {
    let mut warnings = Vec::new();
    let ext = extension_key();
    let paths: BTreeMap<&str, String> = set
        .tokens
        .iter()
        .map(|t| (t.name.as_str(), path_of(&t.name).join(".")))
        .collect();
    let mut root = Map::new();
    if !set.themes.is_empty() {
        root.insert(
            "$extensions".into(),
            json!({ ext.clone(): {"themes": set.themes} }),
        );
    }
    for token in &set.tokens {
        let value = match encode(token, &token.value, options, &paths) {
            Ok(v) => v,
            Err(why) => {
                warnings.push(TokenWarning::token(&token.name, format!("{why} Left out.")));
                continue;
            }
        };
        let path = path_of(&token.name);
        let mut leaf = Map::new();
        leaf.insert("$type".into(), json!(type_of(token, options)));
        leaf.insert("$value".into(), value);
        let mut extra = Map::new();
        if path.join("/") != token.name {
            extra.insert("name".into(), json!(token.name));
        }
        let mut modes: BTreeMap<&str, Map<String, Value>> = BTreeMap::new();
        for mode in &token.modes {
            match encode(token, &mode.value, options, &paths) {
                Ok(v) => {
                    modes
                        .entry(&mode.axis)
                        .or_default()
                        .insert(mode.mode.clone(), v);
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
        if !modes.is_empty() {
            extra.insert("modes".into(), json!(modes));
        }
        if !extra.is_empty() {
            leaf.insert("$extensions".into(), json!({ ext.clone(): extra }));
        }
        if let Err(why) = insert(&mut root, &path, Value::Object(leaf)) {
            warnings.push(TokenWarning::token(
                &token.name,
                format!("{why}; left out."),
            ));
        }
    }
    let mut text = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| TokenError::new(e.to_string()))?;
    text.push('\n');
    Ok(TokenExport { text, warnings })
}

// ---------------------------------------------------------------------------------------------------------
// Reading

struct Leaf {
    name: String,
    dotted: String,
    kind: Option<String>,
    value: Value,
    modes: Vec<(String, String, Value)>,
}

fn weight(word: &str) -> Option<f64> {
    Some(
        match word.to_ascii_lowercase().replace([' ', '_'], "-").as_str() {
            "thin" | "hairline" => 100.0,
            "extra-light" | "ultra-light" => 200.0,
            "light" => 300.0,
            "normal" | "regular" | "book" => 400.0,
            "medium" => 500.0,
            "semi-bold" | "demi-bold" => 600.0,
            "bold" => 700.0,
            "extra-bold" | "ultra-bold" => 800.0,
            "black" | "heavy" => 900.0,
            "extra-black" | "ultra-black" => 950.0,
            _ => return None,
        },
    )
}

fn with_unit(object: &Map<String, Value>) -> Option<(f64, String)> {
    Some((
        object.get("value")?.as_f64()?,
        object.get("unit")?.as_str()?.to_owned(),
    ))
}

/// The alias a value is, as the dotted path in its braces.
fn alias_of(value: &Value) -> Option<&str> {
    value.as_str()?.strip_prefix('{')?.strip_suffix('}')
}

fn colour(value: &Value) -> Result<TokenValue, String> {
    match value {
        Value::String(text) => value::parse_color(text)
            .map(TokenValue::Color)
            .ok_or_else(|| format!("{text:?} is not a color.")),
        Value::Object(object) => {
            if let Some(color) = object
                .get("hex")
                .and_then(Value::as_str)
                .and_then(value::parse_color)
            {
                return Ok(TokenValue::Color(color));
            }
            let space = object
                .get("colorSpace")
                .and_then(Value::as_str)
                .unwrap_or("");
            let components: Option<Vec<f64>> = object
                .get("components")
                .and_then(Value::as_array)
                .map(|c| c.iter().filter_map(Value::as_f64).collect());
            match (space, components) {
                ("srgb", Some(c)) if c.len() == 3 => {
                    let byte = |x: f64| (x * 255.0).round().clamp(0.0, 255.0) as u8;
                    let alpha = object.get("alpha").and_then(Value::as_f64).unwrap_or(1.0);
                    Ok(TokenValue::Color(vellum_types::Color::rgba(
                        byte(c[0]),
                        byte(c[1]),
                        byte(c[2]),
                        byte(alpha),
                    )))
                }
                (space, _) => Err(format!(
                    "a color in {space:?} without a hex fallback cannot be read."
                )),
            }
        }
        _ => Err("it is not a color.".to_owned()),
    }
}

/// A leaf's value as a token value and its type.
fn convert(kind: Option<&str>, value: &Value) -> Result<(VariableType, TokenValue), String> {
    let number_of = |v: TokenValue| (VariableType::Number, v);
    let text_of = |s: String| (VariableType::String, TokenValue::String(s));
    match kind {
        Some("color") => Ok((VariableType::Color, colour(value)?)),
        Some("dimension") => match value {
            Value::String(text) => Ok(match value::parse_length(text) {
                Some(n) => number_of(TokenValue::Number(n)),
                None => text_of(text.clone()),
            }),
            Value::Object(object) => {
                let (n, unit) = with_unit(object).ok_or("it is not a dimension.")?;
                Ok(match unit.as_str() {
                    "px" => number_of(TokenValue::Number(n)),
                    "rem" => number_of(TokenValue::Number(n * 16.0)),
                    other => text_of(format!("{}{other}", value::format_number(n))),
                })
            }
            Value::Number(n) => Ok(number_of(TokenValue::Number(n.as_f64().unwrap_or(0.0)))),
            _ => Err("it is not a dimension.".to_owned()),
        },
        Some("number") => value
            .as_f64()
            .map(|n| number_of(TokenValue::Number(n)))
            .ok_or_else(|| "it is not a number.".to_owned()),
        Some("fontFamily") => match value {
            Value::String(s) => Ok(text_of(s.clone())),
            Value::Array(items) => Ok(text_of(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
            _ => Err("it is not a font family.".to_owned()),
        },
        Some("fontWeight") => match value {
            Value::Number(n) => Ok(number_of(TokenValue::Number(n.as_f64().unwrap_or(0.0)))),
            Value::String(word) => Ok(match weight(word) {
                Some(n) => number_of(TokenValue::Number(n)),
                None => text_of(word.clone()),
            }),
            _ => Err("it is not a font weight.".to_owned()),
        },
        Some("duration") => match value {
            Value::String(s) => Ok(text_of(s.clone())),
            Value::Object(object) => {
                let (n, unit) = with_unit(object).ok_or("it is not a duration.")?;
                Ok(text_of(format!("{}{unit}", value::format_number(n))))
            }
            _ => Err("it is not a duration.".to_owned()),
        },
        Some("string") => value
            .as_str()
            .map(|s| text_of(s.to_owned()))
            .ok_or_else(|| "it is not a string.".to_owned()),
        Some("boolean") => value
            .as_bool()
            .map(|b| (VariableType::Boolean, TokenValue::Boolean(b)))
            .ok_or_else(|| "it is not a boolean.".to_owned()),
        Some(other) => Err(format!("its type `{other}` is not supported.")),
        None => match value {
            Value::Bool(b) => Ok((VariableType::Boolean, TokenValue::Boolean(*b))),
            Value::Number(n) => Ok(number_of(TokenValue::Number(n.as_f64().unwrap_or(0.0)))),
            Value::String(s) => Ok(match value::classify(s) {
                Parsed::Color(c) if s.starts_with('#') => {
                    (VariableType::Color, TokenValue::Color(c))
                }
                _ => text_of(s.clone()),
            }),
            _ => Err("it has no type and its value is not a plain one.".to_owned()),
        },
    }
}

fn walk(
    group: &Map<String, Value>,
    path: &mut Vec<String>,
    inherited: Option<&str>,
    ext: &str,
    leaves: &mut Vec<Leaf>,
    warnings: &mut Vec<TokenWarning>,
) {
    let kind = group.get("$type").and_then(Value::as_str).or(inherited);
    if group.contains_key("$extends") {
        warnings.push(TokenWarning::general(format!(
            "`$extends` in `{}` is not supported; its own tokens are read.",
            path.join(".")
        )));
    }
    for (key, child) in group {
        let Some(child) = child.as_object() else {
            continue;
        };
        let is_root = key == "$root";
        if key.starts_with('$') && !is_root {
            continue;
        }
        if !is_root {
            path.push(key.clone());
        }
        if let Some(value) = child.get("$value") {
            let extension = child.get("$extensions").and_then(|e| e.get(ext));
            let name = extension
                .and_then(|e| e.get("name"))
                .and_then(Value::as_str)
                .map_or_else(|| path.join("/"), str::to_owned);
            if path.iter().any(|p| p.contains('.')) {
                warnings.push(TokenWarning::token(
                    &name,
                    "its name has a dot, which a reference in `{…}` cannot spell.",
                ));
            }
            let mut modes = Vec::new();
            if let Some(by_axis) = extension
                .and_then(|e| e.get("modes"))
                .and_then(Value::as_object)
            {
                for (axis, values) in by_axis {
                    for (mode, value) in values.as_object().into_iter().flatten() {
                        modes.push((axis.clone(), mode.clone(), value.clone()));
                    }
                }
            }
            leaves.push(Leaf {
                dotted: path.join("."),
                name,
                kind: child
                    .get("$type")
                    .and_then(Value::as_str)
                    .or(kind)
                    .map(str::to_owned),
                value: value.clone(),
                modes,
            });
        } else {
            walk(child, path, kind, ext, leaves, warnings);
        }
        if !is_root {
            path.pop();
        }
    }
}

fn read(text: &str, options: &TokenOptions) -> Result<TokenImport, TokenError> {
    let parsed: Value = serde_json::from_str(text).map_err(|e| {
        let error = TokenError::new(format!("this is not JSON: {e}."));
        match u32::try_from(e.line()) {
            Ok(line) if line > 0 => error.at_line(line),
            _ => error,
        }
    })?;
    let Some(root) = parsed.as_object() else {
        return Err(TokenError::new(
            "a Design Tokens file is a JSON object of groups and tokens.",
        ));
    };
    let ext = extension_key();
    let mut warnings = Vec::new();
    let mut leaves = Vec::new();
    walk(
        root,
        &mut Vec::new(),
        None,
        &ext,
        &mut leaves,
        &mut warnings,
    );

    // Names by reference path, and every leaf's type, with aliases followed.
    let by_path: BTreeMap<&str, &str> = leaves
        .iter()
        .map(|l| (l.dotted.as_str(), l.name.as_str()))
        .collect();
    let by_name: BTreeMap<&str, &Leaf> = leaves.iter().map(|l| (l.name.as_str(), l)).collect();

    fn kind_of(
        leaf: &Leaf,
        by_path: &BTreeMap<&str, &str>,
        by_name: &BTreeMap<&str, &Leaf>,
        path: &mut Vec<String>,
    ) -> Result<VariableType, String> {
        match alias_of(&leaf.value) {
            None => convert(leaf.kind.as_deref(), &leaf.value).map(|(kind, _)| kind),
            Some(target) => {
                if path.contains(&leaf.name) {
                    path.push(leaf.name.clone());
                    return Err(format!(
                        "its aliases lead back to it ({}).",
                        path.join(" → ")
                    ));
                }
                path.push(leaf.name.clone());
                let name = by_path
                    .get(target)
                    .ok_or_else(|| format!("it refers to `{{{target}}}`, which is not a token."))?;
                kind_of(by_name[name], by_path, by_name, path)
            }
        }
    }

    let resolve =
        |value: &Value, kind: Option<&str>| -> Result<(VariableType, TokenValue), String> {
            match alias_of(value) {
                Some(target) => {
                    let name = by_path.get(target).ok_or_else(|| {
                        format!("it refers to `{{{target}}}`, which is not a token.")
                    })?;
                    let leaf = by_name[name];
                    let kind = kind_of(leaf, &by_path, &by_name, &mut Vec::new())?;
                    Ok((kind, TokenValue::Alias((*name).to_owned())))
                }
                None => convert(kind, value),
            }
        };

    let mut tokens: BTreeMap<String, Token> = BTreeMap::new();
    let mut used_axes = BTreeSet::new();
    for leaf in &leaves {
        let (kind, value) = match resolve(&leaf.value, leaf.kind.as_deref()) {
            Ok(found) => found,
            Err(why) => {
                warnings.push(TokenWarning::token(&leaf.name, format!("{why} Left out.")));
                continue;
            }
        };
        let mut token = Token {
            name: leaf.name.clone(),
            kind,
            value,
            modes: Vec::new(),
        };
        for (axis, mode, raw) in &leaf.modes {
            match resolve(raw, leaf.kind.as_deref()) {
                Ok((k, v)) if k == kind => {
                    if v != token.value {
                        used_axes.insert(axis.clone());
                        token.modes.push(ModeValue {
                            axis: axis.clone(),
                            mode: mode.clone(),
                            value: v,
                        });
                    }
                }
                Ok(_) => warnings.push(TokenWarning::token(
                    &leaf.name,
                    format!(
                        "the value under {axis}: {mode} is not a {}; left out.",
                        kind.as_str()
                    ),
                )),
                Err(why) => warnings.push(TokenWarning::token(
                    &leaf.name,
                    format!("under {axis}: {mode}, {why} Left out."),
                )),
            }
        }
        tokens.insert(leaf.name.clone(), token);
    }
    // A reference to a token that was left out leads nowhere.
    loop {
        let present: BTreeSet<String> = tokens.keys().cloned().collect();
        let gone = |v: &TokenValue| matches!(v, TokenValue::Alias(t) if !present.contains(t));
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

    let written: BTreeMap<String, Vec<String>> = root
        .get("$extensions")
        .and_then(|e| e.get(&ext))
        .and_then(|e| e.get("themes"))
        .and_then(|t| serde_json::from_value(t.clone()).ok())
        .unwrap_or_default();
    let from_mapping = options.mapping.themes();
    let themes = if written.is_empty() {
        used_axes
            .into_iter()
            .filter_map(|axis| from_mapping.get(&axis).map(|v| (axis.clone(), v.clone())))
            .collect()
    } else {
        written
    };
    Ok(TokenImport {
        set: TokenSet {
            themes,
            tokens: tokens.into_values().collect(),
        },
        warnings,
    })
}
