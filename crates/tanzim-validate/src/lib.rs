#![doc = include_str!("../README.md")]

mod error;

#[cfg(feature = "boolean")]
mod boolean;
#[cfg(feature = "either")]
mod either;
#[cfg(feature = "enumeration")]
mod enumeration;
#[cfg(feature = "float")]
mod float;
#[cfg(feature = "integer")]
mod integer;
#[cfg(feature = "list")]
mod list;
#[cfg(feature = "net")]
mod net;
#[cfg(feature = "non_empty")]
mod non_empty;
#[cfg(feature = "number")]
mod number;
#[cfg(feature = "path")]
mod path;
#[cfg(feature = "percentage")]
mod percentage;
#[cfg(feature = "static_map")]
mod static_map;
#[cfg(feature = "string")]
mod string;

#[cfg(feature = "bytesize")]
mod bytesize;
#[cfg(feature = "cidr")]
mod cidr;
#[cfg(feature = "datetime")]
mod datetime;
#[cfg(feature = "duration")]
mod duration;
#[cfg(feature = "dynamic_map")]
mod dynamic_map;
#[cfg(feature = "encoding")]
mod encoding;
#[cfg(feature = "regex")]
mod regex;
#[cfg(feature = "schema")]
mod schema;
#[cfg(feature = "semver")]
mod semver;
#[cfg(feature = "url")]
mod url;
#[cfg(feature = "uuid")]
mod uuid;

pub use error::{Error, ErrorKind, Segment};
pub use tanzim_value::{LocatedValue, Location, Map, Value, ValueType};

#[cfg(feature = "boolean")]
pub use boolean::Bool;
#[cfg(feature = "dynamic_map")]
pub use dynamic_map::DynamicMap;
#[cfg(feature = "either")]
pub use either::Either;
#[cfg(feature = "enumeration")]
pub use enumeration::Enum;
#[cfg(feature = "float")]
pub use float::Float;
#[cfg(feature = "integer")]
pub use integer::Integer;
#[cfg(feature = "list")]
pub use list::List;
#[cfg(feature = "net")]
pub use net::{Domain, Email, Host, IpAddr, Port, SocketAddr};
#[cfg(feature = "non_empty")]
pub use non_empty::NonEmpty;
#[cfg(feature = "number")]
pub use number::Number;
#[cfg(feature = "path")]
pub use path::{Path, PathKind};
#[cfg(feature = "percentage")]
pub use percentage::Percentage;
#[cfg(feature = "static_map")]
pub use static_map::StaticMap;
#[cfg(feature = "string")]
pub use string::Str;

#[cfg(feature = "bytesize")]
pub use bytesize::ByteSize;
#[cfg(feature = "cidr")]
pub use cidr::Cidr;
#[cfg(feature = "datetime")]
pub use datetime::{Date, DateTime};
#[cfg(feature = "duration")]
pub use duration::Duration;
#[cfg(feature = "encoding")]
pub use encoding::{Base64, Hex};
#[cfg(feature = "regex")]
pub use regex::RegexPattern;
#[cfg(feature = "schema")]
pub use schema::{
    Constructor, Node, Registry, SchemaError, SchemaErrorKind, SchemaValue, build, build_value,
};
#[cfg(feature = "semver")]
pub use semver::Semver;
#[cfg(feature = "url")]
pub use url::Url;
#[cfg(feature = "uuid")]
pub use uuid::Uuid;

/// Validates and optionally coerces a [`Value`] in place.
///
/// The method receives `&mut Value` (not [`LocatedValue`]) so it can rewrite the value when
/// coercing — e.g. a numeric string into an integer, or an empty map into an empty list.
/// Composite validators recurse into their `LocatedValue` children, so the child's
/// [`Location`] is still available to attach to any [`Error`] via
/// [`Error::under_key`]/[`Error::under_index`].
///
/// Validators returned by leaf checks carry no location; the enclosing value owns that and
/// fills it in. Use the free [`validate`] function to validate a whole node and seed the
/// root location.
pub trait Validator {
    /// Validate `value`, mutating it in place when coercion applies.
    fn validate(&self, value: &mut Value) -> Result<(), Error>;
}

impl<V: Validator + 'static> From<V> for Box<dyn Validator> {
    fn from(validator: V) -> Self {
        Box::new(validator)
    }
}

/// Validate a whole node, seeding the root [`Location`] into any error.
///
/// ```
/// use tanzim_validate::{validate, Integer};
/// use tanzim_value::{LocatedValue, Location, Value};
///
/// let mut node = LocatedValue {
///     value: Value::String("42".into()),
///     location: Location::at("file", "config.toml", Some(1), Some(1), None),
/// };
/// validate(&Integer::new().range(0, 100), &mut node).unwrap();
/// assert_eq!(node.value.as_int(), Some(42)); // coerced from string
/// ```
pub fn validate(validator: &dyn Validator, value: &mut LocatedValue) -> Result<(), Error> {
    match validator.validate(&mut value.value) {
        Ok(()) => Ok(()),
        Err(error) => Err(error.with_location(&value.location)),
    }
}
