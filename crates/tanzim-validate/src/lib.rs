#![doc = include_str!("../README.md")]

mod error;

mod boolean;
mod either;
mod enumeration;
mod float;
mod integer;
mod list;
mod net;
mod non_empty;
mod number;
mod path;
mod percentage;
mod static_map;
mod string;

#[cfg(feature = "bytesize")]
mod bytesize;
#[cfg(feature = "cidr")]
mod cidr;
#[cfg(feature = "datetime")]
mod datetime;
#[cfg(feature = "duration")]
mod duration;
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

pub use boolean::Bool;
pub use dynamic_map::DynamicMap;
pub use either::Either;
pub use enumeration::Enum;
pub use float::Float;
pub use integer::Integer;
pub use list::List;
pub use net::{Domain, Email, Host, IpAddr, Port, SocketAddr};
pub use non_empty::NonEmpty;
pub use number::Number;
pub use path::{Path, PathKind};
pub use percentage::Percentage;
pub use static_map::StaticMap;
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
