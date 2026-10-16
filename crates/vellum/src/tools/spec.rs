//! Node specs: how `insert_nodes` and `replace_nodes` read the nodes they are given (ARCHITECTURE §7.3).
//!
//! A spec is text in one of a few formats, each read by a parser in [`PARSERS`] into the file format's node JSON
//! (without ids, which the tool fills in). The file format stays the only model: a parser only spells it
//! differently, so whatever it produces is checked exactly as JSON would be.
//!
//! `jsx` is a compact, tag-based spelling that costs an agent fewer tokens than JSON for a tree:
//!
//! ```text
//! <frame name="Card" width={320} height="hug" layout="column" gap={8}>
//!   <text width="fill" height="hug" font="Inter" size={20} weight={600}>Today</text>
//!   <instance componentId="Button"/>
//! </frame>
//! ```
//!
//! The tag is the node's `type`; an attribute is a property, `"…"` a string and `{…}` any JSON value; a bare
//! attribute is `true`. Elements inside are `children`. Text inside a `text` element becomes its content, one
//! run per piece, with the run properties (`font`, `size`, `weight`, …) taken from the element and from `span`
//! elements around a piece. Text inside a `note`, `context` or `prompt` is its `content`.

use std::fmt;

use serde_json::{Map, Value, json};

/// Why a spec could not be read: where, and what was wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError {
    /// The line, counting from 1.
    pub line: u32,
    /// The column, counting from 1.
    pub column: u32,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

/// Reads one spec format into node JSON.
pub trait SpecParser {
    /// The format's name, as the `format` field gives it.
    fn name(&self) -> &'static str;
    /// The nodes in `text`, in order.
    fn parse(&self, text: &str) -> Result<Vec<Value>, SpecError>;
}

/// Every format, by name.
pub const PARSERS: &[&dyn SpecParser] = &[&JsonSpec, &JsxSpec];

/// The names of the formats there are parsers for.
pub fn formats() -> impl Iterator<Item = &'static str> {
    PARSERS.iter().map(|parser| parser.name())
}

/// Reads `text` as `format`. `None` when there is no parser for that format.
pub fn parse_spec(format: &str, text: &str) -> Option<Result<Vec<Value>, SpecError>> {
    PARSERS
        .iter()
        .find(|parser| parser.name() == format)
        .map(|parser| parser.parse(text))
}

/// A node, or a list of nodes, as the file writes them.
struct JsonSpec;

impl SpecParser for JsonSpec {
    fn name(&self) -> &'static str {
        "json"
    }

    fn parse(&self, text: &str) -> Result<Vec<Value>, SpecError> {
        let value: Value = serde_json::from_str(text).map_err(|e| SpecError {
            line: u32::try_from(e.line()).unwrap_or(u32::MAX),
            column: u32::try_from(e.column()).unwrap_or(u32::MAX),
            message: e.to_string(),
        })?;
        match value {
            Value::Array(nodes) => Ok(nodes),
            node @ Value::Object(_) => Ok(vec![node]),
            _ => Err(SpecError {
                line: 1,
                column: 1,
                message: "a JSON spec is a node object or a list of them".into(),
            }),
        }
    }
}

struct JsxSpec;

impl SpecParser for JsxSpec {
    fn name(&self) -> &'static str {
        "jsx"
    }

    fn parse(&self, text: &str) -> Result<Vec<Value>, SpecError> {
        let mut reader = Reader { text, at: 0 };
        let mut nodes = Vec::new();
        loop {
            reader.skip_space();
            if reader.done() {
                break;
            }
            if !reader.looking_at("<") {
                return Err(reader.error("expected an element such as <frame>"));
            }
            nodes.push(reader.element(0)?.into_node()?);
        }
        if nodes.is_empty() {
            return Err(reader.error("the spec has no elements"));
        }
        Ok(nodes)
    }
}

/// The deepest a spec nests: deeper than any design, shallow enough that reading it cannot overflow the stack.
const MAX_DEPTH: usize = 64;

