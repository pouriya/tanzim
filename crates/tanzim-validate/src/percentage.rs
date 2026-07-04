use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`percentage` feature) Accepts a percentage: an integer in `0..=100`, or a float ratio in `0.0..=1.0`.
#[derive(Debug, Clone, Default)]
pub struct Percentage {
    meta: Meta,
}

impl Percentage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

crate::impl_meta_methods!(Percentage);

impl Validator for Percentage {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        match value {
            Value::Int(number) => {
                if (0..=100).contains(number) {
                    Ok(())
                } else {
                    Err(Error::new(ErrorKind::Format {
                        expected: "percentage in 0..=100",
                    }))
                }
            }
            Value::Float(number) => {
                if (0.0..=1.0).contains(number) {
                    Ok(())
                } else {
                    Err(Error::new(ErrorKind::Format {
                        expected: "ratio in 0.0..=1.0",
                    }))
                }
            }
            other => Err(Error::new(ErrorKind::Type {
                expected: ValueType::Float,
                found: other.type_name(),
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_and_ratio() {
        assert!(Percentage::new().validate(&mut Value::Int(50)).is_ok());
        assert!(Percentage::new().validate(&mut Value::Int(150)).is_err());
        assert!(Percentage::new().validate(&mut Value::Float(0.5)).is_ok());
        assert!(Percentage::new().validate(&mut Value::Float(1.5)).is_err());
    }
}
