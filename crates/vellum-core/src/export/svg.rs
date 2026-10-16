//! SVG writer over lowered, filled paths and self-contained PNG images. No vendor types or I/O.
use std::fmt::Write;
#[derive(Debug, Clone)]
pub enum Command {
    Save,
    Restore,
    Transform([f32; 6]),
    Layer(f32),
    Clip {
        path: String,
        even_odd: bool,
    },
    Path {
        path: String,
        even_odd: bool,
        color: [u8; 4],
    },
    Image {
        png: Vec<u8>,
        size: [u32; 2],
        transform: [f32; 6],
        opacity: f32,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidSize,
    InvalidNumber,
    Unbalanced,
}
/// Transform is absolute. Clip geometry captures its current device matrix; layers isolate opacity.
pub fn write(width: u32, height: u32, commands: &[Command]) -> Result<String, Error> {
    if width == 0 || height == 0 {
        return Err(Error::InvalidSize);
    }
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );
    let mut transform = [1., 0., 0., 1., 0., 0.];
    let mut stack = vec![];
    let mut open = 0;
    let mut clip_id = 0;
    for cmd in commands {
        match cmd {
            Command::Save | Command::Layer(_) => {
                stack.push((transform, open));
                open = 1;
                if let Command::Layer(a) = cmd {
                    if !(0. ..=1.).contains(a) {
                        return Err(Error::InvalidNumber);
                    }
                    write!(out, "<g opacity=\"{a}\">").unwrap();
                } else {
                    out.push_str("<g>");
                }
            }
            Command::Restore => {
                for _ in 0..open {
                    out.push_str("</g>");
                }
                (transform, open) = stack.pop().ok_or(Error::Unbalanced)?;
            }
            Command::Transform(m) => {
                if m.iter().any(|v| !v.is_finite()) {
                    return Err(Error::InvalidNumber);
                }
                transform = *m;
            }
            Command::Clip { path, even_odd } => {
                let id = clip_id;
                clip_id += 1;
                write!(out,"<defs><clipPath id=\"c{id}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{}\" transform=\"{}\" clip-rule=\"{}\"/></clipPath></defs><g clip-path=\"url(#c{id})\">",escape(path),matrix(transform),rule(*even_odd)).unwrap();
                open += 1;
            }
            Command::Path {
                path,
                even_odd,
                color: [r, g, b, a],
            } => {
                write!(out,"<path d=\"{}\" transform=\"{}\" fill-rule=\"{}\" fill=\"#{r:02x}{g:02x}{b:02x}\" fill-opacity=\"{}\"/>",escape(path),matrix(transform),rule(*even_odd),f32::from(*a)/255.).unwrap();
            }
            Command::Image {
                png,
                size: [w, h],
                transform: local,
                opacity,
            } => {
                if !(0. ..=1.).contains(opacity) || local.iter().any(|n| !n.is_finite()) {
                    return Err(Error::InvalidNumber);
                }
                write!(out,"<image width=\"{w}\" height=\"{h}\" transform=\"{}\" opacity=\"{opacity}\" href=\"data:image/png;base64,{}\"/>",matrix(compose(transform,*local)),base64(png)).unwrap();
            }
        }
    }
    if !stack.is_empty() {
        return Err(Error::Unbalanced);
    }
    for _ in 0..open {
        out.push_str("</g>");
    }
    out.push_str("</svg>");
    Ok(out)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn rule(e: bool) -> &'static str {
    if e { "evenodd" } else { "nonzero" }
}
fn matrix(m: [f32; 6]) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        m[0], m[1], m[2], m[3], m[4], m[5]
    )
}
fn compose(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}
fn base64(bytes: &[u8]) -> String {
    crate::base64::encode(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_bytes_and_markup_are_safe() {
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b"a"), "YQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        let svg = write(
            10,
            10,
            &[Command::Path {
                path: "\"/><script>".into(),
                even_odd: false,
                color: [0; 4],
            }],
        )
        .unwrap();
        assert!(!svg.contains("<script>"));
    }
    #[test]
    fn refuses_unbalanced_or_nonfinite_input() {
        assert_eq!(write(1, 1, &[Command::Restore]), Err(Error::Unbalanced));
        assert_eq!(write(1, 1, &[Command::Save]), Err(Error::Unbalanced));
        assert_eq!(
            write(1, 1, &[Command::Transform([f32::NAN; 6])]),
            Err(Error::InvalidNumber)
        );
    }
}
