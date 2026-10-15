//! A lexical guard for the only arithmetic syntax that bypasses portable Math: `**` / `**=`.
//! Strings, comments and template text are skipped; template substitutions are code. Regex
//! literals are unsupported by this context (no RegExp compiler), so slash needs no regex mode.
use vellum_core::ports::ScriptError;
#[derive(Clone, Copy)]
enum Mode {
    Code { braces: usize, substitution: bool },
    Quoted(u8),
    Template,
    LineComment,
    BlockComment,
}

pub fn check(src: &str) -> Result<(), ScriptError> {
    let bytes = src.as_bytes();
    let mut modes = vec![Mode::Code {
        braces: 0,
        substitution: false,
    }];
    let mut i = 0;
    let mut line_clean = true;
    while i < bytes.len() {
        if modes.len() > 128 {
            return Err(ScriptError::Forbidden {
                what: "source nesting depth".into(),
            });
        }
        let character = src[i..].chars().next().expect("within source");
        let c = bytes[i];
        let next = bytes.get(i + 1).copied();
        let newline = matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}');
        let whitespace = matches!(
            character,
            '\t' | '\u{b}' | '\u{c}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
                ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
        );
        match *modes.last().expect("root code remains") {
            Mode::Code {
                braces,
                substitution,
            } => {
                if bytes[i..].starts_with(b"<!--") || (line_clean && bytes[i..].starts_with(b"-->"))
                {
                    modes.push(Mode::LineComment);
                } else if c == b'/' && matches!(next, Some(b'/' | b'*')) {
                    modes.push(if next == Some(b'/') {
                        Mode::LineComment
                    } else {
                        Mode::BlockComment
                    });
                    i += 1;
                } else if newline {
                    line_clean = true;
                } else if !whitespace {
                    line_clean = false;
                    match (c, next) {
                        (b'*', Some(b'*')) => {
                            return Err(ScriptError::Forbidden {
                                what: "exponent syntax; use Math.pow".into(),
                            });
                        }
                        (b'\'' | b'"', _) => modes.push(Mode::Quoted(c)),
                        (b'`', _) => modes.push(Mode::Template),
                        (b'{', _) => {
                            *modes.last_mut().unwrap() = Mode::Code {
                                braces: braces + 1,
                                substitution,
                            }
                        }
                        (b'}', _) if braces > 0 => {
                            *modes.last_mut().unwrap() = Mode::Code {
                                braces: braces - 1,
                                substitution,
                            }
                        }
                        (b'}', _) if substitution => {
                            modes.pop();
                        }
                        _ => {}
                    }
                }
            }
            Mode::Quoted(quote) => {
                if c == b'\\' {
                    i += src[i + 1..].chars().next().map_or(0, char::len_utf8);
                } else if c == quote {
                    modes.pop();
                }
            }
            Mode::Template => match (c, next) {
                (b'\\', _) => i += src[i + 1..].chars().next().map_or(0, char::len_utf8),
                (b'`', _) => {
                    modes.pop();
                }
                (b'$', Some(b'{')) => {
                    modes.push(Mode::Code {
                        braces: 0,
                        substitution: true,
                    });
                    i += 1
                }
                _ => {}
            },
            Mode::LineComment => {
                if newline {
                    modes.pop();
                    line_clean = true;
                }
            }
            Mode::BlockComment => {
                if newline {
                    line_clean = true;
                }
                if c == b'*' && next == Some(b'/') {
                    modes.pop();
                    i += 1
                }
            }
        }
        i += character.len_utf8();
    }
    Ok(())
}
