//! The bytes: 2-space indent, LF, one key or item per line, a trailing newline. Nothing here chooses; there is
//! one way to print each value.

use crate::tree::Tree;

/// `tree` as the text of a `.vellum` file.
pub(crate) fn print(tree: &Tree) -> Vec<u8> {
    let mut out = String::new();
    value(tree, 0, &mut out);
    out.push('\n');
    out.into_bytes()
}

fn indent(level: usize, out: &mut String) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn value(tree: &Tree, level: usize, out: &mut String) {
    match tree {
        Tree::Null => out.push_str("null"),
        Tree::Bool(true) => out.push_str("true"),
        Tree::Bool(false) => out.push_str("false"),
        Tree::Int(n) => out.push_str(&n.to_string()),
        Tree::Uint(n) => out.push_str(&n.to_string()),
        Tree::Float(x) => float(*x, out),
        Tree::Text(text) => string(text, out),
        Tree::Array(items) if items.is_empty() => out.push_str("[]"),
        Tree::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                indent(level + 1, out);
                value(item, level + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            indent(level, out);
            out.push(']');
        }
        Tree::Object(entries) if entries.is_empty() => out.push_str("{}"),
        Tree::Object(entries) => {
            out.push_str("{\n");
            for (i, (key, item)) in entries.iter().enumerate() {
                indent(level + 1, out);
                string(key, out);
                out.push_str(": ");
                value(item, level + 1, out);
                out.push_str(if i + 1 < entries.len() { ",\n" } else { "\n" });
            }
            indent(level, out);
            out.push('}');
        }
    }
}

/// A whole number prints without a fraction. Anything else prints as the shortest text that reads back as the
/// same double, which is the same on every platform. The caller has already refused values that are not finite.
fn float(x: f64, out: &mut String) {
    if x == 0.0 {
        out.push('0');
    } else if x.fract() == 0.0 && x.abs() < 1.0e15 {
        out.push_str(&(x as i64).to_string());
    } else {
        // Finite by now, so `to_string` cannot produce `null`.
        out.push_str(&serde_json::to_string(&x).unwrap_or_else(|_| "0".to_owned()));
    }
}

/// A JSON string with only the escapes JSON requires: the quote, the backslash and control characters.
fn string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
