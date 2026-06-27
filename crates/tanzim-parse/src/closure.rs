//! Custom parser backed by a closure.
//!
//! # Example
//!
//! ```
//! use tanzim_parse::{closure::Closure, Deserialize};
//! use tanzim_value::{LocatedValue, Location, Value};
//!
//! let parser = Closure::new(
//!     "upper",
//!     "txt",
//!     Box::new(|source, resource, bytes| {
//!         Ok(LocatedValue {
//!             value: Value::String(String::from_utf8_lossy(bytes).to_uppercase()),
//!             location: Location::at(source, resource, None, None, None),
//!         })
//!     }),
//! );
//! let value = parser.parse("file", "test.txt", b"hello").unwrap();
//! assert_eq!(value.value.as_string().unwrap(), "HELLO");
//! ```

use crate::Deserialize;
use tanzim_value::{Error, LocatedValue};

pub type BoxedParseFn = Box<dyn Fn(&str, &str, &[u8]) -> Result<LocatedValue, Error>>;
pub type BoxedValidatorFn = Box<dyn Fn(&[u8]) -> Option<bool>>;

pub struct Closure {
    name: String,
    parser: BoxedParseFn,
    validator: BoxedValidatorFn,
    supported_format_list: Vec<String>,
}

impl Closure {
    pub fn new<N: AsRef<str>, F: AsRef<str>>(
        name: N,
        supported_format: F,
        parser: BoxedParseFn,
    ) -> Self {
        Self {
            name: name.as_ref().to_string(),
            parser,
            validator: Box::new(|_| None),
            supported_format_list: vec![supported_format.as_ref().to_string()],
        }
    }

    pub fn with_validator(mut self, validator: BoxedValidatorFn) -> Self {
        self.validator = validator;
        self
    }

    pub fn with_format_list<N: AsRef<str>>(mut self, format_list: &[N]) -> Self {
        let mut formats = Vec::new();
        for format in format_list {
            formats.push(format.as_ref().to_string());
        }
        self.supported_format_list = formats;
        self
    }
}

impl Deserialize for Closure {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn supported_format_list(&self) -> Vec<String> {
        self.supported_format_list.clone()
    }

    fn parse(&self, source: &str, resource: &str, bytes: &[u8]) -> Result<LocatedValue, Error> {
        (self.parser)(source, resource, bytes)
    }

    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
        (self.validator)(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tanzim_value::{Location, Value};

    #[test]
    fn closure_parser_delegates_to_function() {
        let parser = Closure::new(
            "upper",
            "txt",
            Box::new(|source, resource, bytes| {
                Ok(LocatedValue {
                    value: Value::String(String::from_utf8_lossy(bytes).to_uppercase()),
                    location: Location::at(source, resource, None, None, None),
                })
            }),
        )
        .with_validator(Box::new(|bytes| Some(!bytes.is_empty())));
        let parsed = parser.parse("file", "test.txt", b"hello").unwrap();
        assert_eq!(parsed.value.as_string().unwrap(), "HELLO");
        assert_eq!(parser.is_format_supported(b"x"), Some(true));
        assert_eq!(parser.is_format_supported(b""), Some(false));
    }
}
