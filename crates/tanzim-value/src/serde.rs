//! [`serde`] support (`serde` Cargo feature): deserialize a [`Value`] into `T`, and serialize a
//! `T` into a [`Value`] / [`LocatedValue`] tree.
//!
//! [`LocatedValue`] deserialization stamps the offending node's [`crate::Location`] onto errors
//! (see [`crate::Error::Deserialize`]). Serialization stamps every produced node with the
//! caller-supplied location — the path used for programmatic defaults (`Location = "defaults"`).

use crate::{Error, LocatedValue, Location, Map, Value};
use serde::de::{
    Deserialize, DeserializeSeed, Deserializer, EnumAccess, MapAccess, SeqAccess, VariantAccess,
    Visitor,
};
use serde::forward_to_deserialize_any;
use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant, Serializer,
};
use std::fmt::Display;

/// Build an unlocated deserialize error; the nearest [`LocatedValue`] fills in the location.
fn custom(message: impl Display) -> Error {
    Error::Deserialize {
        message: message.to_string(),
        location: None,
    }
}

/// Build a serialize error.
fn serialize_error(message: impl Display) -> Error {
    Error::Serialize {
        message: message.to_string(),
    }
}

impl serde::de::Error for Error {
    fn custom<T: Display>(message: T) -> Self {
        custom(message)
    }
}

impl serde::ser::Error for Error {
    fn custom<T: Display>(message: T) -> Self {
        serialize_error(message)
    }
}

// --- convenience API ---

impl Value {
    /// Deserialize this value into `T`.
    ///
    /// Errors carry no source location. Prefer [`LocatedValue::try_deserialize`] when the tree
    /// was produced by a parser (or [`LocatedValue::try_from_serialize`]) so failures can point at
    /// a file, line, and column.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Deserialize`] on a type mismatch or other serde failure.
    ///
    /// ```
    /// # #[cfg(feature = "serde")] {
    /// use serde::Deserialize;
    /// use tanzim_value::{LocatedValue, Location, Map, Value};
    ///
    /// #[derive(Deserialize, Debug, PartialEq)]
    /// struct Settings {
    ///     host: String,
    ///     port: u16,
    /// }
    ///
    /// let location = Location::at("test", "", None, None, None);
    /// let mut map = Map::new();
    /// map.insert(
    ///     "host".into(),
    ///     LocatedValue::new(Value::String("localhost".into()), location.clone()),
    /// );
    /// map.insert(
    ///     "port".into(),
    ///     LocatedValue::new(Value::Int(8080), location),
    /// );
    ///
    /// let settings: Settings = Value::Map(map).try_deserialize().unwrap();
    /// assert_eq!(
    ///     settings,
    ///     Settings {
    ///         host: "localhost".into(),
    ///         port: 8080,
    ///     }
    /// );
    /// # }
    /// ```
    pub fn try_deserialize<'de, T: Deserialize<'de>>(&'de self) -> Result<T, Error> {
        T::deserialize(self)
    }

    /// Serialize `value` into a bare [`Value`] tree.
    ///
    /// Nested list/map children are stamped with an empty synthetic [`Location`]. Prefer
    /// [`LocatedValue::try_from_serialize`] when provenance matters (e.g. programmatic defaults).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Serialize`] when a map key is not a string, or an integer does not fit in
    /// [`isize`].
    ///
    /// ```
    /// # #[cfg(feature = "serde")] {
    /// use serde::Serialize;
    /// use tanzim_value::Value;
    ///
    /// #[derive(Serialize)]
    /// struct Settings {
    ///     host: String,
    ///     port: u16,
    /// }
    ///
    /// let value = Value::try_from_serialize(&Settings {
    ///     host: "localhost".into(),
    ///     port: 8080,
    /// })
    /// .unwrap();
    ///
    /// let map = value.as_map().unwrap();
    /// assert_eq!(map.get("host").unwrap().value().as_string().unwrap(), "localhost");
    /// assert_eq!(map.get("port").unwrap().value().as_int(), Some(8080));
    /// # }
    /// ```
    pub fn try_from_serialize<T: Serialize>(value: &T) -> Result<Self, Error> {
        value.serialize(ValueSerializer {
            location: Location::at("", "", None, None, None),
        })
    }
}

