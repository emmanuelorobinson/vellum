//! Stable little-endian command records plus typed resource payloads. No native layout/padding or JSON text.
use super::*;
use serde_json::Value as V;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    Version(u32),
    Malformed,
    TooLarge,
    InvalidNumber,
    InvalidResource,
    Unbalanced,
}
impl std::fmt::Display for BufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "display buffer: {self:?}")
    }
}
impl std::error::Error for BufferError {}
const LIMIT: usize = 64 * 1024 * 1024;
fn u32out(b: &mut Vec<u8>, n: usize) -> Result<(), BufferError> {
    b.extend_from_slice(
        &u32::try_from(n)
            .map_err(|_| BufferError::TooLarge)?
            .to_le_bytes(),
    );
    Ok(())
}
fn float(b: &mut Vec<u8>, v: f32) -> Result<(), BufferError> {
    if !v.is_finite() {
        return Err(BufferError::InvalidNumber);
    }
    b.extend_from_slice(&(if v == 0. { 0. } else { v }).to_le_bytes());
    Ok(())
}
// Resource values use fixed tags. Integers retain all 64 bits; floating scalars are exclusively f32.
fn value(b: &mut Vec<u8>, v: &V, depth: usize) -> Result<(), BufferError> {
    if depth > 128 || b.len() > LIMIT {
        return Err(BufferError::TooLarge);
    }
    match v {
        V::Null => b.push(0),
        V::Bool(false) => b.push(1),
        V::Bool(true) => b.push(2),
        V::Number(n) => {
            if let Some(n) = n.as_u64() {
                b.push(3);
                b.extend_from_slice(&n.to_le_bytes());
            } else if let Some(n) = n.as_i64() {
                b.push(4);
                b.extend_from_slice(&n.to_le_bytes());
            } else {
                b.push(5);
                float(b, n.as_f64().ok_or(BufferError::InvalidNumber)? as f32)?;
            }
        }
        V::String(s) => {
            b.push(6);
            u32out(b, s.len())?;
            b.extend_from_slice(s.as_bytes());
        }
        V::Array(a) => {
            b.push(7);
            u32out(b, a.len())?;
            for v in a {
                value(b, v, depth + 1)?;
            }
        }
        V::Object(o) => {
            b.push(8);
            u32out(b, o.len())?;
            let sorted: BTreeMap<_, _> = o.iter().collect();
            for (k, v) in sorted {
                value(b, &V::String(k.clone()), depth + 1)?;
                value(b, v, depth + 1)?;
            }
        }
    }
    Ok(())
}
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], BufferError> {
        let end = self.at.checked_add(n).ok_or(BufferError::Malformed)?;
        let b = self.b.get(self.at..end).ok_or(BufferError::Malformed)?;
        self.at = end;
        Ok(b)
    }
    fn byte(&mut self) -> Result<u8, BufferError> {
        Ok(self.take(1)?[0])
    }
    fn word(&mut self) -> Result<u32, BufferError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn float(&mut self) -> Result<f32, BufferError> {
        let f = f32::from_le_bytes(self.take(4)?.try_into().unwrap());
        if f.is_finite() {
            Ok(f)
        } else {
            Err(BufferError::InvalidNumber)
        }
    }
    fn value(&mut self, depth: usize) -> Result<V, BufferError> {
        if depth > 128 {
            return Err(BufferError::TooLarge);
        }
        Ok(match self.byte()? {
            0 => V::Null,
            1 => V::Bool(false),
            2 => V::Bool(true),
            3 => V::from(u64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            4 => V::from(i64::from_le_bytes(self.take(8)?.try_into().unwrap())),
            5 => V::from(self.float()?),
            6 => {
                let n = self.word()? as usize;
                V::String(
                    std::str::from_utf8(self.take(n)?)
                        .map_err(|_| BufferError::Malformed)?
                        .into(),
                )
            }
            7 => {
                let n = self.word()? as usize;
                if n > self.b.len() - self.at {
                    return Err(BufferError::Malformed);
                }
                let mut a = Vec::new();
                for _ in 0..n {
                    a.push(self.value(depth + 1)?)
                }
                V::Array(a)
            }
            8 => {
                let n = self.word()? as usize;
                if n > (self.b.len() - self.at) / 2 {
                    return Err(BufferError::Malformed);
                }
                let mut o = serde_json::Map::new();
                for _ in 0..n {
                    let V::String(k) = self.value(depth + 1)? else {
                        return Err(BufferError::Malformed);
                    };
                    if o.insert(k, self.value(depth + 1)?).is_some() {
                        return Err(BufferError::Malformed);
                    }
                }
                V::Object(o)
            }
            _ => return Err(BufferError::Malformed),
        })
    }
}
impl DisplayList {
    pub fn to_bytes(&self) -> Result<Vec<u8>, BufferError> {
        self.validate()?;
        let mut b = b"VDL\0".to_vec();
        b.extend_from_slice(&DISPLAY_LIST_VERSION.to_le_bytes());
        u32out(&mut b, self.commands.len())?;
        u32out(&mut b, self.resources.len())?;
        for c in &self.commands {
            let word = |b: &mut Vec<u8>, n: u32| b.extend_from_slice(&n.to_le_bytes());
            let optional = |b: &mut Vec<u8>, n: Option<u32>| word(b, n.unwrap_or(u32::MAX));
            match c {
                DrawCmd::Save => b.push(0),
                DrawCmd::Restore => b.push(1),
                DrawCmd::Transform(m) => {
                    b.push(2);
                    for f in m {
                        float(&mut b, *f)?;
                    }
                }
                DrawCmd::Clip { shape } => {
                    b.push(3);
                    word(&mut b, *shape)
                }
                DrawCmd::Path {
                    shape,
                    paint,
                    stroke,
                } => {
                    b.push(4);
                    word(&mut b, *shape);
                    word(&mut b, *paint);
                    optional(&mut b, *stroke)
                }
                DrawCmd::GlyphRun {
                    glyphs,
                    paint,
                    stroke,
                } => {
                    b.push(5);
                    word(&mut b, *glyphs);
                    word(&mut b, *paint);
                    optional(&mut b, *stroke)
                }
                DrawCmd::Image { image, dst } => {
                    b.push(6);
                    word(&mut b, *image);
                    for f in dst {
                        float(&mut b, *f)?;
                    }
                }
                DrawCmd::Effect { effect } => {
                    b.push(7);
                    word(&mut b, *effect)
                }
                DrawCmd::Layer { opacity, blend } => {
                    b.push(8);
                    float(&mut b, *opacity)?;
                    let tag = BlendMode::ALL.iter().position(|v| v == blend).unwrap();
                    u32out(&mut b, tag)?;
                }
            }
        }
        for r in &self.resources {
            let mut payload = vec![];
            let v = serde_json::to_value(r).map_err(|_| BufferError::InvalidResource)?;
            // A nonfinite f32 serializes as null; re-reading refuses it before encoding can lose it.
            let back: Resource =
                serde_json::from_value(v.clone()).map_err(|_| BufferError::InvalidNumber)?;
            if back != *r {
                return Err(BufferError::InvalidNumber);
            }
            value(&mut payload, &v, 0)?;
            u32out(&mut b, payload.len())?;
            b.extend_from_slice(&payload);
        }
        if b.len() > LIMIT {
            return Err(BufferError::TooLarge);
        }
        Ok(b)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BufferError> {
        if bytes.len() > LIMIT {
            return Err(BufferError::TooLarge);
        }
        let mut r = Reader { b: bytes, at: 0 };
        if r.take(4)? != b"VDL\0" {
            return Err(BufferError::Malformed);
        }
        let version = r.word()?;
        if version != DISPLAY_LIST_VERSION {
            return Err(BufferError::Version(version));
        }
        let commands = r.word()? as usize;
        let resources = r.word()? as usize;
        if commands > bytes.len() || resources > bytes.len() / 4 {
            return Err(BufferError::Malformed);
        }
        let mut list = Self::default();
        for _ in 0..commands {
            list.commands.push(match r.byte()? {
                0 => DrawCmd::Save,
                1 => DrawCmd::Restore,
                2 => {
                    let mut m = [0.; 6];
                    for f in &mut m {
                        *f = r.float()?;
                    }
                    DrawCmd::Transform(m)
                }
                3 => DrawCmd::Clip { shape: r.word()? },
                4 | 5 => {
                    let opcode = bytes[r.at - 1];
                    let a = r.word()?;
                    let paint = r.word()?;
                    let s = r.word()?;
                    let stroke = (s != u32::MAX).then_some(s);
                    if opcode == 4 {
                        DrawCmd::Path {
                            shape: a,
                            paint,
                            stroke,
                        }
                    } else {
                        DrawCmd::GlyphRun {
                            glyphs: a,
                            paint,
                            stroke,
                        }
                    }
                }
                6 => {
                    let image = r.word()?;
                    let mut dst = [0.; 4];
                    for f in &mut dst {
                        *f = r.float()?;
                    }
                    DrawCmd::Image { image, dst }
                }
                7 => DrawCmd::Effect { effect: r.word()? },
                8 => {
                    let opacity = r.float()?;
                    let blend = *BlendMode::ALL
                        .get(r.word()? as usize)
                        .ok_or(BufferError::Malformed)?;
                    DrawCmd::Layer { opacity, blend }
                }
                _ => return Err(BufferError::Malformed),
            });
        }
        for _ in 0..resources {
            let len = r.word()? as usize;
            let mut payload = Reader {
                b: r.take(len)?,
                at: 0,
            };
            let v = payload.value(0)?;
            if payload.at != len {
                return Err(BufferError::Malformed);
            }
            list.resources
                .push(serde_json::from_value(v).map_err(|_| BufferError::InvalidResource)?);
        }
        if r.at != bytes.len() {
            return Err(BufferError::Malformed);
        }
        list.validate()?;
        Ok(list)
    }
    /// Validates command state and typed resource references before either renderer consumes them.
    pub fn validate(&self) -> Result<(), BufferError> {
        let get = |i: u32| {
            self.resources
                .get(i as usize)
                .ok_or(BufferError::InvalidResource)
        };
        let mut stack = Vec::new();
        for cmd in &self.commands {
            match cmd {
                DrawCmd::Save => stack.push(false),
                DrawCmd::Layer { opacity, .. } => {
                    if !opacity.is_finite() || !(0. ..=1.).contains(opacity) {
                        return Err(BufferError::InvalidNumber);
                    }
                    stack.push(true)
                }
                DrawCmd::Restore => {
                    stack.pop().ok_or(BufferError::Unbalanced)?;
                }
                DrawCmd::Transform(m) => {
                    if m.iter().any(|f| !f.is_finite()) {
                        return Err(BufferError::InvalidNumber);
                    }
                }
                DrawCmd::Clip { shape } => {
                    if !matches!(get(*shape)?, Resource::Shape(_)) {
                        return Err(BufferError::InvalidResource);
                    }
                }
                DrawCmd::Path {
                    shape,
                    paint,
                    stroke,
                }
                | DrawCmd::GlyphRun {
                    glyphs: shape,
                    paint,
                    stroke,
                } => {
                    if !matches!(get(*paint)?, Resource::Paint(_))
                        || stroke.is_some_and(|s| !matches!(get(s), Ok(Resource::Stroke(_))))
                    {
                        return Err(BufferError::InvalidResource);
                    }
                    let valid = match cmd {
                        DrawCmd::Path { .. } => matches!(get(*shape)?, Resource::Shape(_)),
                        _ => matches!(get(*shape)?, Resource::Glyphs(_)),
                    };
                    if !valid {
                        return Err(BufferError::InvalidResource);
                    }
                }
                DrawCmd::Image { image, dst } => {
                    if !matches!(get(*image)?, Resource::Image(_)) {
                        return Err(BufferError::InvalidResource);
                    }
                    if dst.iter().any(|f| !f.is_finite()) || dst[2] < 0. || dst[3] < 0. {
                        return Err(BufferError::InvalidNumber);
                    }
                }
                DrawCmd::Effect { effect } => {
                    if !matches!(get(*effect)?, Resource::Effect(_)) || stack.last() != Some(&true)
                    {
                        return Err(BufferError::InvalidResource);
                    }
                }
            }
        }
        if !stack.is_empty() {
            return Err(BufferError::Unbalanced);
        }
        Ok(())
    }
}
