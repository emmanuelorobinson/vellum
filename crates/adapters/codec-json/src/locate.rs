//! Finding things in the text of a file, for errors. A file that has already been read as JSON is indexed into
//! the byte ranges of its objects, arrays and values, so a path such as `pages[0].children[2]` or the name of
//! an unknown field can be turned into a line and a column. The index is built only to explain a failure or
//! to look for duplicate keys; the document itself is read by serde.

use std::borrow::Cow;
use std::collections::HashSet;

use crate::error::Position;
use crate::tree::Step;

/// Deeper than any file serde reads (its limit is 128), so a file that got this far never hits it.
const MAX_DEPTH: usize = 256;

pub(crate) struct Member {
    key_start: usize,
    key_end: usize,
    pub(crate) value: Value,
}

pub(crate) enum Value {
    Object { start: usize, members: Vec<Member> },
    Array { start: usize, items: Vec<Value> },
    Scalar { start: usize },
}

impl Value {
    pub(crate) fn start(&self) -> usize {
        match self {
            Self::Object { start, .. } | Self::Array { start, .. } | Self::Scalar { start } => {
                *start
            }
        }
    }
}

/// The indexed text.
pub(crate) struct Index<'a> {
    bytes: &'a [u8],
    pub(crate) root: Value,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn skip_space(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    /// Past the closing quote of the string that starts at `at`.
    fn string_end(&mut self) -> Option<usize> {
        self.at += 1;
        loop {
            match self.bytes.get(self.at)? {
                b'\\' => self.at += 2,
                b'"' => {
                    self.at += 1;
                    return Some(self.at);
                }
                _ => self.at += 1,
            }
        }
    }

    fn value(&mut self, depth: usize) -> Option<Value> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.skip_space();
        let start = self.at;
        match self.bytes.get(self.at)? {
            b'{' => {
                self.at += 1;
                let mut members = Vec::new();
                loop {
                    self.skip_space();
                    match self.bytes.get(self.at)? {
                        b'}' => {
                            self.at += 1;
                            return Some(Value::Object { start, members });
                        }
                        b',' => self.at += 1,
                        b'"' => {
                            let key_start = self.at;
                            let key_end = self.string_end()?;
                            self.skip_space();
                            if self.bytes.get(self.at)? != &b':' {
                                return None;
                            }
                            self.at += 1;
                            let value = self.value(depth + 1)?;
                            members.push(Member {
                                key_start,
                                key_end,
                                value,
                            });
                        }
                        _ => return None,
                    }
                }
            }
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    match self.bytes.get(self.at)? {
                        b']' => {
                            self.at += 1;
                            return Some(Value::Array { start, items });
                        }
                        b',' => self.at += 1,
                        _ => items.push(self.value(depth + 1)?),
                    }
                }
            }
            b'"' => {
                self.string_end()?;
                Some(Value::Scalar { start })
            }
            _ => {
                while !matches!(
                    self.bytes.get(self.at),
                    None | Some(b',' | b']' | b'}' | b' ' | b'\t' | b'\n' | b'\r')
                ) {
                    self.at += 1;
                }
                (self.at > start).then_some(Value::Scalar { start })
            }
        }
    }
}

impl<'a> Index<'a> {
    /// Indexes `bytes`, or `None` if they are not the JSON this expects (the caller has already parsed them).
    pub(crate) fn new(bytes: &'a [u8]) -> Option<Self> {
        let mut cursor = Cursor { bytes, at: 0 };
        let root = cursor.value(0)?;
        Some(Self { bytes, root })
    }

