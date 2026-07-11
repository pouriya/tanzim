use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`non_empty` feature) Accepts a non-blank string (at least one non-whitespace character).
#[derive(Debug, Clone, Default)]
pub struct NonEmpty {
    meta: Meta,
}

impl NonEmpty {
    /// A new, unconfigured `NonEmpty` validator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

crate::impl_meta_methods!(NonEmpty);

impl Validator for NonEmpty {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = match value {
            Value::String(text) => text,
            other => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::String,
                    found: other.type_name(),
                }));
            }
        };
        if text.trim().is_empty() {
            return Err(Error::new(ErrorKind::TooShort {
                len: text.chars().count(),
                min: 1,
            }));
        }
        Ok(())
    }
}