impl LocatedValue {
    /// Deserialize this value into `T`, attaching the nearest node's [`Location`] to any error that
    /// does not already carry one.
    ///
    /// On failure, `{error}` is a one-line summary that includes the location; `{error:#}` also
    /// appends the pre-rendered source snippet with a caret underline when the location has one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Deserialize`] on a type mismatch or other serde failure. The error's
    /// location is taken from the nearest enclosing [`LocatedValue`] when the leaf itself has none.
    ///
    /// ```
    /// # #[cfg(feature = "serde")] {
    /// use serde::Deserialize;
    /// use tanzim_value::{LocatedValue, Location, Map, Value};
    ///
    /// #[derive(Deserialize, Debug, PartialEq)]
    /// struct Settings {
    ///     host: String,
    ///     port: u16,
    /// }
    ///
    /// let location = Location::at("file", "app.toml", Some(1), Some(1), None);
    /// let mut map = Map::new();
    /// map.insert(
    ///     "host".into(),
    ///     LocatedValue::new(Value::String("localhost".into()), location.clone()),
    /// );
    /// map.insert(
    ///     "port".into(),
    ///     LocatedValue::new(Value::Int(8080), location.clone()),
    /// );
    /// let tree = LocatedValue::new(Value::Map(map), location);
    ///
    /// let settings: Settings = tree.try_deserialize().unwrap();
    /// assert_eq!(settings.host, "localhost");
    /// assert_eq!(settings.port, 8080);
    ///
    /// // A type mismatch surfaces as Error::Deserialize with the offending node's location.
    /// #[derive(Deserialize, Debug)]
    /// struct PortOnly {
    ///     port: String,
    /// }
    /// let error = tree.try_deserialize::<PortOnly>().unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     tanzim_value::Error::Deserialize {
    ///         location: Some(_),
    ///         ..
    ///     }
    /// ));
    /// assert!(error.to_string().contains("file:app.toml"));
    /// # }
    /// ```
    pub fn try_deserialize<'de, T: Deserialize<'de>>(&'de self) -> Result<T, Error> {
        T::deserialize(self)
    }

    /// Serialize `value` into a located tree, stamping `location` onto every node (root and nested).
    ///
    /// This is the bridge used for programmatic defaults: stamp a synthetic origin with
    /// [`Location::at`] (e.g. `"defaults"`) so "where did this value come from?" answers with a
    /// built-in origin rather than a file or env source.
    ///
    /// Mapping rules:
    /// - structs / maps → [`Value::Map`] (keys must serialize as strings)
    /// - sequences / tuples → [`Value::List`]
    /// - `None` / unit → [`Value::Null`]
    /// - unit enum variants → [`Value::String`] (variant name)
    ///
    /// # Errors
    ///
    /// Returns [`Error::Serialize`] when a map key is not a string, or an integer does not fit in
    /// [`isize`].
    ///
    /// ```
    /// # #[cfg(feature = "serde")] {
    /// use serde::Serialize;
    /// use tanzim_value::{LocatedValue, Location};
    ///
    /// #[derive(Serialize)]
    /// struct Settings {
    ///     host: String,
    ///     port: u16,
    /// }
    ///
    /// let tree = LocatedValue::try_from_serialize(
    ///     &Settings {
    ///         host: "localhost".into(),
    ///         port: 8080,
    ///     },
    ///     Location::at("defaults", "", None, None, None),
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(tree.location().source_name(), "defaults");
    /// let port = tree.value().as_map().unwrap().get("port").unwrap();
    /// assert_eq!(port.value().as_int(), Some(8080));
    /// assert_eq!(port.location().source_name(), "defaults");
    /// # }
    /// ```
    pub fn try_from_serialize<T: Serialize>(value: &T, location: Location) -> Result<Self, Error> {
        match value.serialize(ValueSerializer {
            location: location.clone(),
        }) {
            Ok(tree) => Ok(LocatedValue::new(tree, location)),
            Err(error) => Err(error),
        }
    }
}

// --- Serializer: T → Value ---

struct ValueSerializer {
    location: Location,
}

