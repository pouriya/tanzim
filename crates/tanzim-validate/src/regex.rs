use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

/// Accepts a string that is itself a valid regular expression.
#[derive(Debug, Clone, Default)]
pub struct RegexPattern;

impl RegexPattern {
    pub fn new() -> Self {
        Self
    }
}

impl Validator for RegexPattern {
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
        match regex::Regex::new(text) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Format {
                expected: "regular expression",
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_and_rejects_invalid() {
        assert!(
            RegexPattern::new()
                .validate(&mut Value::String("^a.*$".into()))
                .is_ok()
        );
        assert!(
            RegexPattern::new()
                .validate(&mut Value::String("(".into()))
                .is_err()
        );
    }
}
