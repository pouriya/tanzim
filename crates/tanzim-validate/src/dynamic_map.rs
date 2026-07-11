use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`dynamic_map` feature) Accepts a map with arbitrary keys and uniform values.
///
/// Optional entry-count bounds and an optional validator applied to every value.
/// Coercion: an empty list becomes an empty map (the list counterpart of an empty
/// collection). A non-empty list or any other type is rejected.
#[derive(Default)]
pub struct DynamicMap {
    meta: Meta,
    min_len: Option<usize>,
    max_len: Option<usize>,
    values: Option<Box<dyn Validator + Send + Sync>>,
}

impl DynamicMap {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// A new, unconfigured `DynamicMap` validator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Require at least `min` entries.
    pub fn min_len(mut self, min: usize) -> Self {
        self.min_len = Some(min);
        self
    }

    /// Require at most `max` entries.
    pub fn max_len(mut self, max: usize) -> Self {
        self.max_len = Some(max);
        self
    }

    /// Validate every value with `validator`.
    pub fn values(mut self, validator: impl Into<Box<dyn Validator + Send + Sync>>) -> Self {
        self.values = Some(validator.into());
        self
    }
}

crate::impl_meta_methods!(DynamicMap);

impl Validator for DynamicMap {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        match value {
            Value::Map(_) => {}
            Value::List(list) if list.is_empty() => *value = Value::new_map(),
            _ => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::Map,
                    found: value.type_name(),
                }));
            }
        }

        let map = match value.map_mut() {
            Some(map) => map,
            None => unreachable!("value coerced to a map above"),
        };

        let length = map.len();
        if let Some(min) = self.min_len
            && length < min
        {
            return Err(Error::new(ErrorKind::TooShort { len: length, min }));
        }
        if let Some(max) = self.max_len
            && length > max
        {
            return Err(Error::new(ErrorKind::TooLong { len: length, max }));
        }

        if let Some(validator) = &self.values {
            for (key, entry) in map.entries_mut() {
                match validator.validate(entry.value_mut()) {
                    Ok(()) => {}
                    Err(error) => return Err(error.under_key(key, entry.location())),
                }
            }
        }

        Ok(())
    }
}
