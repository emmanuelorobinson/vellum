//! The part of CSS an SVG's `<style>` uses in practice: rules whose selectors are `*`, a tag, `.class`, `#id` or
//! a tag with a class, each with plain declarations. Selectors with combinators, attributes or pseudo-classes are
//! reported and skipped.
use crate::xml::Element;

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    selector: Selector,
    pub decls: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Selector {
    tag: Option<String>,
    class: Option<String>,
    id: Option<String>,
}

pub fn declarations(text: &str) -> Vec<(String, String)> {
    text.split(';')
        .filter_map(|d| {
            let (name, value) = d.split_once(':')?;
            let value = value.trim().trim_end_matches("!important").trim();
            Some((name.trim().to_ascii_lowercase(), value.to_owned()))
                .filter(|(n, v)| !n.is_empty() && !v.is_empty())
        })
        .collect()
}

/// Rules in source order, and how many selectors were skipped.
pub fn parse(css: &str) -> (Vec<Rule>, usize) {
    let mut clean = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(i) = rest.find("/*") {
        clean.push_str(&rest[..i]);
        rest = match rest[i + 2..].find("*/") {
            Some(j) => &rest[i + 2 + j + 2..],
            None => "",
        };
    }
    clean.push_str(rest);
    let (mut rules, mut skipped) = (Vec::new(), 0);
    let mut rest = clean.as_str();
    while let Some(open) = rest.find('{') {
        let head = rest[..open].trim();
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let mut body_end = open + close;
        if head.starts_with('@') {
            // @media and friends nest braces: skip the whole block.
            let mut depth = 0;
            for (i, c) in rest[open..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            body_end = open + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            rest = &rest[(body_end + 1).min(rest.len())..];
            skipped += 1;
            continue;
        }
        let decls = declarations(&rest[open + 1..body_end]);
        for sel in head.split(',') {
            match simple(sel.trim()) {
                Some(selector) => rules.push(Rule {
                    selector,
                    decls: decls.clone(),
                }),
                None => skipped += 1,
            }
        }
        rest = &rest[body_end + 1..];
    }
    (rules, skipped)
}

fn simple(text: &str) -> Option<Selector> {
    if text.is_empty()
        || text.contains(|c: char| c.is_whitespace() || matches!(c, '>' | '+' | '~' | '[' | ':'))
    {
        return None;
    }
    if text == "*" {
        return Some(Selector::default());
    }
    let (tag, rest) = match text.find(['.', '#']) {
        Some(0) => (None, text),
        Some(i) => (Some(text[..i].to_owned()), &text[i..]),
        None => (Some(text.to_owned()), ""),
    };
    let mut sel = Selector {
        tag,
        ..Selector::default()
    };
    if let Some(r) = rest.strip_prefix('.') {
        if r.is_empty() || r.contains(['.', '#']) {
            return None;
        }
        sel.class = Some(r.to_owned());
    } else if let Some(r) = rest.strip_prefix('#') {
        if r.is_empty() || r.contains(['.', '#']) {
            return None;
        }
        sel.id = Some(r.to_owned());
    }
    Some(sel)
}

impl Rule {
    pub fn matches(&self, el: &Element) -> bool {
        let s = &self.selector;
        s.tag.as_ref().is_none_or(|t| *t == el.name)
            && s.id.as_deref().is_none_or(|id| el.attr("id") == Some(id))
            && s.class.as_deref().is_none_or(|c| {
                el.attr("class")
                    .is_some_and(|l| l.split_whitespace().any(|x| x == c))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::parse as xml;

    #[test]
    fn simple_selectors_match_and_complex_ones_are_counted() {
        let (rules, skipped) = parse(
            "/* c */ .a, path.b { fill: #fff; stroke:none } #x{opacity:.5} g path { fill:red } @media print { .a{fill:red} } *{stroke-width:2}",
        );
        assert_eq!(skipped, 2);
        assert_eq!(rules.len(), 4);
        let el = xml("<path class='b c' id='x'/>").unwrap();
        let hit: Vec<_> = rules
            .iter()
            .filter(|r| r.matches(&el))
            .map(|r| r.decls[0].0.clone())
            .collect();
        assert_eq!(hit, ["fill", "opacity", "stroke-width"]);
    }

    #[test]
    fn declarations_ignore_important_and_blank_values() {
        assert_eq!(
            declarations("fill: red !important; ;stroke:; Opacity : .3"),
            vec![
                ("fill".into(), "red".into()),
                ("opacity".into(), ".3".into())
            ]
        );
    }
}
