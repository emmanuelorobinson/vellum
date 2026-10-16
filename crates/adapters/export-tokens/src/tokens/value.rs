//! Values as stylesheets and token files write them: colours (hex, `rgb()`, `hsl()`, `oklch()`), lengths, strings.
//!
//! Writing and reading agree on one rule so a string never turns into something else: a string is written bare
//! only when reading it back would give the same string; otherwise it is quoted.

use vellum_types::Color;

/// What a stylesheet value is, as far as a token is concerned.
#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    Color(Color),
    Number(f64),
    Boolean(bool),
    /// `var(--name)`: the property name without its dashes.
    Var(String),
    Text(String),
}

/// Reads a value (comments and `!important` already removed).
pub fn classify(text: &str) -> Parsed {
    let text = text.trim();
    if let Some(inner) = unquote(text) {
        return Parsed::Text(inner);
    }
    match text {
        "true" => return Parsed::Boolean(true),
        "false" => return Parsed::Boolean(false),
        _ => {}
    }
    if let Some(color) = parse_color(text) {
        return Parsed::Color(color);
    }
    if let Some(number) = parse_length(text) {
        return Parsed::Number(number);
    }
    if let Some(name) = parse_var(text) {
        return Parsed::Var(name);
    }
    Parsed::Text(text.to_owned())
}

/// `var(--name)` and nothing else.
fn parse_var(text: &str) -> Option<String> {
    let inner = text.strip_prefix("var(")?.strip_suffix(')')?.trim();
    let name = inner.strip_prefix("--")?;
    (!name.is_empty()
        && !name.contains(|c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')')))
    .then(|| name.to_owned())
}

/// A number with no unit, `px`, or `rem` (sixteen pixels).
pub fn parse_length(text: &str) -> Option<f64> {
    let lower = text.to_ascii_lowercase();
    let (digits, factor) = if let Some(d) = lower.strip_suffix("rem") {
        (d, 16.0)
    } else if let Some(d) = lower.strip_suffix("px") {
        (d, 1.0)
    } else {
        (lower.as_str(), 1.0)
    };
    number(digits).map(|n| n * factor)
}

fn number(text: &str) -> Option<f64> {
    let first = text.chars().next()?;
    if !(first.is_ascii_digit() || matches!(first, '.' | '-' | '+'))
        || !text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
    {
        return None;
    }
    text.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// A number as written: the shortest text that reads back as the same number.
pub fn format_number(n: f64) -> String {
    if n == 0.0 {
        "0".to_owned()
    } else {
        n.to_string()
    }
}

fn byte(x: f64) -> u8 {
    (x * 255.0).round().clamp(0.0, 255.0) as u8
}

fn hex_color(digits: &str) -> Option<Color> {
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let pair = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    let nibble = |i: usize| u8::from_str_radix(&digits[i..=i], 16).ok().map(|n| n * 17);
    match digits.len() {
        3 => Some(Color::rgb(nibble(0)?, nibble(1)?, nibble(2)?)),
        4 => Some(Color::rgba(nibble(0)?, nibble(1)?, nibble(2)?, nibble(3)?)),
        6 => Some(Color::rgb(pair(0)?, pair(2)?, pair(4)?)),
        8 => Some(Color::rgba(pair(0)?, pair(2)?, pair(4)?, pair(6)?)),
        _ => None,
    }
}

/// A component: a number, or a percentage of `full`.
fn component(text: &str, full: f64) -> Option<f64> {
    match text.strip_suffix('%') {
        Some(percent) => number(percent).map(|n| n / 100.0 * full),
        None => number(text),
    }
}

fn alpha(text: Option<&str>) -> Option<f64> {
    match text {
        None => Some(1.0),
        Some(t) => component(t, 1.0).map(|a| a.clamp(0.0, 1.0)),
    }
}

/// The arguments of `name( … )`, split at commas, spaces and the `/` before alpha.
fn arguments<'a>(text: &'a str, names: &[&str]) -> Option<Vec<&'a str>> {
    let open = text.find('(')?;
    let name = text[..open].trim();
    if !names.iter().any(|n| name.eq_ignore_ascii_case(n)) {
        return None;
    }
    let inner = text[open + 1..].strip_suffix(')')?;
    Some(
        inner
            .split(|c: char| c.is_whitespace() || matches!(c, ',' | '/'))
            .filter(|part| !part.is_empty())
            .collect(),
    )
}

