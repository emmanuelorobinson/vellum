//! Token names as CSS custom properties: `color/primary` is `--color-primary`.
//!
//! Going back, a name is the document's own when it has one with that property name; otherwise the first hyphen
//! splits a known namespace from the rest (`--font-weight-bold` is `font/weight-bold`), and anything else stays
//! as written. A name that would not come back the same way carries its own name in a comment after the
//! declaration, so a file written here always reads back exactly.

use vellum_core::tokens::TokenOptions;

/// The first words of the property names design systems use. Only these are split into a namespace and a rest.
const NAMESPACES: &[&str] = &[
    "color",
    "font",
    "text",
    "spacing",
    "space",
    "radius",
    "shadow",
    "blur",
    "size",
    "width",
    "height",
    "gap",
    "border",
    "stroke",
    "leading",
    "tracking",
    "breakpoint",
    "container",
    "ease",
    "animate",
    "opacity",
    "z",
    "duration",
    "inset",
    "drop",
    "perspective",
    "aspect",
];

/// The comment that carries a name: `name: color/brand/primary`.
pub const HINT_KEY: &str = "name:";

/// The property name of the token `name`, without the dashes. `None` for a name a stylesheet cannot carry.
pub fn property(name: &str, options: &TokenOptions) -> Option<String> {
    if name.contains("*/") || name.chars().any(char::is_control) {
        return None;
    }
    let mut segments = name.split('/');
    let first = segments.next()?;
    let first = options.namespaces.get(first).map_or(first, String::as_str);
    let joined = std::iter::once(first)
        .chain(segments)
        .collect::<Vec<_>>()
        .join("-");
    Some(
        joined
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') || !c.is_ascii() {
                    c
                } else {
                    '_'
                }
            })
            .collect(),
    )
}

/// The token a property name stands for, when the file does not say.
pub fn name_of(property: &str, options: &TokenOptions) -> String {
    if let Some(known) = options
        .known
        .keys()
        .find(|name| self::property(name, options).as_deref() == Some(property))
    {
        return known.clone();
    }
    if let Some((head, rest)) = property.split_once('-')
        && !rest.is_empty()
        && !rest.starts_with('-')
    {
        let renamed = options
            .namespaces
            .iter()
            .find(|(_, css)| css.as_str() == head)
            .map(|(vellum, _)| vellum.as_str());
        if renamed.is_some() || NAMESPACES.contains(&head) {
            return format!("{}/{rest}", renamed.unwrap_or(head));
        }
    }
    property.to_owned()
}

/// Whether the name must be written out beside its declaration to be read back.
pub fn needs_hint(name: &str, property: &str, options: &TokenOptions) -> bool {
    let mut bare = options.clone();
    bare.known.clear();
    name_of(property, &bare) != name
}
