//! An ordered JSON tree, and the serde `Serializer` that builds it.
//!
//! `serde_json::Value` keeps object keys sorted, which would lose the order the types declare their fields in
//! (`type`, `id`, `name`, ... first). Building the tree with our own serializer keeps struct fields in
//! declaration order, and lets the codec round numbers before anything is printed.

use std::fmt;

use serde::ser::{self, Serialize};

/// A JSON value whose objects keep the order they were built in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tree {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Text(String),
    Array(Vec<Tree>),
    Object(Vec<(String, Tree)>),
}

/// A step on the way to a value, for naming where a fault is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    Key(String),
    Index(usize),
}

/// `pages[0].children[1].width`.
pub(crate) fn render_path(steps: &[Step]) -> String {
    let mut out = String::new();
    for step in steps {
        match step {
            Step::Key(key) => {
                if !out.is_empty() {
                    out.push('.');
                }
                out.push_str(key);
            }
            Step::Index(i) => out.push_str(&format!("[{i}]")),
        }
    }
    out
}

/// Why building the tree failed. The path is filled in as the error travels back up.
#[derive(Debug)]
pub(crate) struct BuildError {
    pub(crate) message: String,
    /// Innermost first while travelling; reversed by [`BuildError::steps`].
    reversed: Vec<Step>,
}

impl BuildError {
    pub(crate) fn steps(&self) -> Vec<Step> {
        self.reversed.iter().rev().cloned().collect()
    }

    fn at(mut self, step: Step) -> Self {
        self.reversed.push(step);
        self
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BuildError {}

impl ser::Error for BuildError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self {
            message: message.to_string(),
            reversed: Vec::new(),
        }
    }
}

/// Builds a [`Tree`] from anything serializable.
pub(crate) fn to_tree<T: Serialize + ?Sized>(value: &T) -> Result<Tree, BuildError> {
    value.serialize(Builder)
}

struct Builder;

impl ser::Serializer for Builder {
    type Ok = Tree;
    type Error = BuildError;
    type SerializeSeq = SeqBuilder;
    type SerializeTuple = SeqBuilder;
    type SerializeTupleStruct = SeqBuilder;
    type SerializeTupleVariant = VariantSeqBuilder;
    type SerializeMap = MapBuilder;
    type SerializeStruct = StructBuilder;
    type SerializeStructVariant = VariantStructBuilder;

