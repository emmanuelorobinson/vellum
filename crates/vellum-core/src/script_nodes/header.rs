//! Bounded, total parser for declarations in leading // and /* */ comments. No JS evaluation.
use crate::{doc_api::error, ports::ScriptError};
use std::collections::BTreeMap;
use vellum_types::{Color, Scalar};
const MAX_HEADER: usize = 64 * 1024;
#[derive(Debug, Clone, PartialEq)]
pub enum InputType {
    Number,
    String,
    Boolean,
    Color,
    Enum(Vec<String>),
    Ref,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub name: String,
    pub kind: InputType,
    pub args: BTreeMap<String, serde_json::Value>,
    pub default: Option<Scalar>,
    pub line: u32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub schema: String,
    pub inputs: Vec<Input>,
}
impl Header {
    pub fn parse(source: &str) -> Result<Self, ScriptError> {
        let mut header = Self {
            schema: "1".into(),
            inputs: vec![],
        };
        let mut schema = false;
        let mut offset = 0;
        let mut block = false;
        for (index, line) in source.lines().enumerate() {
            offset += line.len() + 1;
            let line = line.trim_start_matches('\u{feff}').trim();
            let body = if block {
                line
            } else if let Some(s) = line.strip_prefix("//") {
                s.trim()
            } else if let Some(s) = line.strip_prefix("/*") {
                block = true;
                s
            } else if line.is_empty() {
                continue;
            } else {
                break;
            };
            if offset > MAX_HEADER {
                return Err(error("Script header exceeds 64 KiB"));
            }
            let body = if block && let Some((s, _)) = body.split_once("*/") {
                block = false;
                s.trim()
            } else {
                body
            };
            let body = body.trim_start_matches('*').trim();
            if let Some(value) = body.strip_prefix("@schema ") {
                if schema || !matches!(value.trim(), "1" | "1.0") {
                    return Err(error(format!(
                        "Header line {}: unsupported or duplicate schema",
                        index + 1
                    )));
                }
                schema = true;
                header.schema = value.trim().into();
            } else if let Some(value) = body.strip_prefix("@input ") {
                if header.inputs.len() >= 128 {
                    return Err(error("Too many script inputs"));
                }
                let input = parse_input(value, index as u32 + 1)
                    .map_err(|e| error(format!("Header line {}: {e}", index + 1)))?;
                if header.inputs.iter().any(|i| i.name == input.name) {
                    return Err(error("Duplicate script input"));
                }
                header.inputs.push(input);
            } else if body.starts_with("@schema") || body.starts_with("@input") {
                return Err(error(format!(
                    "Header line {}: malformed declaration",
                    index + 1
                )));
            }
        }
        if block {
            return Err(error("Unterminated script header comment"));
        }
        Ok(header)
    }
}
fn split(s: &str, separator: char) -> Result<Vec<&str>, ScriptError> {
    let mut parts = vec![];
    let mut start = 0;
    let mut quote = false;
    let mut escape = false;
    let mut depth = 0u32;
    for (i, c) in s.char_indices() {
        if quote {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quote = false;
            }
            continue;
        }
        match c {
            '"' => quote = true,
            '(' | '[' | '{' => {
                depth += 1;
                if depth > 16 {
                    return Err(error("Header nesting exceeds 16"));
                }
            }
            ')' | ']' | '}' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| error("Unbalanced header arguments"))?;
            }
            _ if c == separator && depth == 0 => {
                parts.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    if quote || depth != 0 {
        return Err(error("Unterminated header value"));
    }
    parts.push(s[start..].trim());
    Ok(parts)
}
fn parse_input(s: &str, line: u32) -> Result<Input, ScriptError> {
    let (name, rest) = s
        .split_once(':')
        .ok_or_else(|| error("Expected name: type"))?;
    let name = name.trim();
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || c == b'_' || (i > 0 && c.is_ascii_digit()))
        || matches!(name, "__proto__" | "prototype" | "constructor")
    {
        return Err(error("Invalid input name"));
    }
    let pieces = split(rest, '=')?;
    if pieces.len() > 2 {
        return Err(error("Too many defaults"));
    }
    let ty = pieces[0].trim();
    let (ty, raw_args) = if let Some((ty, args)) = ty.split_once('(') {
        (
            ty.trim(),
            Some(
                args.strip_suffix(')')
                    .ok_or_else(|| error("Unclosed input arguments"))?,
            ),
        )
    } else {
        (ty, None)
    };
    let mut args = BTreeMap::new();
    let mut options = Vec::new();
    if let Some(raw) = raw_args {
        for item in split(raw, ',')? {
            if item.is_empty() {
                return Err(error("Empty input argument"));
            }
            if ty == "enum" && item.starts_with('"') {
                options.push(serde_json::from_str::<String>(item).map_err(error)?);
                continue;
            }
            let pair = split(item, '=')?;
            if pair.len() != 2 {
                return Err(error("Expected argument=value"));
            }
            if args
                .insert(
                    pair[0].to_owned(),
                    serde_json::from_str::<serde_json::Value>(pair[1]).map_err(error)?,
                )
                .is_some()
            {
                return Err(error("Duplicate input argument"));
            }
        }
    }
    let kind = match ty {
        "number" => {
            for (key, v) in &args {
                if !matches!(key.as_str(), "min" | "max" | "step")
                    || v.as_f64()
                        .is_none_or(|n| !n.is_finite() || (key == "step" && n <= 0.))
                {
                    return Err(error("Invalid number argument"));
                }
            }
            if args
                .get("min")
                .zip(args.get("max"))
                .is_some_and(|(a, b)| a.as_f64() > b.as_f64())
            {
                return Err(error("min exceeds max"));
            }
            InputType::Number
        }
        "string" => {
            for (key, v) in &args {
                if key != "multiline" || !v.is_boolean() {
                    return Err(error("Invalid string argument"));
                }
            }
            InputType::String
        }
        "boolean" if args.is_empty() => InputType::Boolean,
        "color" if args.is_empty() => InputType::Color,
        "ref" => {
            for (key, v) in &args {
                if key != "kind"
                    || !matches!(v.as_str(), Some("node" | "page" | "component" | "variable"))
                {
                    return Err(error("Invalid ref argument"));
                }
            }
            InputType::Ref
        }
        "enum" => {
            if let Some(v) = args.remove("options") {
                if !options.is_empty() {
                    return Err(error("Duplicate enum options"));
                }
                options = serde_json::from_value(v).map_err(error)?;
            }
            if !args.is_empty()
                || options.is_empty()
                || options.len() > 128
                || options
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != options.len()
            {
                return Err(error("Invalid enum options"));
            }
            InputType::Enum(options)
        }
        _ => return Err(error("Unknown input type or arguments")),
    };
    let default = pieces
        .get(1)
        .map(|s| serde_json::from_str::<Scalar>(s).map_err(error))
        .transpose()?;
    let input = Input {
        name: name.into(),
        kind,
        args,
        default,
        line,
    };
    if let Some(value) = &input.default {
        input.validate(value)?;
    }
    Ok(input)
}
impl Input {
    pub fn validate(&self, value: &Scalar) -> Result<(), ScriptError> {
        let valid = match (&self.kind, value) {
            (InputType::Number, Scalar::Number(n)) => {
                n.is_finite()
                    && self
                        .args
                        .get("min")
                        .and_then(|v| v.as_f64())
                        .is_none_or(|min| *n >= min)
                    && self
                        .args
                        .get("max")
                        .and_then(|v| v.as_f64())
                        .is_none_or(|max| *n <= max)
            }
            (InputType::String | InputType::Ref, Scalar::Text(_)) => true,
            (InputType::Boolean, Scalar::Bool(_)) => true,
            (InputType::Color, Scalar::Text(s)) => s.parse::<Color>().is_ok(),
            (InputType::Enum(options), Scalar::Text(s)) => options.contains(s),
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(error(format!("Invalid value for input {}", self.name)))
        }
    }
}