fn hsl(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let h = h.rem_euclid(360.0);
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = chroma * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - chroma / 2.0;
    let (r, g, b) = match h {
        h if h < 60.0 => (chroma, x, 0.0),
        h if h < 120.0 => (x, chroma, 0.0),
        h if h < 180.0 => (0.0, chroma, x),
        h if h < 240.0 => (0.0, x, chroma),
        h if h < 300.0 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    (r + m, g + m, b + m)
}

fn oklch(l: f64, c: f64, h: f64) -> (f64, f64, f64) {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
    let (l3, m3, s3) = (l_.powi(3), m_.powi(3), s_.powi(3));
    let linear = [
        4.076_741_662_1 * l3 - 3.307_711_591_3 * m3 + 0.230_969_929_2 * s3,
        -1.268_438_004_6 * l3 + 2.609_757_401_1 * m3 - 0.341_319_396_5 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_614_7 * m3 + 1.707_614_701_0 * s3,
    ];
    let encode = |c: f64| {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.003_130_8 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    (encode(linear[0]), encode(linear[1]), encode(linear[2]))
}

fn hue(text: &str) -> Option<f64> {
    number(text.strip_suffix("deg").unwrap_or(text))
}

/// A colour in any spelling a design system's stylesheet uses.
pub fn parse_color(text: &str) -> Option<Color> {
    let text = text.trim();
    if let Some(digits) = text.strip_prefix('#') {
        return hex_color(digits);
    }
    if text.eq_ignore_ascii_case("transparent") {
        return Some(Color::rgba(0, 0, 0, 0));
    }
    if let Some(args) = arguments(text, &["rgb", "rgba"]) {
        let [r, g, b, rest @ ..] = args.as_slice() else {
            return None;
        };
        let channel = |t: &str| component(t, 255.0).map(|n| n.clamp(0.0, 255.0).round() as u8);
        return Some(Color::rgba(
            channel(r)?,
            channel(g)?,
            channel(b)?,
            byte(alpha(rest.first().copied())?),
        ));
    }
    if let Some(args) = arguments(text, &["hsl", "hsla"]) {
        let [h, s, l, rest @ ..] = args.as_slice() else {
            return None;
        };
        let (r, g, b) = hsl(
            hue(h)?,
            component(s, 1.0)?.clamp(0.0, 1.0),
            component(l, 1.0)?.clamp(0.0, 1.0),
        );
        return Some(Color::rgba(
            byte(r),
            byte(g),
            byte(b),
            byte(alpha(rest.first().copied())?),
        ));
    }
    if let Some(args) = arguments(text, &["oklch"]) {
        let [l, c, h, rest @ ..] = args.as_slice() else {
            return None;
        };
        let (r, g, b) = oklch(component(l, 1.0)?, component(c, 0.4)?, hue(h)?);
        return Some(Color::rgba(
            byte(r),
            byte(g),
            byte(b),
            byte(alpha(rest.first().copied())?),
        ));
    }
    None
}

/// `"text"` with CSS escapes, and nothing after the closing quote.
pub fn unquote(text: &str) -> Option<String> {
    let mut chars = text.chars();
    let quote = chars.next().filter(|c| matches!(c, '"' | '\''))?;
    let mut out = String::new();
    loop {
        match chars.next()? {
            c if c == quote => return chars.next().is_none().then_some(out),
            '\\' => {
                let first = chars.clone().next()?;
                if first.is_ascii_hexdigit() {
                    let mut digits = String::new();
                    while digits.len() < 6
                        && chars.clone().next().is_some_and(|c| c.is_ascii_hexdigit())
                    {
                        digits.push(chars.next()?);
                    }
                    if chars.clone().next() == Some(' ') {
                        chars.next();
                    }
                    out.push(char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?);
                } else {
                    out.push(chars.next()?);
                }
            }
            c => out.push(c),
        }
    }
}

/// `text` as a quoted CSS string.
pub fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\{:X} ", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A string as it is written after `--name:`.
pub fn format_string(text: &str) -> String {
    let balanced = {
        let mut depth = 0i32;
        text.chars().all(|c| {
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth -= 1,
                _ => {}
            }
            depth >= 0
        }) && depth == 0
    };
    let bare = !text.is_empty()
        && text == text.trim()
        && balanced
        && !text.contains("/*")
        && !text
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '\'' | '\\' | ';' | '{' | '}'))
        && !text.to_ascii_lowercase().ends_with("!important")
        && classify(text) == Parsed::Text(text.to_owned());
    if bare { text.to_owned() } else { quote(text) }
}
