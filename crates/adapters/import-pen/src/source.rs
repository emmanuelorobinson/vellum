use crate::{MAX_BYTES, MAX_DEPTH, MAX_NODES, error};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use vellum_core::ports::{ImportError, ImportInput};
pub type Theme = BTreeMap<String, String>;
pub fn read(input: &ImportInput<'_>) -> Result<Value, ImportError> {
    if input.bytes.len() > MAX_BYTES {
        return Err(error("The .pen input exceeds 32 MB."));
    }
    let bytes = comments(input.bytes)?;
    let v: Value = serde_json::from_slice(&bytes)
        .map_err(|_| error("The .pen file is malformed or nested too deeply."))?;
    if v.get("version").and_then(Value::as_str) != Some("2.21") {
        return Err(error(
            "This importer supports .pen 2.21. Keep the original and export a supported version from pen.dev.",
        ));
    }
    if !v["children"].is_array() {
        return Err(error("The .pen file has no layer list."));
    }
    Ok(v)
}
fn comments(bytes: &[u8]) -> Result<Vec<u8>, ImportError> {
    let mut out = bytes.to_vec();
    let (mut i, mut string, mut escaped) = (0, false, false);
    while i < out.len() {
        if string {
            if escaped {
                escaped = false
            } else if out[i] == b'\\' {
                escaped = true
            } else if out[i] == b'"' {
                string = false
            }
            i += 1;
            continue;
        }
        if out[i] == b'"' {
            string = true;
            i += 1;
            continue;
        }
        if out.get(i..i + 2) == Some(b"//") {
            out[i] = b' ';
            out[i + 1] = b' ';
            i += 2;
            while i < out.len() && out[i] != b'\n' {
                out[i] = b' ';
                i += 1
            }
            continue;
        }
        if out.get(i..i + 2) == Some(b"/*") {
            out[i] = b' ';
            out[i + 1] = b' ';
            i += 2;
            let mut ended = false;
            while i < out.len() {
                if out.get(i..i + 2) == Some(b"*/") {
                    out[i] = b' ';
                    out[i + 1] = b' ';
                    i += 2;
                    ended = true;
                    break;
                }
                if out[i] != b'\n' {
                    out[i] = b' '
                }
                i += 1
            }
            if !ended {
                return Err(error("An input comment is incomplete."));
            }
            continue;
        }
        i += 1;
    }
    Ok(out)
}
pub fn index(root: &Value) -> Result<BTreeMap<String, &Value>, ImportError> {
    fn collect<'a>(
        v: &'a Value,
        out: &mut BTreeMap<String, &'a Value>,
        count: &mut usize,
        depth: usize,
    ) -> Result<(), ImportError> {
        if depth > MAX_DEPTH {
            return Err(error("The .pen tree exceeds the depth limit."));
        }
        *count += 1;
        if *count > MAX_NODES {
            return Err(error("The .pen file exceeds the layer limit."));
        }
        if let Some(id) = v.get("id") {
            let id = id
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 512 && !s.contains('/'))
                .ok_or_else(|| error("A layer id is invalid."))?;
            if out.insert(id.to_string(), v).is_some() {
                return Err(error("Layer ids must be unique."));
            }
        }
        if let Some(children) = v.get("children") {
            for c in children
                .as_array()
                .ok_or_else(|| error("A layer's children must be a list."))?
            {
                collect(c, out, count, depth + 1)?
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    let mut count = 0;
    for child in root["children"].as_array().expect("read validated") {
        collect(child, &mut out, &mut count, 0)?
    }
    Ok(out)
}
pub fn defaults(root: &Value) -> Theme {
    root["themes"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((k.clone(), v.as_array()?.first()?.as_str()?.to_string())))
        .collect()
}
pub fn theme(parent: &Theme, node: &Value) -> Result<Theme, ImportError> {
    let mut t = parent.clone();
    if let Some(v) = node.get("theme") {
        for (k, v) in v
            .as_object()
            .ok_or_else(|| error("A layer theme must be an object."))?
        {
            t.insert(
                k.clone(),
                v.as_str()
                    .ok_or_else(|| error("A theme value must be a string."))?
                    .to_string(),
            );
        }
    }
    Ok(t)
}
pub fn resolve(root: &Value, v: &Value, theme: &Theme, depth: usize) -> Result<Value, ImportError> {
    if depth > 32 {
        return Err(error("A variable is cyclic or too deeply aliased."));
    }
    match v {
        Value::String(s) if s.starts_with('$') => {
            let variable = root["variables"]
                .get(&s[1..])
                .ok_or_else(|| error("An imported variable is missing."))?;
            let mut value = variable
                .get("value")
                .ok_or_else(|| error("A variable has no value."))?;
            if let Some(entries) = value.as_array() {
                value = entries
                    .iter()
                    .rev()
                    .find(|entry| {
                        entry.get("theme").is_none_or(|t| {
                            t.as_object().is_some_and(|m| {
                                m.iter().all(|(k, v)| {
                                    v.as_str()
                                        .is_some_and(|s| theme.get(k).is_some_and(|q| q == s))
                                })
                            })
                        })
                    })
                    .and_then(|e| e.get("value"))
                    .ok_or_else(|| error("No variable value matches the imported theme."))?;
            }
            resolve(root, value, theme, depth + 1)
        }
        Value::Array(a) => Ok(Value::Array(
            a.iter()
                .map(|v| resolve(root, v, theme, depth + 1))
                .collect::<Result<_, _>>()?,
        )),
        Value::Object(m) => Ok(Value::Object(
            m.iter()
                .map(|(k, v)| Ok((k.clone(), resolve(root, v, theme, depth + 1)?)))
                .collect::<Result<_, ImportError>>()?,
        )),
        _ => Ok(v.clone()),
    }
}
pub fn prop(
    root: &Value,
    node: &Value,
    key: &str,
    default: Value,
    theme: &Theme,
) -> Result<Value, ImportError> {
    resolve(root, node.get(key).unwrap_or(&default), theme, 0)
}
pub fn number(v: &Value) -> Result<f64, ImportError> {
    v.as_f64()
        .filter(|n| n.is_finite() && n.abs() <= 1e9)
        .ok_or_else(|| error("An imported numeric value is invalid."))
}
pub fn length(v: &Value) -> Result<Value, ImportError> {
    if let Some(s) = v.as_str() {
        for (from, to) in [("fit_content", "hug"), ("fill_container", "fill")] {
            if s == from {
                return Ok(json!(to));
            }
            if let Some(v) = s
                .strip_prefix(&format!("{from}("))
                .and_then(|s| s.strip_suffix(')'))
            {
                let n = v
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite() && *n >= 0.)
                    .ok_or_else(|| error("A sizing fallback is invalid."))?;
                return Ok(json!({to:n}));
            }
        }
        return Err(error("The imported sizing mode is not supported."));
    }
    let n = number(v)?;
    if n < 0. {
        return Err(error("An imported size cannot be negative."));
    }
    Ok(json!(n))
}
