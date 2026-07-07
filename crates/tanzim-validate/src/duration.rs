use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`duration` feature) Accepts a human duration string (e.g. `"30s"`, `"5m"`, `"1h30m"`) and coerces it to an
/// integer number of seconds (or milliseconds with [`Duration::millis`]).
#[derive(Debug, Clone, Default)]
pub struct Duration {
    meta: Meta,
    millis: bool,
}

impl Duration {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// Coerce to milliseconds instead of seconds.
    pub fn millis(mut self) -> Self {
        self.millis = true;
        self
    }
}

crate::impl_meta_methods!(Duration);

impl Validator for Duration {
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
