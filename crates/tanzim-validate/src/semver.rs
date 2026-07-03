use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`semver` feature) Accepts a semantic version string such as `1.4.2` or `2.0.0-rc.1`.
#[derive(Debug, Clone, Default)]
pub struct Semver {
    meta: Meta,
}

impl Semver {
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

impl Validator for Semver {
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
        match text.parse::<semver::Version>() {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Format {
                expected: "semantic version",
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_rejects() {
        assert!(
            Semver::new()
                .validate(&mut Value::String("1.2.3".into()))
                .is_ok()
        );
        assert!(
            Semver::new()
                .validate(&mut Value::String("1.2".into()))
                .is_err()
        );
    }
}
