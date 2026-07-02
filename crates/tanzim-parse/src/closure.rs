//! Custom parser backed by a closure.
//!
//! Use this when a format isn't built-in and you don't want to define a whole type just to
//! implement [`Parse`]. Wrap a closure of the same shape as
//! [`Parse::parse`] — see [`BoxedParseFn`] — and the resulting
//! [`Closure`] *is* a `Parse`, so it plugs straight into the pipeline. Optionally attach a
//! [`BoxedValidatorFn`] with [`Closure::with_validator`] to take part in format auto-detection.
//!
//! For anything with non-trivial state, prefer a real `impl Parse`. Reach for `Closure` for
//! small, stateless, or one-off parsers.
//!
//! # Example
//!
//! ```
//! use tanzim_parse::{closure::Closure, Parse};
//! use tanzim_source::SourceBuilder;
//! use tanzim_value::{LocatedValue, Location, Value};
//!
//! let parser = Closure::new(
//!     "upper",
//!     "txt",
//!     Box::new(|source, bytes| {
//!         Ok(LocatedValue {
//!             value: Value::String(String::from_utf8_lossy(bytes).to_uppercase()),
//!             location: Location::at(source.source(), source.resource(), None, None, None),
//!         })
//!     }),
//! );
//! let source = SourceBuilder::new()
//!     .with_source("file")
//!     .with_resource("test.txt")
//!     .build()
//!     .unwrap();
//! let value = parser.parse(&source, b"hello").unwrap();
//! assert_eq!(value.value.as_string().unwrap(), "HELLO");
//! ```

use crate::{Parse, Source};
use tanzim_value::{Error, LocatedValue};

/// The parse closure driving a [`Closure`] parser — same contract as
/// [`Parse::parse`].
///
/// Called with the [`Source`] declaration and the raw `&[u8]` bytes. Return a [`LocatedValue`]
/// tree (ideally with a [`Location`](tanzim_value::Location) on every node), or an [`Error`] on
/// failure.
pub type BoxedParseFn = Box<dyn Fn(&Source, &[u8]) -> Result<LocatedValue, Error>>;

/// The optional auto-detection probe for a [`Closure`] parser — same contract as
/// [`Parse::is_format_supported`].
///
/// Given the raw bytes, return `Some(true)` if confident, `Some(false)` if definitely not this
/// format, or `None` to abstain. The default (when none is set) abstains with `None`.
pub type BoxedValidatorFn = Box<dyn Fn(&[u8]) -> Option<bool>>;

/// A [`Parse`] implementation whose behaviour is supplied by closures.
///
/// Reach for this instead of a full `impl Parse` when the parser is small, stateless, or a
/// one-off adapter. See the [module docs](self) for a complete example.
pub struct Closure {
    name: String,
    parser: BoxedParseFn,
    validator: BoxedValidatorFn,
    supported_format_list: Vec<String>,
}

impl Closure {
    /// Build a closure-backed parser.
    ///
    /// - `name` — the parser [`name`](crate::Parse::name) used in error messages.
    /// - `supported_format` — the single format extension this parser handles (widen later with
    ///   [`Closure::with_format_list`]).
    /// - `parser` — the closure run by [`parse`](crate::Parse::parse).
    ///
    /// The auto-detection probe defaults to abstaining (`None`); set one with
    /// [`Closure::with_validator`].
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

    /// Attach an auto-detection probe (see [`BoxedValidatorFn`]) used when a payload has no format
    /// hint.
    pub fn with_validator(mut self, validator: BoxedValidatorFn) -> Self {
        self.validator = validator;
        self
    }

    /// Replace the list of format extensions this parser handles (e.g. `["yml", "yaml"]`).
    pub fn with_format_list<N: AsRef<str>>(mut self, format_list: &[N]) -> Self {
        let mut formats = Vec::new();
        for format in format_list {
            formats.push(format.as_ref().to_string());
        }
        self.supported_format_list = formats;
        self
    }
}

impl Parse for Closure {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn supported_format_list(&self) -> Vec<String> {
        self.supported_format_list.clone()
    }

    fn parse(&self, source: &Source, bytes: &[u8]) -> Result<LocatedValue, Error> {
        (self.parser)(source, bytes)
    }

    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
        (self.validator)(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tanzim_source::SourceBuilder;
    use tanzim_value::{Location, Value};

    #[test]
    fn closure_parser_delegates_to_function() {
        let parser = Closure::new(
            "upper",
            "txt",
            Box::new(|source, bytes| {
                Ok(LocatedValue {
                    value: Value::String(String::from_utf8_lossy(bytes).to_uppercase()),
                    location: Location::at(source.source(), source.resource(), None, None, None),
                })
            }),
        )
        .with_validator(Box::new(|bytes| Some(!bytes.is_empty())));
        let source = SourceBuilder::new()
            .with_source("file")
            .with_resource("test.txt")
            .build()
            .unwrap();
        let parsed = parser.parse(&source, b"hello").unwrap();
        assert_eq!(parsed.value.as_string().unwrap(), "HELLO");
        assert_eq!(parser.is_format_supported(b"x"), Some(true));
        assert_eq!(parser.is_format_supported(b""), Some(false));
    }
}
