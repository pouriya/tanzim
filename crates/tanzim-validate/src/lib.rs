#![doc = include_str!("../README.md")]
#![deny(missing_docs)]

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

/// Human-facing metadata a validator carries and attaches to its errors.
///
/// Set through the builder methods (`with_name`, `with_description`, `with_default`,
/// `to_int`, …) available on every validator. On a validation failure a validator attaches its own
/// `Meta` to the [`Error`] (innermost wins), so messages can name the field and offer a description,
/// examples, and a default. `convert` requests a post-validation output cast (see
/// [`Validator::validate`]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Meta {
    /// The human-readable name, surfaced in error messages.
    pub name: String,
    /// A human-readable description.
    pub description: Option<String>,
    /// Example values, each with an optional note explaining it.
    pub examples: Vec<(Value, Option<String>)>,
    /// The default value used as an on-error fallback.
    pub default: Option<Value>,
    /// Target type for the post-validation output cast, if any.
    pub convert: Option<ValueType>,
}

impl Meta {
    /// A metadata block with just the name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
}

/// A validator: a rule ([`check`](Validator::check)) plus human-facing [`Meta`].
///
/// Each validator stores a [`Meta`] and returns it from [`meta`](Validator::meta). [`check`](Validator::meta) is the
/// rule; it receives `&mut Value` (not [`LocatedValue`]) so it can coerce in place — e.g. a numeric
/// string into an integer. [`validate`](Validator::validate) is provided: it runs `check`, attaches
/// this validator's [`Meta`] to any error (innermost wins), and applies the output conversion in
/// `meta().convert` on success. Composite validators recurse by calling `validate` on their
/// children, then attach the child's [`Location`] via [`Error::under_key`]/[`Error::under_index`].
///
/// Most callers reach for one of the [built-in validators](crate) via their fluent builders
/// (e.g. [`Integer`]) rather than implementing this trait directly; the built-ins all follow the
/// same `meta`/`meta_mut`/`check` shape shown below.
///
/// ```
/// # #[cfg(feature = "integer")]
/// # {
/// use tanzim_validate::{Integer, Validator};
/// use tanzim_value::Value;
///
/// let validator = Integer::new().range(0, 65535).with_name("Port");
/// let mut value = Value::String("8080".into());
/// validator.validate(&mut value).unwrap();
/// assert_eq!(value.as_int(), Some(8080)); // coerced from string
/// # }
/// ```
pub trait Validator {
    /// This validator's human-facing metadata.
    fn meta(&self) -> &Meta;

    /// Mutable access to this validator's metadata (backs the builder setters).
    fn meta_mut(&mut self) -> &mut Meta;

    /// The validation rule: check (and coerce) `value` in place.
    fn check(&self, value: &mut Value) -> Result<(), Error>;

    /// Run [`check`](Validator::check); on error attach this validator's [`Meta`] (innermost wins);
    /// on success apply the output conversion in `meta().convert`, if any.
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
        if matches!(value, Value::Null)
            && let Some(default) = self.meta().default.as_ref()
        {
            *value = default.clone();
        }
        if let Err(error) = self.check(value) {
            return Err(error.with_meta(self.meta()));
        }
        if let Some(target) = self.meta().convert {
            cast(value, target).map_err(|error| error.with_meta(self.meta()))?;
        }
        Ok(())
    }
}

impl<V: Validator + 'static> From<V> for Box<dyn Validator> {
    fn from(validator: V) -> Self {
        Box::new(validator)
    }
}

impl<V: Validator + Send + Sync + 'static> From<V> for Box<dyn Validator + Send + Sync> {
    fn from(validator: V) -> Self {
        Box::new(validator)
    }
}

