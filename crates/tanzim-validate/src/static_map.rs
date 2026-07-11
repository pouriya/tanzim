use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

struct Field {
    key: String,
    required: bool,
    validator: Option<Box<dyn Validator + Send + Sync>>,
}

/// (`static_map` feature) Accepts a map with a known set of keys.
///
/// Each declared key is either required or optional, and may carry a validator for its
/// value. By default keys not declared in the schema are rejected; call
/// [`StaticMap::allow_unknown`] to permit them.
///
/// ```
/// # #[cfg(all(feature = "static_map", feature = "non_empty", feature = "integer"))]
/// # {
/// use tanzim_validate::{Integer, NonEmpty, StaticMap, validate};
/// use tanzim_value::{LocatedValue, Location, Value};
///
/// let schema = StaticMap::new()
///     .required("name", NonEmpty::new())
///     .optional("port", Integer::new().range(1, 65535));
///
/// let mut map = Value::new_map();
/// map.map_mut().unwrap().insert(
///     "name".into(),
///     LocatedValue::new(Value::String("db".into()), Location::at("cfg", "", None, None, None)),
/// );
/// map.map_mut().unwrap().insert(
///     "port".into(),
///     LocatedValue::new(Value::String("5432".into()), Location::at("cfg", "", None, None, None)),
/// );
/// let mut node = LocatedValue::new(map, Location::at("cfg", "", None, None, None));
///
/// validate(&schema, &mut node).unwrap();
/// assert_eq!(
///     node.value_mut().map_mut().unwrap().get("port").unwrap().value().as_int(),
///     Some(5432), // coerced from string
/// );
/// # }
/// ```
#[derive(Default)]
pub struct StaticMap {
    meta: Meta,
    fields: Vec<Field>,
    deny_unknown: bool,
}

impl StaticMap {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// A new `StaticMap` validator with no declared keys, denying unknown keys by default.
    pub fn new() -> Self {
        Self {
            meta: Meta::default(),
            fields: Vec::new(),
            deny_unknown: true,
        }
    }

    /// Declare a required key whose value is validated by `validator`.
    pub fn required(
        mut self,
        key: impl Into<String>,
        validator: impl Into<Box<dyn Validator + Send + Sync>>,
    ) -> Self {
        self.fields.push(Field {
            key: key.into(),
            required: true,
            validator: Some(validator.into()),
        });
        self
    }

    /// Declare an optional key whose value, when present, is validated by `validator`.
    pub fn optional(
        mut self,
        key: impl Into<String>,
        validator: impl Into<Box<dyn Validator + Send + Sync>>,
    ) -> Self {
        self.fields.push(Field {
            key: key.into(),
            required: false,
            validator: Some(validator.into()),
        });
        self
    }

    /// Declare a required key whose value is accepted without validation.
    pub fn required_any(mut self, key: impl Into<String>) -> Self {
        self.fields.push(Field {
            key: key.into(),
            required: true,
            validator: None,
        });
        self
    }

    /// Declare an optional key whose value is accepted without validation.
    pub fn optional_any(mut self, key: impl Into<String>) -> Self {
        self.fields.push(Field {
            key: key.into(),
            required: false,
            validator: None,
        });
        self
    }

    /// Reject keys not declared in the schema (the default).
    pub fn deny_unknown(mut self) -> Self {
        self.deny_unknown = true;
        self
    }

    /// Accept keys not declared in the schema, leaving their values untouched.
    pub fn allow_unknown(mut self) -> Self {
        self.deny_unknown = false;
        self
    }
}

crate::impl_meta_methods!(StaticMap);

impl Validator for StaticMap {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let map = match value.map_mut() {
            Some(map) => map,
            None => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::Map,
                    found: value.type_name(),
                }));
            }
        };

        for field in &self.fields {
            if field.required && !map.contains_key(&field.key) {
                return Err(Error::new(ErrorKind::MissingKey {
                    key: field.key.clone(),
                }));
            }
        }

        for field in &self.fields {
            if let Some(validator) = &field.validator
                && let Some(entry) = map.get_mut(&field.key)
            {
                match validator.validate(entry.value_mut()) {
                    Ok(()) => {}
                    Err(error) => return Err(error.under_key(&field.key, entry.location())),
                }
            }
        }

        if self.deny_unknown {
            for (key, entry) in map.entries() {
                let mut declared = false;
                for field in &self.fields {
                    if &field.key == key {
                        declared = true;
                        break;
                    }
                }
                if !declared {
                    return Err(Error::new(ErrorKind::UnknownKey { key: key.clone() })
                        .with_location(entry.location()));
                }
            }
        }

        Ok(())
    }
}
