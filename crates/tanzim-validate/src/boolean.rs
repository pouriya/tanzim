use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`boolean` feature) Accepts only a boolean value. No coercion, no options.
#[derive(Debug, Clone, Default)]
pub struct Bool {
    meta: Meta,
}

impl Bool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

impl Validator for Bool {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        if value.is_bool() {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Type {
                expected: ValueType::Bool,
                found: value.type_name(),
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_bool() {
        let mut value = Value::Bool(true);
        assert!(Bool::new().validate(&mut value).is_ok());
    }

    #[test]
    fn rejects_non_bool() {
        let mut value = Value::Int(1);
        let error = Bool::new().validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Type { .. }));
    }
}
