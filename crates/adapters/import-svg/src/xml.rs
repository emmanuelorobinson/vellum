//! A small, bounded XML reader for SVG. It builds a tree of elements and text and nothing else: no entity
//! definitions (so no entity expansion attacks), no external references, no namespaces beyond dropping `svg:`.

/// The deepest nesting accepted.
const MAX_DEPTH: usize = 128;
/// The most elements accepted.
const MAX_ELEMENTS: usize = 200_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Child {
    Element(Element),
    Text(String),
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Child::Element(e) => Some(e),
            Child::Text(_) => None,
        })
    }

    /// All the text inside, in order.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for child in &self.children {
            match child {
                Child::Text(t) => out.push_str(t),
                Child::Element(e) => out.push_str(&e.text()),
            }
        }
        out
    }
}

struct Reader<'a> {
    s: &'a str,
    at: usize,
    elements: usize,
}

pub fn parse(source: &str) -> Result<Element, String> {
    let mut r = Reader {
        s: source.strip_prefix('\u{feff}').unwrap_or(source),
        at: 0,
        elements: 0,
    };
    loop {
        r.skip_space();
        if r.rest().starts_with("<?") {
            r.skip_past("?>")?;
        } else if r.rest().starts_with("<!--") {
            r.skip_past("-->")?;
        } else if r.rest().starts_with("<!") {
            r.skip_doctype()?;
        } else {
            break;
        }
    }
    if !r.rest().starts_with('<') {
        return Err("This is not XML: it does not start with a tag.".into());
    }
    let root = r.element(0)?;
    Ok(root)
}

impl<'a> Reader<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.at..]
    }

    fn skip_space(&mut self) {
        let rest = self.rest();
        self.at += rest.len() - rest.trim_start().len();
    }

    fn skip_past(&mut self, end: &str) -> Result<(), String> {
        match self.rest().find(end) {
            Some(i) => {
                self.at += i + end.len();
                Ok(())
            }
            None => Err(format!(
                "The XML ends inside a section that should end with {end:?}."
            )),
        }
    }

    fn skip_doctype(&mut self) -> Result<(), String> {
        // <!DOCTYPE ... [ internal subset ]> : entities declared there are never expanded.
        let mut depth = 0usize;
        let bytes = self.rest().as_bytes();
        for (i, b) in bytes.iter().enumerate() {
            match b {
                b'[' => depth += 1,
                b']' => depth = depth.saturating_sub(1),
                b'>' if depth == 0 => {
                    self.at += i + 1;
                    return Ok(());
                }
                _ => {}
            }
        }
        Err("The XML ends inside its DOCTYPE.".into())
    }

    fn name(&mut self) -> Result<String, String> {
        let rest = self.rest();
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '/' | '>' | '=' | '<'))
            .unwrap_or(rest.len());
        if end == 0 {
            return Err("A tag or attribute has no name.".into());
        }
        self.at += end;
        let name = &rest[..end];
        Ok(name.strip_prefix("svg:").unwrap_or(name).to_owned())
    }

    fn element(&mut self, depth: usize) -> Result<Element, String> {
        if depth > MAX_DEPTH {
            return Err(format!("Elements nest more than {MAX_DEPTH} deep."));
        }
        self.elements += 1;
        if self.elements > MAX_ELEMENTS {
            return Err(format!("The file has more than {MAX_ELEMENTS} elements."));
        }
        self.at += 1; // '<'
        let name = self.name()?;
        let mut attrs = Vec::new();
        loop {
            self.skip_space();
            let rest = self.rest();
            if rest.starts_with("/>") {
                self.at += 2;
                return Ok(Element {
                    name,
                    attrs,
                    children: vec![],
                });
            }
            if rest.starts_with('>') {
                self.at += 1;
                break;
            }
            if rest.is_empty() {
                return Err(format!("The tag <{name}> is not closed."));
            }
            let key = self.name()?;
            self.skip_space();
            if !self.rest().starts_with('=') {
                // An attribute without a value is not valid XML; keep reading as an empty one.
                attrs.push((key, String::new()));
                continue;
            }
            self.at += 1;
            self.skip_space();
            let quote = self
                .rest()
                .chars()
                .next()
                .filter(|c| matches!(c, '"' | '\''));
            let Some(quote) = quote else {
                return Err(format!(
                    "The attribute {key} of <{name}> has no quoted value."
                ));
            };
            self.at += 1;
            let end = self
                .rest()
                .find(quote)
                .ok_or_else(|| format!("The attribute {key} of <{name}> is not closed."))?;
            let value = decode(&self.rest()[..end]);
            self.at += end + 1;
            if !attrs.iter().any(|(k, _)| *k == key) {
                attrs.push((key, value));
            }
        }
        let mut children = Vec::new();
        loop {
            let rest = self.rest();
            if rest.is_empty() {
                return Err(format!("The tag <{name}> is never closed."));
            }
            if rest.starts_with("</") {
                let close = rest.find('>').ok_or("A closing tag is not finished.")?;
                let closing = rest[2..close].trim();
                let closing = closing.strip_prefix("svg:").unwrap_or(closing);
                if closing != name {
                    return Err(format!("<{name}> is closed by </{closing}>."));
                }
                self.at += close + 1;
                return Ok(Element {
                    name,
                    attrs,
                    children,
                });
            }
            if rest.starts_with("<!--") {
                self.skip_past("-->")?;
            } else if let Some(cdata) = rest.strip_prefix("<![CDATA[") {
                let end = cdata.find("]]>").ok_or("A CDATA section is not closed.")?;
                children.push(Child::Text(cdata[..end].to_owned()));
                self.at += 9 + end + 3;
            } else if rest.starts_with("<?") {
                self.skip_past("?>")?;
            } else if rest.starts_with('<') {
                children.push(Child::Element(self.element(depth + 1)?));
            } else {
                let end = rest.find('<').unwrap_or(rest.len());
                children.push(Child::Text(decode(&rest[..end])));
                self.at += end;
            }
        }
    }
}

/// The five predefined entities and numeric references. Anything else is left as written.
fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let replaced = rest.find(';').filter(|&e| e <= 10).and_then(|e| {
            let body = &rest[1..e];
            let ch = match body {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => body
                    .strip_prefix("#x")
                    .or_else(|| body.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| body.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, e + 1))
        });
        match replaced {
            Some((c, used)) => {
                out.push(c);
                rest = &rest[used..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_elements_attributes_text_and_entities() {
        let root = parse(
            "<?xml version='1.0'?><!DOCTYPE svg [<!ENTITY a 'b'>]><!-- c --><svg:svg a=\"1\" b='x &amp; y'><g/><text>A&#66;&#x43;<![CDATA[<d>]]></text></svg:svg>",
        )
        .unwrap();
        assert_eq!(root.name, "svg");
        assert_eq!(root.attr("b"), Some("x & y"));
        let kids: Vec<_> = root.elements().collect();
        assert_eq!(kids[0].name, "g");
        assert_eq!(kids[1].text(), "ABC<d>");
    }

    #[test]
    fn refuses_mismatched_or_unfinished_documents() {
        for bad in [
            "",
            "hello",
            "<a>",
            "<a></b>",
            "<a b=1/>",
            "<a b='x/>",
            "<!-- x",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn deep_nesting_is_refused() {
        let deep = "<a>".repeat(200) + &"</a>".repeat(200);
        assert!(parse(&deep).is_err());
    }
}
