use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use base64::Engine;
use tanzim_value::{Value, ValueType};

/// Borrow the inner string, or produce a `Type` error expecting a string.
fn as_str(value: &mut Value) -> Result<&str, Error> {
    match value {
        Value::String(text) => Ok(text),
        other => Err(Error::new(ErrorKind::Type {
            expected: ValueType::String,
            found: other.type_name(),
        })),
    }
}

/// (`encoding` feature) Accepts a standard (RFC 4648) base64-encoded string.
#[derive(Debug, Clone, Default)]
pub struct Base64 {
    meta: Meta,
}

impl Base64 {
    /// A new, unconfigured `Base64` validator.
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

crate::impl_meta_methods!(Base64);

impl Validator for Base64 {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_str(value)?;
        match base64::engine::general_purpose::STANDARD.decode(text) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Format { expected: "base64" })),
        }
    }
}

/// (`encoding` feature) Accepts a hexadecimal string (even number of `0-9a-fA-F` digits).
#[derive(Debug, Clone, Default)]
pub struct Hex {
    meta: Meta,
}

impl Hex {
    /// A new, unconfigured `Hex` validator.
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

crate::impl_meta_methods!(Hex);

impl Validator for Hex {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_str(value)?;
        let bytes = text.as_bytes();
        if bytes.is_empty() || bytes.len() % 2 != 0 {
            return Err(Error::new(ErrorKind::Format {
                expected: "hexadecimal",
            }));
        }
        for &byte in bytes {
            if !byte.is_ascii_hexdigit() {
                return Err(Error::new(ErrorKind::Format {
                    expected: "hexadecimal",
                }));
            }
        }
        Ok(())
    }
}