    fn serialize_bool(self, v: bool) -> Result<Tree, BuildError> {
        Ok(Tree::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Tree, BuildError> {
        Ok(Tree::Int(v.into()))
    }

    fn serialize_i16(self, v: i16) -> Result<Tree, BuildError> {
        Ok(Tree::Int(v.into()))
    }

    fn serialize_i32(self, v: i32) -> Result<Tree, BuildError> {
        Ok(Tree::Int(v.into()))
    }

    fn serialize_i64(self, v: i64) -> Result<Tree, BuildError> {
        Ok(Tree::Int(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Tree, BuildError> {
        Ok(Tree::Uint(v.into()))
    }

    fn serialize_u16(self, v: u16) -> Result<Tree, BuildError> {
        Ok(Tree::Uint(v.into()))
    }

    fn serialize_u32(self, v: u32) -> Result<Tree, BuildError> {
        Ok(Tree::Uint(v.into()))
    }

    fn serialize_u64(self, v: u64) -> Result<Tree, BuildError> {
        Ok(Tree::Uint(v))
    }

    fn serialize_f32(self, v: f32) -> Result<Tree, BuildError> {
        Ok(Tree::Float(v.into()))
    }

    fn serialize_f64(self, v: f64) -> Result<Tree, BuildError> {
        Ok(Tree::Float(v))
    }

    fn serialize_char(self, v: char) -> Result<Tree, BuildError> {
        Ok(Tree::Text(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Tree, BuildError> {
        Ok(Tree::Text(v.to_owned()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Tree, BuildError> {
        Ok(Tree::Array(
            v.iter().map(|b| Tree::Uint(u64::from(*b))).collect(),
        ))
    }

    fn serialize_none(self) -> Result<Tree, BuildError> {
        Ok(Tree::Null)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Tree, BuildError> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Tree, BuildError> {
        Ok(Tree::Null)
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<Tree, BuildError> {
        Ok(Tree::Null)
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<Tree, BuildError> {
        Ok(Tree::Text(variant.to_owned()))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<Tree, BuildError> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Tree, BuildError> {
        let inner = value
            .serialize(Builder)
            .map_err(|e| e.at(Step::Key(variant.to_owned())))?;
        Ok(Tree::Object(vec![(variant.to_owned(), inner)]))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<SeqBuilder, BuildError> {
        Ok(SeqBuilder(Vec::with_capacity(len.unwrap_or(0))))
    }

    fn serialize_tuple(self, len: usize) -> Result<SeqBuilder, BuildError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<SeqBuilder, BuildError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<VariantSeqBuilder, BuildError> {
        Ok(VariantSeqBuilder {
            variant,
            items: SeqBuilder(Vec::with_capacity(len)),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<MapBuilder, BuildError> {
        Ok(MapBuilder {
            entries: Vec::with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }

    fn serialize_struct(self, _: &'static str, len: usize) -> Result<StructBuilder, BuildError> {
        Ok(StructBuilder(Vec::with_capacity(len)))
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<VariantStructBuilder, BuildError> {
        Ok(VariantStructBuilder {
            variant,
            fields: StructBuilder(Vec::with_capacity(len)),
        })
    }
}

struct SeqBuilder(Vec<Tree>);

impl SeqBuilder {
    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), BuildError> {
        let index = self.0.len();
        let item = value
            .serialize(Builder)
            .map_err(|e| e.at(Step::Index(index)))?;
        self.0.push(item);
        Ok(())
    }
}

impl ser::SerializeSeq for SeqBuilder {
    type Ok = Tree;
    type Error = BuildError;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), BuildError> {
        self.push(value)
    }

    fn end(self) -> Result<Tree, BuildError> {
        Ok(Tree::Array(self.0))
    }
}

impl ser::SerializeTuple for SeqBuilder {
    type Ok = Tree;
    type Error = BuildError;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), BuildError> {
        self.push(value)
    }

    fn end(self) -> Result<Tree, BuildError> {
        Ok(Tree::Array(self.0))
    }
}

impl ser::SerializeTupleStruct for SeqBuilder {
    type Ok = Tree;
    type Error = BuildError;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), BuildError> {
        self.push(value)
    }

    fn end(self) -> Result<Tree, BuildError> {
        Ok(Tree::Array(self.0))
    }
}

struct VariantSeqBuilder {
    variant: &'static str,
    items: SeqBuilder,
}

impl ser::SerializeTupleVariant for VariantSeqBuilder {
    type Ok = Tree;
    type Error = BuildError;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), BuildError> {
        self.items
            .push(value)
            .map_err(|e| e.at(Step::Key(self.variant.to_owned())))
    }

    fn end(self) -> Result<Tree, BuildError> {
        Ok(Tree::Object(vec![(
            self.variant.to_owned(),
            Tree::Array(self.items.0),
        )]))
    }
}

/// A struct keeps its fields in the order the type declares them.
struct StructBuilder(Vec<(String, Tree)>);

impl StructBuilder {
    fn push<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), BuildError> {
        let item = value
            .serialize(Builder)
            .map_err(|e| e.at(Step::Key(key.to_owned())))?;
        self.0.push((key.to_owned(), item));
        Ok(())
    }
}

impl ser::SerializeStruct for StructBuilder {
    type Ok = Tree;
    type Error = BuildError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), BuildError> {
        self.push(key, value)
    }

    fn end(self) -> Result<Tree, BuildError> {
        Ok(Tree::Object(self.0))
    }
}

struct VariantStructBuilder {
    variant: &'static str,
    fields: StructBuilder,
}

impl ser::SerializeStructVariant for VariantStructBuilder {
    type Ok = Tree;
    type Error = BuildError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), BuildError> {
        self.fields
            .push(key, value)
            .map_err(|e| e.at(Step::Key(self.variant.to_owned())))
    }

    fn end(self) -> Result<Tree, BuildError> {
        Ok(Tree::Object(vec![(
            self.variant.to_owned(),
            Tree::Object(self.fields.0),
        )]))
    }
}

/// A map prints its keys sorted, whatever order it was filled in or iterates in (a `HashMap`, or a
/// `serde_json::Map` built with `preserve_order` by some other crate in the build). Byte order, so the file
/// does not depend on a type's `Ord`.
struct MapBuilder {
    entries: Vec<(String, Tree)>,
    key: Option<String>,
}

impl ser::SerializeMap for MapBuilder {
    type Ok = Tree;
    type Error = BuildError;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), BuildError> {
        self.key = Some(key.serialize(KeyBuilder)?);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), BuildError> {
        let key = self
            .key
            .take()
            .ok_or_else(|| <BuildError as ser::Error>::custom("a map value came before its key"))?;
        let item = value
            .serialize(Builder)
            .map_err(|e| e.at(Step::Key(key.clone())))?;
        self.entries.push((key, item));
        Ok(())
    }

    fn end(mut self) -> Result<Tree, BuildError> {
        self.entries
            .sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        Ok(Tree::Object(self.entries))
    }
}

/// Map keys are text; a number or a character is written as its text, as JSON requires.
struct KeyBuilder;

impl KeyBuilder {
    fn refuse() -> BuildError {
        <BuildError as ser::Error>::custom("a map key must be text")
    }
}

macro_rules! key_as_text {
    ($($method:ident($ty:ty)),* $(,)?) => {
        $(fn $method(self, v: $ty) -> Result<String, BuildError> { Ok(v.to_string()) })*
    };
}

macro_rules! key_refused {
    ($($method:ident($($arg:ty),*) -> $ret:ty),* $(,)?) => {
        $(fn $method(self, $(_: $arg),*) -> Result<$ret, BuildError> { Err(Self::refuse()) })*
    };
}

impl ser::Serializer for KeyBuilder {
    type Ok = String;
    type Error = BuildError;
    type SerializeSeq = ser::Impossible<String, BuildError>;
    type SerializeTuple = ser::Impossible<String, BuildError>;
    type SerializeTupleStruct = ser::Impossible<String, BuildError>;
    type SerializeTupleVariant = ser::Impossible<String, BuildError>;
    type SerializeMap = ser::Impossible<String, BuildError>;
    type SerializeStruct = ser::Impossible<String, BuildError>;
    type SerializeStructVariant = ser::Impossible<String, BuildError>;

    key_as_text!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_char(char),
        serialize_str(&str),
    );

    key_refused!(
        serialize_f32(f32) -> String,
        serialize_f64(f64) -> String,
        serialize_bytes(&[u8]) -> String,
        serialize_none() -> String,
        serialize_unit() -> String,
        serialize_unit_struct(&'static str) -> String,
        serialize_seq(Option<usize>) -> Self::SerializeSeq,
        serialize_tuple(usize) -> Self::SerializeTuple,
        serialize_tuple_struct(&'static str, usize) -> Self::SerializeTupleStruct,
        serialize_tuple_variant(&'static str, u32, &'static str, usize) -> Self::SerializeTupleVariant,
        serialize_map(Option<usize>) -> Self::SerializeMap,
        serialize_struct(&'static str, usize) -> Self::SerializeStruct,
        serialize_struct_variant(&'static str, u32, &'static str, usize) -> Self::SerializeStructVariant,
    );

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<String, BuildError> {
        value.serialize(self)
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<String, BuildError> {
        Ok(variant.to_owned())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<String, BuildError> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<String, BuildError> {
        Err(Self::refuse())
    }
}
