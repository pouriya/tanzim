use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`uuid` feature) Accepts a UUID string in the canonical hyphenated form.
#[derive(Debug, Clone, Default)]
pub struct Uuid {
    meta: Meta,
}

impl Uuid {
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

crate::impl_meta_methods!(Uuid);

impl Validator for Uuid {
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
        match uuid::Uuid::parse_str(text) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Format { expected: "uuid" })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_rejects() {
        assert!(
            Uuid::new()
                .validate(&mut Value::String(
                    "67e55044-10b1-426f-9247-bb680e5fe0c8".into()
                ))
                .is_ok()
        );
        assert!(
            Uuid::new()
                .validate(&mut Value::String("nope".into()))
                .is_err()
        );
    }
}