/// The text properties that belong to a run rather than to the text node.
const RUN_PROPERTIES: &[&str] = &[
    "font",
    "weight",
    "style",
    "size",
    "lineHeight",
    "letterSpacing",
    "underline",
    "strikethrough",
    "link",
];

/// Nodes whose `content` is a plain string.
const PLAIN_TEXT: &[&str] = &["note", "context", "prompt"];

enum Content {
    Element(Element),
    Text(String, Position),
}

#[derive(Clone, Copy)]
struct Position {
    line: u32,
    column: u32,
}

struct Element {
    tag: String,
    at: Position,
    attributes: Map<String, Value>,
    content: Vec<Content>,
}

impl Element {
    fn fail(at: Position, message: impl Into<String>) -> SpecError {
        SpecError {
            line: at.line,
            column: at.column,
            message: message.into(),
        }
    }

    /// The node this element spells.
    fn into_node(self) -> Result<Value, SpecError> {
        if self.tag == "span" {
            return Err(Self::fail(self.at, "a span goes inside a text element"));
        }
        let mut node = Map::new();
        node.insert("type".into(), json!(self.tag));
        let is_text = self.tag == "text";
        let mut run = Map::new();
        for (key, value) in self.attributes {
            if key == "type" {
                return Err(Self::fail(self.at, "the tag is the type; leave out `type`"));
            }
            if is_text && RUN_PROPERTIES.contains(&key.as_str()) {
                run.insert(key, value);
            } else {
                node.insert(key, value);
            }
        }
        if is_text {
            let mut runs = Vec::new();
            text_runs(self.content, &run, &mut runs)?;
            if !runs.is_empty() {
                if node.contains_key("content") {
                    return Err(Self::fail(
                        self.at,
                        "give a text element its content as text inside it or as `content`, not both",
                    ));
                }
                node.insert("content".into(), Value::Array(runs));
            }
            return Ok(Value::Object(node));
        }
        let plain = PLAIN_TEXT.contains(&self.tag.as_str());
        let mut children = Vec::new();
        let mut words = String::new();
        for item in self.content {
            match item {
                Content::Element(child) => children.push(child.into_node()?),
                Content::Text(text, _) if plain => {
                    if !words.is_empty() {
                        words.push(' ');
                    }
                    words.push_str(&text);
                }
                Content::Text(_, at) => {
                    return Err(Self::fail(
                        at,
                        format!(
                            "a {} does not take text; put it in a <text> element",
                            self.tag
                        ),
                    ));
                }
            }
        }
        if plain && !words.is_empty() {
            node.insert("content".into(), json!(words));
        }
        if !children.is_empty() {
            node.insert("children".into(), Value::Array(children));
        }
        Ok(Value::Object(node))
    }
}

/// The runs of a text element's content: each piece of text with the run properties around it.
fn text_runs(
    content: Vec<Content>,
    inherited: &Map<String, Value>,
    runs: &mut Vec<Value>,
) -> Result<(), SpecError> {
    for item in content {
        match item {
            Content::Text(text, _) => {
                let mut run = inherited.clone();
                run.insert("text".into(), json!(text));
                runs.push(Value::Object(run));
            }
            Content::Element(span) if span.tag == "span" => {
                let mut style = inherited.clone();
                for (key, value) in span.attributes {
                    if !RUN_PROPERTIES.contains(&key.as_str()) {
                        return Err(Element::fail(
                            span.at,
                            format!(
                                "a span sets run properties ({}), not {key}",
                                RUN_PROPERTIES.join(", ")
                            ),
                        ));
                    }
                    style.insert(key, value);
                }
                text_runs(span.content, &style, runs)?;
            }
            Content::Element(other) => {
                return Err(Element::fail(
                    other.at,
                    format!("a text element holds text and spans, not a {}", other.tag),
                ));
            }
        }
    }
    Ok(())
}

struct Reader<'a> {
    text: &'a str,
    /// A byte offset into `text`.
    at: usize,
}

