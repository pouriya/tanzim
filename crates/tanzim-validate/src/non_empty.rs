use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

/// (`non_empty` feature) Accepts a non-blank string (at least one non-whitespace character).
#[derive(Debug, Clone, Default)]
pub struct NonEmpty;

impl NonEmpty {
    pub fn new() -> Self {
        Self
    }
}

impl Validator for NonEmpty {
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank() {
        assert!(
            NonEmpty::new()
                .validate(&mut Value::String("x".into()))
                .is_ok()
        );
        assert!(
            NonEmpty::new()
                .validate(&mut Value::String("   ".into()))
                .is_err()
        );
    }
}