impl ValueSerializer {
    fn located(&self, value: Value) -> LocatedValue {
        LocatedValue::new(value, self.location.clone())
    }
}

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = ListSerializer;
    type SerializeTuple = ListSerializer;
    type SerializeTupleStruct = ListSerializer;
    type SerializeTupleVariant = VariantListSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = MapSerializer;
    type SerializeStructVariant = VariantMapSerializer;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bool(value))
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(value as isize))
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(value as isize))
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(value as isize))
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        match isize::try_from(value) {
            Ok(number) => Ok(Value::Int(number)),
            Err(_) => Err(serialize_error("integer out of range")),
        }
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(value as isize))
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int(value as isize))
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        match isize::try_from(value) {
            Ok(number) => Ok(Value::Int(number)),
            Err(_) => Err(serialize_error("integer out of range")),
        }
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        match isize::try_from(value) {
            Ok(number) => Ok(Value::Int(number)),
            Err(_) => Err(serialize_error("integer out of range")),
        }
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(f64::from(value)))
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(value))
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(value.to_string()))
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(value.to_string()))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        let mut items = Vec::with_capacity(value.len());
        for byte in value {
            items.push(self.located(Value::Int(*byte as isize)));
        }
        Ok(Value::List(items))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(variant.to_string()))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        match value.serialize(ValueSerializer {
            location: self.location.clone(),
        }) {
            Ok(inner) => {
                let mut map = Map::new();
                map.insert(variant.to_string(), self.located(inner));
                Ok(Value::Map(map))
            }
            Err(error) => Err(error),
        }
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(ListSerializer {
            location: self.location,
            items: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(VariantListSerializer {
            location: self.location,
            variant,
            items: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(MapSerializer {
            location: self.location,
            map: Map::new(),
            next_key: None,
        })
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(VariantMapSerializer {
            location: self.location,
            variant,
            map: Map::new(),
        })
    }
}

struct ListSerializer {
    location: Location,
    items: Vec<LocatedValue>,
}

impl ListSerializer {
    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        match value.serialize(ValueSerializer {
            location: self.location.clone(),
        }) {
            Ok(inner) => {
                self.items
                    .push(LocatedValue::new(inner, self.location.clone()));
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn finish(self) -> Value {
        Value::List(self.items)
    }
}

impl SerializeSeq for ListSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.finish())
    }
}

impl SerializeTuple for ListSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.finish())
    }
}

impl SerializeTupleStruct for ListSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.finish())
    }
}

struct VariantListSerializer {
    location: Location,
    variant: &'static str,
    items: Vec<LocatedValue>,
}

impl SerializeTupleVariant for VariantListSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        match value.serialize(ValueSerializer {
            location: self.location.clone(),
        }) {
            Ok(inner) => {
                self.items
                    .push(LocatedValue::new(inner, self.location.clone()));
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut map = Map::new();
        map.insert(
            self.variant.to_string(),
            LocatedValue::new(Value::List(self.items), self.location),
        );
        Ok(Value::Map(map))
    }
}

struct MapSerializer {
    location: Location,
    map: Map,
    next_key: Option<String>,
}

impl SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Self::Error> {
        match key.serialize(MapKeySerializer) {
            Ok(key) => {
                self.next_key = Some(key);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Self::Error> {
        let key = match self.next_key.take() {
            Some(key) => key,
            None => {
                return Err(serialize_error(
                    "serialize_value called before serialize_key",
                ));
            }
        };
        match value.serialize(ValueSerializer {
            location: self.location.clone(),
        }) {
            Ok(inner) => {
                self.map
                    .insert(key, LocatedValue::new(inner, self.location.clone()));
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Map(self.map))
    }
}

impl SerializeStruct for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        match value.serialize(ValueSerializer {
            location: self.location.clone(),
        }) {
            Ok(inner) => {
                self.map.insert(
                    key.to_string(),
                    LocatedValue::new(inner, self.location.clone()),
                );
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Map(self.map))
    }
}

struct VariantMapSerializer {
    location: Location,
    variant: &'static str,
    map: Map,
}

impl SerializeStructVariant for VariantMapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        match value.serialize(ValueSerializer {
            location: self.location.clone(),
        }) {
            Ok(inner) => {
                self.map.insert(
                    key.to_string(),
                    LocatedValue::new(inner, self.location.clone()),
                );
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        let mut outer = Map::new();
        outer.insert(
            self.variant.to_string(),
            LocatedValue::new(Value::Map(self.map), self.location),
        );
        Ok(Value::Map(outer))
    }
}

/// Map keys must serialize as strings (configuration trees are string-keyed).
struct MapKeySerializer;

impl Serializer for MapKeySerializer {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = serde::ser::Impossible<String, Error>;
    type SerializeTuple = serde::ser::Impossible<String, Error>;
    type SerializeTupleStruct = serde::ser::Impossible<String, Error>;
    type SerializeTupleVariant = serde::ser::Impossible<String, Error>;
    type SerializeMap = serde::ser::Impossible<String, Error>;
    type SerializeStruct = serde::ser::Impossible<String, Error>;
    type SerializeStructVariant = serde::ser::Impossible<String, Error>;

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        Ok(value.to_string())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(variant.to_string())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(serialize_error("map keys must be strings"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(serialize_error("map keys must be strings"))
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