impl Reader<'_> {
    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    fn done(&self) -> bool {
        self.at >= self.text.len()
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        Some(c)
    }

    fn looking_at(&self, s: &str) -> bool {
        self.rest().starts_with(s)
    }

    fn position(&self) -> Position {
        let before = &self.text[..self.at];
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        Position {
            line: u32::try_from(line).unwrap_or(u32::MAX),
            column: u32::try_from(column).unwrap_or(u32::MAX),
        }
    }

    fn error(&self, message: impl Into<String>) -> SpecError {
        Element::fail(self.position(), message)
    }

    fn expect(&mut self, s: &str) -> Result<(), SpecError> {
        if self.looking_at(s) {
            self.at += s.len();
            Ok(())
        } else {
            Err(self.error(format!("expected `{s}`")))
        }
    }

    fn skip_space(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }
            if self.looking_at("<!--") {
                match self.rest().find("-->") {
                    Some(end) => self.at += end + 3,
                    None => self.at = self.text.len(),
                }
            } else {
                break;
            }
        }
    }

    fn name(&mut self) -> Result<String, SpecError> {
        let start = self.at;
        if !self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            return Err(self.error("expected a name"));
        }
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            self.bump();
        }
        Ok(self.text[start..self.at].to_owned())
    }

    fn element(&mut self, depth: usize) -> Result<Element, SpecError> {
        if depth > MAX_DEPTH {
            return Err(self.error(format!("elements nest at most {MAX_DEPTH} deep")));
        }
        let at = self.position();
        self.expect("<")?;
        let tag = self.name()?;
        let mut attributes = Map::new();
        loop {
            self.skip_space();
            if self.looking_at("/>") {
                self.at += 2;
                return Ok(Element {
                    tag,
                    at,
                    attributes,
                    content: Vec::new(),
                });
            }
            if self.looking_at(">") {
                self.at += 1;
                break;
            }
            let key_at = self.position();
            let key = self.name()?;
            let value = if self.looking_at("=") {
                self.at += 1;
                self.value()?
            } else {
                Value::Bool(true)
            };
            if attributes.insert(key.clone(), value).is_some() {
                return Err(Element::fail(key_at, format!("`{key}` is given twice")));
            }
        }
        let mut content = Vec::new();
        loop {
            if self.done() {
                return Err(Element::fail(at, format!("<{tag}> is never closed")));
            }
            if self.looking_at("</") {
                self.at += 2;
                let close_at = self.position();
                let closing = self.name()?;
                if closing != tag {
                    return Err(Element::fail(
                        close_at,
                        format!("</{closing}> closes <{tag}>; expected </{tag}>"),
                    ));
                }
                self.skip_space();
                self.expect(">")?;
                return Ok(Element {
                    tag,
                    at,
                    attributes,
                    content,
                });
            }
            if self.looking_at("<!--") {
                self.skip_space();
                continue;
            }
            if self.looking_at("<") {
                content.push(Content::Element(self.element(depth + 1)?));
                continue;
            }
            let text_at = self.position();
            let start = self.at;
            while !self.done() && !self.looking_at("<") {
                self.bump();
            }
            let raw = &self.text[start..self.at];
            // Whitespace between elements is layout of the spec, not text; runs of it inside text are one space.
            let words = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if !words.is_empty() {
                content.push(Content::Text(unescape(&words), text_at));
            }
        }
    }

    fn value(&mut self) -> Result<Value, SpecError> {
        match self.peek() {
            Some(quote @ ('"' | '\'')) => {
                self.bump();
                let mut out = String::new();
                loop {
                    match self.bump() {
                        None => return Err(self.error("this string is never closed")),
                        Some(c) if c == quote => break,
                        Some('\\') => match self.bump() {
                            Some('n') => out.push('\n'),
                            Some('t') => out.push('\t'),
                            Some(c) => out.push(c),
                            None => return Err(self.error("this string is never closed")),
                        },
                        Some(c) => out.push(c),
                    }
                }
                Ok(Value::String(unescape(&out)))
            }
            Some('{') => {
                self.bump();
                let mut stream =
                    serde_json::Deserializer::from_str(self.rest()).into_iter::<Value>();
                let value = match stream.next() {
                    Some(Ok(value)) => value,
                    Some(Err(e)) => {
                        let at = self.position();
                        let line = u32::try_from(e.line()).unwrap_or(u32::MAX);
                        let column = u32::try_from(e.column()).unwrap_or(u32::MAX);
                        return Err(SpecError {
                            line: at.line + line - 1,
                            column: if line == 1 {
                                at.column + column - 1
                            } else {
                                column
                            },
                            message: format!("the value in braces is not JSON: {e}"),
                        });
                    }
                    None => return Err(self.error("expected a JSON value in the braces")),
                };
                self.at += stream.byte_offset();
                self.skip_space();
                self.expect("}")?;
                Ok(value)
            }
            _ => Err(self.error(
                "an attribute value is a \"string\" or a {JSON value}, such as width={320}",
            )),
        }
    }
}