/// Getters and fluent setters for every validator's [`Meta`].
///
/// Invoked via [`impl_meta_methods!`] on each concrete validator so
/// `Integer::new().with_name("Port").to_int()` works without importing a trait.
/// Getters read `meta()`; setters mutate `meta_mut()` and return `self` for chaining.
#[macro_export]
macro_rules! impl_meta_methods {
    ($ty:ty) => {
        #[allow(clippy::wrong_self_convention)]
        impl $ty {
            /// The human-readable name.
            pub fn name(&self) -> &str {
                &<$ty as $crate::Validator>::meta(self).name
            }

            /// The description, if any.
            pub fn description(&self) -> Option<&str> {
                <$ty as $crate::Validator>::meta(self)
                    .description
                    .as_deref()
            }

            /// The example values (each with an optional note).
            pub fn examples(&self) -> &[(tanzim_value::Value, Option<String>)] {
                &<$ty as $crate::Validator>::meta(self).examples
            }

            /// The default value, if any.
            pub fn default_value(&self) -> Option<&tanzim_value::Value> {
                <$ty as $crate::Validator>::meta(self).default.as_ref()
            }

            /// The output conversion target, if any.
            pub fn convert(&self) -> Option<tanzim_value::ValueType> {
                <$ty as $crate::Validator>::meta(self).convert
            }

            /// Set the human-readable name (surfaced in error messages).
            pub fn with_name(mut self, name: impl Into<String>) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self).name = name.into();
                self
            }

            /// Attach a human-readable description.
            pub fn with_description(mut self, text: impl Into<String>) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self).description = Some(text.into());
                self
            }

            /// Add an example value.
            pub fn with_example(mut self, value: impl Into<tanzim_value::Value>) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self)
                    .examples
                    .push((value.into(), None));
                self
            }

            /// Add an example value with an explanatory note.
            pub fn with_example_noted(
                mut self,
                value: impl Into<tanzim_value::Value>,
                note: impl Into<String>,
            ) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self)
                    .examples
                    .push((value.into(), Some(note.into())));
                self
            }

            /// Set the default value used as an on-error fallback (see the pipeline's validate stage).
            pub fn with_default(mut self, value: impl Into<tanzim_value::Value>) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self).default = Some(value.into());
                self
            }

            /// After validation succeeds, cast the value to a string.
            pub fn to_string(mut self) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self).convert =
                    Some(tanzim_value::ValueType::String);
                self
            }

            /// After validation succeeds, cast the value to an integer.
            pub fn to_int(mut self) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self).convert =
                    Some(tanzim_value::ValueType::Int);
                self
            }

            /// After validation succeeds, cast the value to a float.
            pub fn to_float(mut self) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self).convert =
                    Some(tanzim_value::ValueType::Float);
                self
            }

            /// After validation succeeds, cast the value to a boolean.
            pub fn to_bool(mut self) -> Self {
                <$ty as $crate::Validator>::meta_mut(&mut self).convert =
                    Some(tanzim_value::ValueType::Bool);
                self
            }
        }
    };
}

/// Cast a validated [`Value`] to `target`, reusing the same lenient coercions the leaf validators
/// use. An impossible cast is a [`ErrorKind::NotConvertible`] error.
fn cast(value: &mut Value, target: ValueType) -> Result<(), Error> {
    if matches!(value, Value::Null) {
        return Ok(());
    }
    if value.type_name() == target {
        return Ok(());
    }
    let converted = match target {
        ValueType::String => Value::String(match value {
            Value::Bool(inner) => inner.to_string(),
            Value::Int(inner) => inner.to_string(),
            Value::Float(inner) => inner.to_string(),
            Value::String(inner) => std::mem::take(inner),
            _ => {
                return Err(Error::new(ErrorKind::NotConvertible {
                    target,
                    found: value.type_name(),
                }));
            }
        }),
        ValueType::Int => match value {
            Value::Int(inner) => Value::Int(*inner),
            Value::Bool(inner) => Value::Int(*inner as isize),
            Value::Float(inner) if inner.fract() == 0.0 => Value::Int(*inner as isize),
            Value::String(inner) if inner.parse::<isize>().is_ok() => {
                Value::Int(inner.parse::<isize>().unwrap())
            }
            _ => {
                return Err(Error::new(ErrorKind::NotConvertible {
                    target,
                    found: value.type_name(),
                }));
            }
        },
        ValueType::Float => match value {
            Value::Float(inner) => Value::Float(*inner),
            Value::Int(inner) => Value::Float(*inner as f64),
            Value::String(inner) if inner.parse::<f64>().is_ok() => {
                Value::Float(inner.parse::<f64>().unwrap())
            }
            _ => {
                return Err(Error::new(ErrorKind::NotConvertible {
                    target,
                    found: value.type_name(),
                }));
            }
        },
        ValueType::Bool => match value {
            Value::Bool(inner) => Value::Bool(*inner),
            Value::String(inner) if inner.eq_ignore_ascii_case("true") => Value::Bool(true),
            Value::String(inner) if inner.eq_ignore_ascii_case("false") => Value::Bool(false),
            _ => {
                return Err(Error::new(ErrorKind::NotConvertible {
                    target,
                    found: value.type_name(),
                }));
            }
        },
        ValueType::List | ValueType::Map | ValueType::Null => {
            return Err(Error::new(ErrorKind::NotConvertible {
                target,
                found: value.type_name(),
            }));
        }
    };
    *value = converted;
    Ok(())
}

/// Validate a whole node, seeding the root [`Location`] into any error.
///
/// ```
/// use tanzim_validate::{validate, Integer};
/// use tanzim_value::{LocatedValue, Location, Value};
///
/// let mut node = LocatedValue::new(
///     Value::String("42".into()),
///     Location::at("file", "config.toml", Some(1), Some(1), None),
/// );
/// validate(&Integer::new().range(0, 100), &mut node).unwrap();
/// assert_eq!(node.value().as_int(), Some(42)); // coerced from string
/// ```
pub fn validate(validator: &dyn Validator, value: &mut LocatedValue) -> Result<(), Error> {
    match validator.validate(value.value_mut()) {
        Ok(()) => Ok(()),
        Err(error) => Err(error.with_location(value.location())),
    }
}
