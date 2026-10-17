//! SVG colours: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()`, `hsl()`/`hsla()` and the CSS names.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 1.,
    };

    pub fn with_alpha(self, factor: f64) -> Rgba {
        Rgba {
            a: (self.a * factor).clamp(0., 1.),
            ..self
        }
    }

    /// `#RRGGBB`, or `#RRGGBBAA` when not opaque.
    pub fn hex(self) -> String {
        let mut s = format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b);
        if self.a < 1. {
            s.push_str(&format!("{:02X}", (self.a * 255.).round() as u8));
        }
        s
    }
}

/// What a paint property says.
#[derive(Debug, Clone, PartialEq)]
pub enum PaintValue {
    None,
    Color(Rgba),
    CurrentColor,
    /// `url(#id)` and the colour to use when the reference cannot be followed.
    Url(String, Option<Rgba>),
}

pub fn parse_paint(text: &str) -> Option<PaintValue> {
    let t = text.trim();
    if t.eq_ignore_ascii_case("none") || t.eq_ignore_ascii_case("transparent") {
        return Some(PaintValue::None);
    }
    if t.eq_ignore_ascii_case("currentcolor") {
        return Some(PaintValue::CurrentColor);
    }
    if let Some(rest) = t.strip_prefix("url(") {
        let end = rest.find(')')?;
        let id = rest[..end]
            .trim()
            .trim_matches(['"', '\''])
            .strip_prefix('#')?
            .to_owned();
        let fallback = parse_color(rest[end + 1..].trim());
        return Some(PaintValue::Url(id, fallback));
    }
    parse_color(t).map(PaintValue::Color)
}

pub fn parse_color(text: &str) -> Option<Rgba> {
    let t = text.trim();
    if let Some(hex) = t.strip_prefix('#') {
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let nib = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok().map(|v| v * 17);
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        return match hex.len() {
            3 => Some(Rgba {
                r: nib(0)?,
                g: nib(1)?,
                b: nib(2)?,
                a: 1.,
            }),
            4 => Some(Rgba {
                r: nib(0)?,
                g: nib(1)?,
                b: nib(2)?,
                a: f64::from(nib(3)?) / 255.,
            }),
            6 => Some(Rgba {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: 1.,
            }),
            8 => Some(Rgba {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: f64::from(byte(6)?) / 255.,
            }),
            _ => None,
        };
    }
    let lower = t.to_ascii_lowercase();
    if let Some(open) = lower.find('(') {
        let name = lower[..open].trim();
        let body = lower[open + 1..].strip_suffix(')')?;
        let parts: Vec<&str> = body
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() < 3 || parts.len() > 4 {
            return None;
        }
        let alpha = match parts.get(3) {
            Some(a) => unit(a)?,
            None => 1.,
        };
        return match name {
            "rgb" | "rgba" => {
                let channel = |s: &str| match s.strip_suffix('%') {
                    Some(p) => p
                        .parse::<f64>()
                        .ok()
                        .map(|v| (v * 2.55).round().clamp(0., 255.) as u8),
                    None => s
                        .parse::<f64>()
                        .ok()
                        .map(|v| v.round().clamp(0., 255.) as u8),
                };
                Some(Rgba {
                    r: channel(parts[0])?,
                    g: channel(parts[1])?,
                    b: channel(parts[2])?,
                    a: alpha,
                })
            }
            "hsl" | "hsla" => {
                let h = parts[0].trim_end_matches("deg").parse::<f64>().ok()?;
                let s = parts[1].strip_suffix('%')?.parse::<f64>().ok()? / 100.;
                let l = parts[2].strip_suffix('%')?.parse::<f64>().ok()? / 100.;
                let (r, g, b) = hsl(h, s.clamp(0., 1.), l.clamp(0., 1.));
                Some(Rgba { r, g, b, a: alpha })
            }
            _ => None,
        };
    }
    NAMED
        .binary_search_by_key(&lower.as_str(), |(n, _)| n)
        .ok()
        .map(|i| {
            let v = NAMED[i].1;
            Rgba {
                r: (v >> 16) as u8,
                g: (v >> 8) as u8,
                b: v as u8,
                a: 1.,
            }
        })
}

