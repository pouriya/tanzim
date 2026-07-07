use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`list` feature) Accepts a list, with optional length bounds, a uniqueness check, and an optional
/// per-item validator.
///
/// Coercion: an empty map becomes an empty list (matching formats that render an empty
/// collection as `{}`). A non-empty map or any other type is rejected.
#[derive(Default)]
pub struct List {
    meta: Meta,
    min_len: Option<usize>,
    max_len: Option<usize>,
    unique: bool,
    items: Option<Box<dyn Validator>>,
}

impl List {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn min_len(mut self, min: usize) -> Self {
        self.min_len = Some(min);
        self
    }

    pub fn max_len(mut self, max: usize) -> Self {
        self.max_len = Some(max);
        self
    }

    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Validate every item with `validator`.
    pub fn items(mut self, validator: impl Into<Box<dyn Validator>>) -> Self {
        self.items = Some(validator.into());
        self
    }
}

crate::impl_meta_methods!(List);

impl Validator for List {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        match value {
            Value::List(_) => {}
            Value::Map(map) if map.is_empty() => *value = Value::new_list(),
            _ => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::List,
                    found: value.type_name(),
                }));
            }
        }

        let items = match value.list_mut() {
            Some(items) => items,
            None => unreachable!("value coerced to a list above"),
        };

        let length = items.len();
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

        if let Some(validator) = &self.items {
            for (index, item) in items.iter_mut().enumerate() {
                match validator.validate(item.value_mut()) {
                    Ok(()) => {}
                    Err(error) => return Err(error.under_index(index, item.location())),
                }
            }
        }

        if self.unique {
            let mut seen: Vec<&Value> = Vec::new();
            for (index, item) in items.iter().enumerate() {
                for previous in &seen {
                    if **previous == *item.value() {
                        return Err(Error::new(ErrorKind::Duplicate { index })
                            .with_location(item.location()));
                    }
                }
                seen.push(item.value());
            }
        }

        Ok(())
    }
}
