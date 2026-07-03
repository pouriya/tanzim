use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`bytesize` feature) Accepts a human byte-size string (e.g. `"10MB"`, `"1GiB"`) and coerces it to an integer
/// number of bytes.
#[derive(Debug, Clone, Default)]
pub struct ByteSize {
    meta: Meta,
}

impl ByteSize {
    pub fn new() -> Self {
        Self {
            meta: Meta::default(),
        }
    }

    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

impl Validator for ByteSize {
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

        let parsed = match text.parse::<bytesize::ByteSize>() {
            Ok(parsed) => parsed,
            Err(_) => {
                return Err(Error::new(ErrorKind::Format {
                    expected: "byte size",
                }));
            }
        };

        let coerced = match isize::try_from(parsed.as_u64()) {
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
    fn coerces_to_bytes() {
        let mut value = Value::String("10 KB".into());
        ByteSize::new().validate(&mut value).unwrap();
        assert_eq!(value, Value::Int(10_000));
    }

    #[test]
    fn rejects_garbage() {
        let mut value = Value::String("lots".into());
        assert!(ByteSize::new().validate(&mut value).is_err());
    }
}
