use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

/// (`duration` feature) Accepts a human duration string (e.g. `"30s"`, `"5m"`, `"1h30m"`) and coerces it to an
/// integer number of seconds (or milliseconds with [`Duration::millis`]).
#[derive(Debug, Clone, Default)]
pub struct Duration {
    millis: bool,
}

impl Duration {
    pub fn new() -> Self {
        Self::default()
    }

    /// Coerce to milliseconds instead of seconds.
    pub fn millis(mut self) -> Self {
        self.millis = true;
        self
    }
}

impl Validator for Duration {
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

        let parsed = match humantime::parse_duration(text) {
            Ok(parsed) => parsed,
            Err(_) => {
                return Err(Error::new(ErrorKind::Format {
                    expected: "duration",
                }));
            }
        };

        let amount = if self.millis {
            parsed.as_millis()
        } else {
            parsed.as_secs() as u128
        };
        let coerced = match isize::try_from(amount) {
            Ok(coerced) => coerced,
            Err(_) => {
                return Err(Error::new(ErrorKind::NotConvertible {
                    target: ValueType::Int,
                    found: ValueType::String,
                }));
            }
        };

        *value = Value::Int(coerced);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coerces_to_seconds() {
        let mut value = Value::String("1h30m".into());
        Duration::new().validate(&mut value).unwrap();
        assert_eq!(value, Value::Int(5400));
    }

    #[test]
    fn coerces_to_millis() {
        let mut value = Value::String("250ms".into());
        Duration::new().millis().validate(&mut value).unwrap();
        assert_eq!(value, Value::Int(250));
    }

    #[test]
    fn rejects_garbage() {
        let mut value = Value::String("soon".into());
        assert!(Duration::new().validate(&mut value).is_err());
    }
}