    fn key_text(&self, member: &Member) -> Cow<'a, str> {
        let raw = &self.bytes[member.key_start..member.key_end];
        let inner = &raw[1..raw.len().saturating_sub(1).max(1)];
        if inner.contains(&b'\\') {
            serde_json::from_slice::<String>(raw).map_or(Cow::Borrowed(""), Cow::Owned)
        } else {
            String::from_utf8_lossy(inner)
        }
    }

    /// The value at `path`, or the deepest one the path reaches.
    pub(crate) fn deepest(&self, path: &[Step]) -> &Value {
        let mut here = &self.root;
        for step in path {
            let next = match (here, step) {
                (Value::Object { members, .. }, Step::Key(key)) => members
                    .iter()
                    .find(|m| self.key_text(m) == key.as_str())
                    .map(|m| &m.value),
                (Value::Array { items, .. }, Step::Index(i)) => items.get(*i),
                _ => None,
            };
            match next {
                Some(value) => here = value,
                None => break,
            }
        }
        here
    }

    /// Where the value of member `key` of `object` starts.
    pub(crate) fn member_value_start(&self, object: &Value, key: &str) -> Option<usize> {
        let Value::Object { members, .. } = object else {
            return None;
        };
        members
            .iter()
            .find(|m| self.key_text(m) == key)
            .map(|m| m.value.start())
    }

    /// Where the unknown field `key` is: in `within` or below it, the first object that has a member called
    /// `key` and no member outside `expected` (and the `type` tag). The field may sit below `within`, because
    /// serde reports a fault inside a tagged enum at the enum's own path.
    pub(crate) fn find_unknown(
        &self,
        within: &Value,
        key: &str,
        expected: &[String],
    ) -> Option<usize> {
        match within {
            Value::Object { members, .. } => {
                let has_key = members.iter().any(|m| self.key_text(m) == key);
                let others_known = members.iter().all(|m| {
                    let name = self.key_text(m);
                    name == key || name == "type" || expected.iter().any(|e| *e == name)
                });
                if has_key && others_known {
                    return members
                        .iter()
                        .find(|m| self.key_text(m) == key)
                        .map(|m| m.key_start);
                }
                members
                    .iter()
                    .find_map(|m| self.find_unknown(&m.value, key, expected))
            }
            Value::Array { items, .. } => items
                .iter()
                .find_map(|v| self.find_unknown(v, key, expected)),
            Value::Scalar { .. } => None,
        }
    }

    /// The first key that appears twice in one object: where the path to that object is, the offset of the
    /// second key and its name. serde would keep the last and say nothing, so a merge that leaves both would
    /// silently drop one.
    pub(crate) fn first_duplicate(&self) -> Option<(Vec<Step>, usize, String)> {
        fn walk(
            index: &Index<'_>,
            value: &Value,
            path: &mut Vec<Step>,
        ) -> Option<(Vec<Step>, usize, String)> {
            match value {
                Value::Object { members, .. } => {
                    let mut seen = HashSet::with_capacity(members.len());
                    for member in members {
                        let name = index.key_text(member);
                        if !seen.insert(name.clone()) {
                            return Some((path.clone(), member.key_start, name.into_owned()));
                        }
                    }
                    for member in members {
                        path.push(Step::Key(index.key_text(member).into_owned()));
                        let found = walk(index, &member.value, path);
                        path.pop();
                        if found.is_some() {
                            return found;
                        }
                    }
                    None
                }
                Value::Array { items, .. } => {
                    for (i, item) in items.iter().enumerate() {
                        path.push(Step::Index(i));
                        let found = walk(index, item, path);
                        path.pop();
                        if found.is_some() {
                            return found;
                        }
                    }
                    None
                }
                Value::Scalar { .. } => None,
            }
        }
        walk(self, &self.root, &mut Vec::new())
    }
}

/// The line and column (in characters) of the byte at `offset`.
pub(crate) fn position(bytes: &[u8], offset: usize) -> Position {
    let offset = offset.min(bytes.len());
    let before = &bytes[..offset];
    let line = before.iter().filter(|b| **b == b'\n').count() + 1;
    let line_start = before
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |at| at + 1);
    let column = before[line_start..]
        .iter()
        .filter(|b| (**b & 0xC0) != 0x80)
        .count()
        + 1;
    Position { line, column }
}

/// The byte offset of a position serde_json reports: its column counts bytes within the line.
pub(crate) fn offset_of(bytes: &[u8], line: usize, byte_column: usize) -> usize {
    let mut offset = 0;
    for _ in 1..line {
        match bytes[offset..].iter().position(|b| *b == b'\n') {
            Some(at) => offset += at + 1,
            None => return bytes.len(),
        }
    }
    (offset + byte_column.saturating_sub(1)).min(bytes.len())
}
