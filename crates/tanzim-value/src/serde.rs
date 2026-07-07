//! [`serde`] deserialization support (`serde` Cargo feature).
//!
//! Implements [`serde::Deserializer`] over the configuration tree so a [`Value`] or
//! [`LocatedValue`] can be turned into any [`serde::Deserialize`] type. [`LocatedValue`] drives the
//! same [`Value`] deserializer but, on failure, stamps the offending node's [`crate::Location`]
//! onto the error (see [`crate::Error::Deserialize`]). This is the mirror image of
//! `tanzim-validate`'s `SchemaValue` visitor, which goes serde → [`Value`].

use crate::{Error, LocatedValue, Value};
use serde::de::{
    Deserialize, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use serde::forward_to_deserialize_any;
use std::fmt::Display;

/// Build an unlocated deserialize error; the nearest [`LocatedValue`] fills in the location.
fn custom(message: impl Display) -> Error {
    Error::Deserialize {
        message: message.to_string(),
        location: None,
    }
}

impl serde::de::Error for Error {
    fn custom<T: Display>(message: T) -> Self {
        custom(message)
    }
}

// --- convenience API ---

impl Value {
    /// Deserialize this value into `T`. Errors carry no source location; prefer
    /// [`LocatedValue::try_deserialize`] when a location is available.
    pub fn try_deserialize<'de, T: Deserialize<'de>>(&'de self) -> Result<T, Error> {
        T::deserialize(self)
    }
}

impl LocatedValue {
    /// Deserialize this value into `T`, attaching the nearest node's [`crate::Location`] to any
    /// error that does not already carry one.
    pub fn try_deserialize<'de, T: Deserialize<'de>>(&'de self) -> Result<T, Error> {
        T::deserialize(self)
    }
}

// --- Deserializer for &Value (the workhorse) ---

impl<'de> Deserializer<'de> for &'de Value {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Bool(value) => visitor.visit_bool(*value),
            Value::Int(value) => visitor.visit_i64(*value as i64),
            Value::Float(value) => visitor.visit_f64(*value),
            Value::String(value) => visitor.visit_borrowed_str(value),
            Value::List(items) => visitor.visit_seq(SeqDeserializer::new(items)),
            Value::Map(map) => visitor.visit_map(MapDeserializer::new(map.entries())),
            Value::Null => visitor.visit_unit(),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_enum(EnumDeserializer::new(self)?)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}

// --- Deserializer for &LocatedValue (delegates to &Value, stamps location on error) ---

impl<'de> Deserializer<'de> for &'de LocatedValue {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.value()
            .deserialize_any(visitor)
            .map_err(|error| error.or_location(self.location()))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.value()
            .deserialize_option(visitor)
            .map_err(|error| error.or_location(self.location()))
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.value()
            .deserialize_newtype_struct(name, visitor)
            .map_err(|error| error.or_location(self.location()))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.value()
            .deserialize_enum(name, variants, visitor)
            .map_err(|error| error.or_location(self.location()))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}

// --- access helpers ---

struct SeqDeserializer<'de> {
    iter: std::slice::Iter<'de, LocatedValue>,
}

impl<'de> SeqDeserializer<'de> {
    fn new(items: &'de [LocatedValue]) -> Self {
        Self { iter: items.iter() }
    }
}

impl<'de> SeqAccess<'de> for SeqDeserializer<'de> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        match self.iter.next() {
            Some(element) => seed.deserialize(element).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

struct MapDeserializer<'de> {
    iter: std::slice::Iter<'de, (String, LocatedValue)>,
    value: Option<&'de LocatedValue>,
}

impl<'de> MapDeserializer<'de> {
    fn new(entries: &'de [(String, LocatedValue)]) -> Self {
        Self {
            iter: entries.iter(),
            value: None,
        }
    }
}

impl<'de> MapAccess<'de> for MapDeserializer<'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.iter.next() {
            Some((key, value)) => {
                self.value = Some(value);
                seed.deserialize(MapKeyDeserializer { key }).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let value = self
            .value
            .take()
            .expect("next_value_seed called before next_key_seed");
        seed.deserialize(value)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

/// Deserializes a map key (always a string) into an identifier or `String`.
struct MapKeyDeserializer<'de> {
    key: &'de str,
}

impl<'de> Deserializer<'de> for MapKeyDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_borrowed_str(self.key)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

/// Externally-tagged enum access: a bare string is a unit variant; a single-entry map is a variant
/// carrying a payload.
struct EnumDeserializer<'de> {
    variant: &'de str,
    value: Option<&'de LocatedValue>,
}

impl<'de> EnumDeserializer<'de> {
    fn new(value: &'de Value) -> Result<Self, Error> {
        match value {
            Value::String(variant) => Ok(Self {
                variant,
                value: None,
            }),
            Value::Map(map) if map.len() == 1 => {
                let (variant, value) = &map.entries()[0];
                Ok(Self {
                    variant,
                    value: Some(value),
                })
            }
            other => Err(custom(format!(
                "cannot deserialize enum from {}",
                other.type_name()
            ))),
        }
    }
}

impl<'de> EnumAccess<'de> for EnumDeserializer<'de> {
    type Error = Error;
    type Variant = VariantDeserializer<'de>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let variant = seed.deserialize(MapKeyDeserializer { key: self.variant })?;
        Ok((variant, VariantDeserializer { value: self.value }))
    }
}

struct VariantDeserializer<'de> {
    value: Option<&'de LocatedValue>,
}

impl<'de> VariantAccess<'de> for VariantDeserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.value {
            None => Ok(()),
            Some(_) => Err(custom("expected a unit variant, found a payload")),
        }
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        match self.value {
            Some(value) => seed.deserialize(value),
            None => Err(custom("expected a newtype variant payload")),
        }
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        match self.value {
            Some(value) => value.deserialize_any(visitor),
            None => Err(custom("expected a tuple variant payload")),
        }
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.value {
            Some(value) => value.deserialize_any(visitor),
            None => Err(custom("expected a struct variant payload")),
        }
    }
}
