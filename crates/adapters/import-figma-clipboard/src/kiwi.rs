//! Kiwi, the binary format a Figma paste is written in: a schema, then a message in that schema. Only decoding,
//! and into plain JSON: enums become their names, byte arrays become base64 text, missing fields stay missing.
//! Written from the format's reference implementation (`kiwi-schema`, MIT, `ByteBuffer` and `decodeBinarySchema`).
use serde_json::{Map, Number, Value};

const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Enum,
    Struct,
    Message,
}

#[derive(Debug, Clone, PartialEq)]
enum Ty {
    Bool,
    Byte,
    Int,
    Uint,
    Float,
    Str,
    Int64,
    Uint64,
    Def(usize),
}

#[derive(Debug, Clone)]
struct Field {
    name: String,
    ty: Ty,
    array: bool,
    value: u32,
}

#[derive(Debug, Clone)]
struct Def {
    name: String,
    kind: Kind,
    fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Schema {
    defs: Vec<Def>,
}

/// A field as the schema file writes it: name, type number, array flag, field number.
type RawField = (String, i32, bool, u32);
type RawDef = (String, Kind, Vec<RawField>);

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

type Res<T> = Result<T, String>;

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Res<u8> {
        let v = *self.b.get(self.at).ok_or("the data ends too soon")?;
        self.at += 1;
        Ok(v)
    }

    fn var_uint(&mut self) -> Res<u32> {
        let (mut value, mut shift) = (0u32, 0u32);
        loop {
            let byte = self.byte()?;
            value |= u32::from(byte & 127).wrapping_shl(shift);
            shift += 7;
            if byte & 128 == 0 || shift >= 35 {
                return Ok(value);
            }
        }
    }

    fn var_int(&mut self) -> Res<i32> {
        let v = self.var_uint()? as i32;
        Ok(if v & 1 != 0 {
            !((v as u32 >> 1) as i32)
        } else {
            (v as u32 >> 1) as i32
        })
    }

    fn var_uint64(&mut self) -> Res<u64> {
        let (mut value, mut shift) = (0u64, 0u32);
        loop {
            let byte = self.byte()?;
            if byte & 128 == 0 || shift >= 56 {
                return Ok(value | u64::from(byte).wrapping_shl(shift));
            }
            value |= u64::from(byte & 127) << shift;
            shift += 7;
        }
    }

    fn var_int64(&mut self) -> Res<i64> {
        let v = self.var_uint64()?;
        Ok(if v & 1 != 0 {
            !((v >> 1) as i64)
        } else {
            (v >> 1) as i64
        })
    }

    fn var_float(&mut self) -> Res<f32> {
        let first = *self.b.get(self.at).ok_or("the data ends too soon")?;
        if first == 0 {
            self.at += 1;
            return Ok(0.);
        }
        let raw = self
            .b
            .get(self.at..self.at + 4)
            .ok_or("the data ends too soon")?;
        self.at += 4;
        let bits = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        Ok(f32::from_bits(bits.rotate_left(23)))
    }

    fn string(&mut self) -> Res<String> {
        let rest = self.b.get(self.at..).ok_or("the data ends too soon")?;
        let end = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or("a string is not finished")?;
        self.at += end + 1;
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }
}