/// The five XML entities, so text can hold `<` and `&`.
fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jsx(text: &str) -> Result<Vec<Value>, SpecError> {
        parse_spec("jsx", text).expect("a jsx parser")
    }

    #[test]
    fn a_tree_reads_as_node_json() {
        let nodes =
            jsx(r#"<frame name="A" width={10} clip><rect width={1} height="hug"/></frame>"#)
                .unwrap();
        assert_eq!(
            nodes,
            vec![
                json!({"type": "frame", "name": "A", "width": 10, "clip": true,
                        "children": [{"type": "rect", "width": 1, "height": "hug"}]})
            ]
        );
    }

    #[test]
    fn text_becomes_runs_with_the_run_properties() {
        let nodes =
            jsx(r#"<text width="fill" font="Inter" size={14}>Hello <span weight={700}>you</span> &amp; me</text>"#)
                .unwrap();
        assert_eq!(
            nodes[0],
            json!({"type": "text", "width": "fill", "content": [
                {"text": "Hello", "font": "Inter", "size": 14},
                {"text": "you", "font": "Inter", "size": 14, "weight": 700},
                {"text": "& me", "font": "Inter", "size": 14}]})
        );
    }

    #[test]
    fn notes_take_plain_content_and_several_roots_are_a_list() {
        let nodes = jsx("<note>Check  the\n copy</note>\n<rect/>").unwrap();
        assert_eq!(
            nodes[0],
            json!({"type": "note", "content": "Check the copy"})
        );
        assert_eq!(nodes[1], json!({"type": "rect"}));
    }

    #[test]
    fn mistakes_name_where_they_are() {
        let error = jsx("<frame>\n  <rect width=10/>\n</frame>").unwrap_err();
        assert_eq!((error.line, error.column), (2, 15));
        let error = jsx("<frame>\n</group>").unwrap_err();
        assert_eq!(error.line, 2);
        assert!(error.message.contains("</frame>"));
        let error = jsx("<frame>").unwrap_err();
        assert!(error.message.contains("never closed"));
        let error = jsx("<rect>words</rect>").unwrap_err();
        assert!(error.message.contains("<text>"));
        let error = jsx("<rect a={1} a={2}/>").unwrap_err();
        assert!(error.message.contains("twice"));
        let error = jsx(r#"<rect fills={[{"type": }]}/>"#).unwrap_err();
        assert_eq!(error.line, 1);
        assert!(error.message.contains("JSON"));
        assert!(jsx("").is_err());
        assert!(
            jsx(&"<frame>".repeat(100))
                .unwrap_err()
                .message
                .contains("nest")
        );
    }

    #[test]
    fn json_specs_are_one_node_or_a_list() {
        assert_eq!(
            parse_spec("json", r#"{"type": "rect"}"#)
                .unwrap()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            parse_spec("json", r#"[{"type": "rect"}, {"type": "rect"}]"#)
                .unwrap()
                .unwrap()
                .len(),
            2
        );
        let error = parse_spec("json", "[\n{").unwrap().unwrap_err();
        assert_eq!(error.line, 2);
        assert!(parse_spec("json", "3").unwrap().is_err());
        assert!(parse_spec("yaml", "").is_none());
    }
}
