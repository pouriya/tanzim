use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

/// (`boolean` feature) Accepts only a boolean value. No coercion, no options.
#[derive(Debug, Clone, Default)]
pub struct Bool;

impl Bool {
    pub fn new() -> Self {
        Self
    }
}

impl Validator for Bool {
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
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
