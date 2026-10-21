//! Vellum shader dialect v1: finite, straight-line expressions. No renderer or I/O dependency.
//! A structural admission pass runs before any vendor compiler, including for untrusted resources.
use crate::ports::ShaderError;
use std::collections::BTreeSet;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniformKind {
    Number,
    Boolean,
    Color,
    Vector(u8),
    Content,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniformInfo {
    pub name: String,
    pub kind: UniformKind,
}
fn unsupported(message: &str) -> ShaderError {
    ShaderError::Unsupported {
        message: message.into(),
    }
}
fn invalid(message: &str) -> ShaderError {
    ShaderError::Invalid {
        message: message.into(),
    }
}
fn kind(t: &str) -> Option<UniformKind> {
    Some(match t {
        "float" | "half" => UniformKind::Number,
        "int" => UniformKind::Boolean,
        "float2" | "half2" => UniformKind::Vector(2),
        "float3" | "half3" => UniformKind::Vector(3),
        "float4" | "half4" => UniformKind::Vector(4),
        "shader" => UniformKind::Content,
        _ => return None,
    })
}
fn ident(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
/// Bound source, token count, nesting, statement count, call graph and per-pixel work.
/// `// @color NAME` marks a float4 as an unpremultiplied sRGB colour control.
pub fn inspect(src: &str) -> Result<Vec<UniformInfo>, ShaderError> {
    if src.len() > 16 * 1024 {
        return Err(unsupported("Source exceeds 16 KiB"));
    }
    if src.trim().is_empty() {
        return Err(ShaderError::Empty);
    }
    let mut cleaned = String::new();
    let mut colors = BTreeSet::new();
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            let mut comment = String::new();
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
                comment.push(c);
            }
            if let Some(name) = comment.trim().strip_prefix("@color ") {
                colors.insert(name.trim().to_owned());
            }
            cleaned.push('\n');
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut last = ' ';
            let mut closed = false;
            for c in chars.by_ref() {
                if last == '*' && c == '/' {
                    closed = true;
                    break;
                }
                last = c;
            }
            if !closed {
                return Err(invalid("Unclosed comment"));
            }
            cleaned.push(' ');
        } else if c.is_ascii_alphanumeric() || c == '_' || c.is_ascii_whitespace() {
            cleaned.push(c);
        } else if ".,+-*/%=!<>?:&|(){};".contains(c) {
            cleaned.push(' ');
            cleaned.push(c);
            if let Some(next) = chars.peek().copied()
                && (("=!<>+-*/%".contains(c) && next == '=') || (c == next && "+-&|".contains(c)))
            {
                cleaned.push(next);
                chars.next();
            }
            cleaned.push(' ');
        } else {
            return Err(unsupported(
                "Preprocessors, arrays and non-dialect syntax are not supported",
            ));
        }
    }
    let t: Vec<_> = cleaned.split_whitespace().collect();
    if t.len() > 2048 {
        return Err(unsupported("Program exceeds 2048 tokens"));
    }
    let samples = t.iter().filter(|t| **t == "eval").count();
    if samples > 1 {
        return Err(unsupported("At most one content sample is allowed"));
    }
    for (i, token) in t.iter().enumerate() {
        if *token == "eval" && t.get(i + 1..i + 4) != Some(&["(", "p", ")"][..]) {
            return Err(unsupported("content.eval must sample p without an offset"));
        }
    }
    let mut at = 0;
    let mut uniforms = Vec::new();
    let mut names = BTreeSet::new();
    while t.get(at) == Some(&"uniform") {
        let ty = *t
            .get(at + 1)
            .ok_or_else(|| invalid("Missing uniform type"))?;
        let name = *t
            .get(at + 2)
            .ok_or_else(|| invalid("Missing uniform name"))?;
        let mut k = kind(ty).ok_or_else(|| unsupported("Unsupported uniform type"))?;
        if !ident(name)
            || !names.insert(name.to_owned())
            || ["main", "p"].contains(&name)
            || t.get(at + 3) != Some(&";")
        {
            return Err(invalid("Invalid or duplicate uniform declaration"));
        }
        if name == "size" && k != UniformKind::Vector(2) {
            return Err(invalid("size must be float2"));
        }
        if k == UniformKind::Content && name != "content" {
            return Err(unsupported("Only the content child shader is supported"));
        }
        if colors.contains(name) {
            if k != UniformKind::Vector(4) {
                return Err(invalid("A colour uniform must be float4"));
            }
            k = UniformKind::Color;
        }
        uniforms.push(UniformInfo {
            name: name.into(),
            kind: k,
        });
        at += 4;
        if uniforms.len() > 32 {
            return Err(unsupported("At most 32 uniforms are allowed"));
        }
    }
    if t.get(at..at + 7) != Some(&["half4", "main", "(", "float2", "p", ")", "{"][..]) {
        return Err(unsupported(
            "Expected half4 main(float2 p); GLSL import is not supported",
        ));
    }
    at += 7;
    let mut returned = false;
    let mut statements = 0;
    while t.get(at) != Some(&"}") {
        if at >= t.len() || returned {
            return Err(invalid("Expected a final return"));
        }
        if t[at] == "return" {
            returned = true;
            at += 1;
        } else {
            if kind(t[at]).is_none() || t[at] == "shader" {
                return Err(unsupported(
                    "Only local declarations and return are supported; no loops or helpers",
                ));
            }
            let name = *t.get(at + 1).ok_or_else(|| invalid("Missing local name"))?;
            if !ident(name)
                || !names.insert(name.to_owned())
                || ["p", "main"].contains(&name)
                || t.get(at + 2) != Some(&"=")
            {
                return Err(invalid("Expected a unique initialized local"));
            }
            at += 3;
        }
        let start = at;
        let mut depth = 0;
        while let Some(token) = t.get(at) {
            if *token == ";" {
                break;
            }
            if *token == "(" {
                depth += 1;
                if depth > 16 {
                    return Err(unsupported("Expression nesting exceeds 16"));
                }
            }
            if *token == ")" {
                if depth == 0 {
                    return Err(invalid("Unbalanced parentheses"));
                }
                depth -= 1;
            }
            if [
                "{", "}", "=", "++", "--", "+=", "-=", "*=", "/=", "%=", "return", "uniform",
                "while", "for", "do", "if", "switch", "discard",
            ]
            .contains(token)
            {
                return Err(unsupported(
                    "Control flow and assignments are not supported",
                ));
            }
            if t.get(at + 1) == Some(&"(")
                && ident(token)
                && ![
                    "float",
                    "float2",
                    "float3",
                    "float4",
                    "half",
                    "half2",
                    "half3",
                    "half4",
                    "int",
                    "abs",
                    "min",
                    "max",
                    "clamp",
                    "mix",
                    "step",
                    "smoothstep",
                    "floor",
                    "ceil",
                    "fract",
                    "mod",
                    "sin",
                    "cos",
                    "sqrt",
                    "dot",
                    "length",
                    "normalize",
                    "saturate",
                    "eval",
                ]
                .contains(token)
            {
                return Err(unsupported("Function is outside the bounded dialect"));
            }
            if *token == "eval" && (at < 2 || t[at - 1] != "." || t[at - 2] != "content") {
                return Err(unsupported("Only content.eval may sample a child"));
            }
            at += 1;
        }
        if at == start || depth != 0 || t.get(at) != Some(&";") {
            return Err(invalid("Expected an expression ending in a semicolon"));
        }
        at += 1;
        statements += 1;
        if statements > 64 {
            return Err(unsupported("At most 64 statements are allowed"));
        }
    }
    if !returned || at + 1 != t.len() {
        return Err(invalid("Expected one entry point and one final return"));
    }
    Ok(uniforms)
}
/// Visible file dependencies of one resolved occurrence, shared by hosts and export preparation.
pub fn sources(node: &vellum_types::Node) -> BTreeSet<String> {
    use vellum_types::{Effect, Paint};
    node.fills()
        .iter()
        .chain(node.stroke().into_iter().flat_map(|s| &s.paints))
        .filter_map(|p| match p {
            Paint::Shader(s) if s.visible => Some(s.src.clone()),
            _ => None,
        })
        .chain(node.effects().iter().filter_map(|e| match e {
            Effect::Shader(s) if s.visible => Some(s.src.clone()),
            _ => None,
        }))
        .collect()
}