fn unit(s: &str) -> Option<f64> {
    match s.strip_suffix('%') {
        Some(p) => p.parse::<f64>().ok().map(|v| (v / 100.).clamp(0., 1.)),
        None => s.parse::<f64>().ok().map(|v| v.clamp(0., 1.)),
    }
}

fn hsl(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1. - (2. * l - 1.).abs()) * s;
    let h = h.rem_euclid(360.) / 60.;
    let x = c * (1. - (h % 2. - 1.).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = l - c / 2.;
    let to = |v: f64| ((v + m) * 255.).round().clamp(0., 255.) as u8;
    (to(r), to(g), to(b))
}

/// The CSS colour names, sorted for binary search.
const NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xF0F8FF),
    ("antiquewhite", 0xFAEBD7),
    ("aqua", 0x00FFFF),
    ("aquamarine", 0x7FFFD4),
    ("azure", 0xF0FFFF),
    ("beige", 0xF5F5DC),
    ("bisque", 0xFFE4C4),
    ("black", 0x000000),
    ("blanchedalmond", 0xFFEBCD),
    ("blue", 0x0000FF),
    ("blueviolet", 0x8A2BE2),
    ("brown", 0xA52A2A),
    ("burlywood", 0xDEB887),
    ("cadetblue", 0x5F9EA0),
    ("chartreuse", 0x7FFF00),
    ("chocolate", 0xD2691E),
    ("coral", 0xFF7F50),
    ("cornflowerblue", 0x6495ED),
    ("cornsilk", 0xFFF8DC),
    ("crimson", 0xDC143C),
    ("cyan", 0x00FFFF),
    ("darkblue", 0x00008B),
    ("darkcyan", 0x008B8B),
    ("darkgoldenrod", 0xB8860B),
    ("darkgray", 0xA9A9A9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xA9A9A9),
    ("darkkhaki", 0xBDB76B),
    ("darkmagenta", 0x8B008B),
    ("darkolivegreen", 0x556B2F),
    ("darkorange", 0xFF8C00),
    ("darkorchid", 0x9932CC),
    ("darkred", 0x8B0000),
    ("darksalmon", 0xE9967A),
    ("darkseagreen", 0x8FBC8F),
    ("darkslateblue", 0x483D8B),
    ("darkslategray", 0x2F4F4F),
    ("darkslategrey", 0x2F4F4F),
    ("darkturquoise", 0x00CED1),
    ("darkviolet", 0x9400D3),
    ("deeppink", 0xFF1493),
    ("deepskyblue", 0x00BFFF),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1E90FF),
    ("firebrick", 0xB22222),
    ("floralwhite", 0xFFFAF0),
    ("forestgreen", 0x228B22),
    ("fuchsia", 0xFF00FF),
    ("gainsboro", 0xDCDCDC),
    ("ghostwhite", 0xF8F8FF),
    ("gold", 0xFFD700),
    ("goldenrod", 0xDAA520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xADFF2F),
    ("grey", 0x808080),
    ("honeydew", 0xF0FFF0),
    ("hotpink", 0xFF69B4),
    ("indianred", 0xCD5C5C),
    ("indigo", 0x4B0082),
    ("ivory", 0xFFFFF0),
    ("khaki", 0xF0E68C),
    ("lavender", 0xE6E6FA),
    ("lavenderblush", 0xFFF0F5),
    ("lawngreen", 0x7CFC00),
    ("lemonchiffon", 0xFFFACD),
    ("lightblue", 0xADD8E6),
    ("lightcoral", 0xF08080),
    ("lightcyan", 0xE0FFFF),
    ("lightgoldenrodyellow", 0xFAFAD2),
    ("lightgray", 0xD3D3D3),
    ("lightgreen", 0x90EE90),
    ("lightgrey", 0xD3D3D3),
    ("lightpink", 0xFFB6C1),
    ("lightsalmon", 0xFFA07A),
    ("lightseagreen", 0x20B2AA),
    ("lightskyblue", 0x87CEFA),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xB0C4DE),
    ("lightyellow", 0xFFFFE0),
    ("lime", 0x00FF00),
    ("limegreen", 0x32CD32),
    ("linen", 0xFAF0E6),
    ("magenta", 0xFF00FF),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66CDAA),
    ("mediumblue", 0x0000CD),
    ("mediumorchid", 0xBA55D3),
    ("mediumpurple", 0x9370DB),
    ("mediumseagreen", 0x3CB371),
    ("mediumslateblue", 0x7B68EE),
    ("mediumspringgreen", 0x00FA9A),
    ("mediumturquoise", 0x48D1CC),
    ("mediumvioletred", 0xC71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xF5FFFA),
    ("mistyrose", 0xFFE4E1),
    ("moccasin", 0xFFE4B5),
    ("navajowhite", 0xFFDEAD),
    ("navy", 0x000080),
    ("oldlace", 0xFDF5E6),
    ("olive", 0x808000),
    ("olivedrab", 0x6B8E23),
    ("orange", 0xFFA500),
    ("orangered", 0xFF4500),
    ("orchid", 0xDA70D6),
    ("palegoldenrod", 0xEEE8AA),
    ("palegreen", 0x98FB98),
    ("paleturquoise", 0xAFEEEE),
    ("palevioletred", 0xDB7093),
    ("papayawhip", 0xFFEFD5),
    ("peachpuff", 0xFFDAB9),
    ("peru", 0xCD853F),
    ("pink", 0xFFC0CB),
    ("plum", 0xDDA0DD),
    ("powderblue", 0xB0E0E6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xFF0000),
    ("rosybrown", 0xBC8F8F),
    ("royalblue", 0x4169E1),
    ("saddlebrown", 0x8B4513),
    ("salmon", 0xFA8072),
    ("sandybrown", 0xF4A460),
    ("seagreen", 0x2E8B57),
    ("seashell", 0xFFF5EE),
    ("sienna", 0xA0522D),
    ("silver", 0xC0C0C0),
    ("skyblue", 0x87CEEB),
    ("slateblue", 0x6A5ACD),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xFFFAFA),
    ("springgreen", 0x00FF7F),
    ("steelblue", 0x4682B4),
    ("tan", 0xD2B48C),
    ("teal", 0x008080),
    ("thistle", 0xD8BFD8),
    ("tomato", 0xFF6347),
    ("turquoise", 0x40E0D0),
    ("violet", 0xEE82EE),
    ("wheat", 0xF5DEB3),
    ("white", 0xFFFFFF),
    ("whitesmoke", 0xF5F5F5),
    ("yellow", 0xFFFF00),
    ("yellowgreen", 0x9ACD32),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_table_is_sorted_and_complete() {
        assert!(NAMED.windows(2).all(|w| w[0].0 < w[1].0), "not sorted");
        assert_eq!(NAMED.len(), 148);
    }

    #[test]
    fn every_spelling_reads() {
        let rgb = |r, g, b| Rgba { r, g, b, a: 1. };
        assert_eq!(parse_color("#f80"), Some(rgb(255, 136, 0)));
        assert_eq!(parse_color("#FF8800"), Some(rgb(255, 136, 0)));
        assert_eq!(
            parse_color("#ff880080").map(|c| c.hex()),
            Some("#FF880080".into())
        );
        assert_eq!(parse_color("rgb(255, 136, 0)"), Some(rgb(255, 136, 0)));
        assert_eq!(parse_color("rgb(100% 0% 0% / 50%)").map(|c| c.a), Some(0.5));
        assert_eq!(parse_color("hsl(120, 100%, 50%)"), Some(rgb(0, 255, 0)));
        assert_eq!(parse_color("Rebeccapurple"), Some(rgb(0x66, 0x33, 0x99)));
        assert_eq!(parse_color("notacolour"), None);
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("#ggg"), None);
    }

    #[test]
    fn paints_name_none_current_colour_and_references() {
        assert_eq!(parse_paint("none"), Some(PaintValue::None));
        assert_eq!(parse_paint("currentColor"), Some(PaintValue::CurrentColor));
        assert_eq!(
            parse_paint("url(#g1) red"),
            Some(PaintValue::Url("g1".into(), parse_color("red")))
        );
        assert_eq!(
            parse_paint("url('#g1')"),
            Some(PaintValue::Url("g1".into(), None))
        );
    }
}