impl Schema {
    pub fn parse(bytes: &[u8]) -> Res<Schema> {
        let mut r = Reader { b: bytes, at: 0 };
        let count = r.var_uint()? as usize;
        if count > 4096 {
            return Err("the schema has too many definitions".into());
        }
        let mut raw: Vec<RawDef> = Vec::new();
        for _ in 0..count {
            let name = r.string()?;
            let kind = match r.byte()? {
                0 => Kind::Enum,
                1 => Kind::Struct,
                2 => Kind::Message,
                other => return Err(format!("the schema has a definition of kind {other}")),
            };
            let fields = r.var_uint()? as usize;
            if fields > 65_536 {
                return Err("a definition has too many fields".into());
            }
            let mut list = Vec::with_capacity(fields);
            for _ in 0..fields {
                let field = r.string()?;
                let ty = r.var_int()?;
                let array = r.byte()? & 1 != 0;
                let value = r.var_uint()?;
                list.push((field, ty, array, value));
            }
            raw.push((name, kind, list));
        }
        let mut defs = Vec::with_capacity(raw.len());
        for (name, kind, fields) in &raw {
            let mut out = Vec::with_capacity(fields.len());
            for (field, ty, array, value) in fields {
                let ty = if *kind == Kind::Enum {
                    Ty::Byte
                } else if *ty < 0 {
                    match !*ty {
                        0 => Ty::Bool,
                        1 => Ty::Byte,
                        2 => Ty::Int,
                        3 => Ty::Uint,
                        4 => Ty::Float,
                        5 => Ty::Str,
                        6 => Ty::Int64,
                        7 => Ty::Uint64,
                        _ => return Err(format!("the schema uses an unknown type {ty}")),
                    }
                } else if (*ty as usize) < raw.len() {
                    Ty::Def(*ty as usize)
                } else {
                    return Err(format!("the schema uses an unknown type {ty}"));
                };
                out.push(Field {
                    name: field.clone(),
                    ty,
                    array: *array,
                    value: *value,
                });
            }
            defs.push(Def {
                name: name.clone(),
                kind: *kind,
                fields: out,
            });
        }
        Ok(Schema { defs })
    }

    /// Decodes `bytes` as the message type called `root`.
    pub fn decode(&self, root: &str, bytes: &[u8]) -> Res<Value> {
        let index = self
            .defs
            .iter()
            .position(|d| d.name == root)
            .ok_or_else(|| format!("the schema has no {root} message"))?;
        let mut r = Reader { b: bytes, at: 0 };
        self.def(&mut r, index, 0)
    }

    fn def(&self, r: &mut Reader<'_>, index: usize, depth: usize) -> Res<Value> {
        if depth > MAX_DEPTH {
            return Err("the data nests too deeply".into());
        }
        let def = &self.defs[index];
        match def.kind {
            Kind::Enum => {
                let n = r.var_uint()?;
                Ok(def
                    .fields
                    .iter()
                    .find(|f| f.value == n)
                    .map_or(Value::Null, |f| Value::String(f.name.clone())))
            }
            Kind::Struct => {
                let mut map = Map::new();
                for field in &def.fields {
                    map.insert(field.name.clone(), self.field(r, field, depth)?);
                }
                Ok(Value::Object(map))
            }
            Kind::Message => {
                let mut map = Map::new();
                loop {
                    let id = r.var_uint()?;
                    if id == 0 {
                        return Ok(Value::Object(map));
                    }
                    let field = def.fields.iter().find(|f| f.value == id).ok_or_else(|| {
                        format!("{} has a field {id} the schema does not know", def.name)
                    })?;
                    map.insert(field.name.clone(), self.field(r, field, depth)?);
                }
            }
        }
    }

    fn field(&self, r: &mut Reader<'_>, field: &Field, depth: usize) -> Res<Value> {
        if !field.array {
            return self.value(r, &field.ty, depth);
        }
        let len = r.var_uint()? as usize;
        if len > r.b.len().saturating_sub(r.at) {
            return Err("an array is longer than the data".into());
        }
        if field.ty == Ty::Byte {
            let bytes = r.b.get(r.at..r.at + len).ok_or("the data ends too soon")?;
            r.at += len;
            return Ok(Value::String(vellum_core::base64::encode(bytes)));
        }
        let mut items = Vec::with_capacity(len.min(4096));
        for _ in 0..len {
            items.push(self.value(r, &field.ty, depth)?);
        }
        Ok(Value::Array(items))
    }

    fn value(&self, r: &mut Reader<'_>, ty: &Ty, depth: usize) -> Res<Value> {
        let number = |f: f64| Number::from_f64(f).map_or(Value::Null, Value::Number);
        Ok(match ty {
            Ty::Bool => Value::Bool(r.byte()? != 0),
            Ty::Byte => Value::from(r.byte()?),
            Ty::Int => Value::from(r.var_int()?),
            Ty::Uint => Value::from(r.var_uint()?),
            Ty::Float => number(f64::from(r.var_float()?)),
            Ty::Str => Value::String(r.string()?),
            Ty::Int64 => Value::from(r.var_int64()?),
            Ty::Uint64 => Value::from(r.var_uint64()?),
            Ty::Def(i) => self.def(r, *i, depth + 1)?,
        })
    }
}
