use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`string` feature) Accepts a string, with optional length bounds and (with the `regex` feature) a
/// pattern. No coercion: non-string values are rejected.
#[derive(Debug, Clone, Default)]
pub struct Str {
    meta: Meta,
    min_chars: Option<usize>,
    max_chars: Option<usize>,
    #[cfg(feature = "regex")]
    pattern: Option<regex::Regex>,
}

impl Str {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn min_chars(mut self, min: usize) -> Self {
        self.min_chars = Some(min);
        self
    }

    pub fn max_chars(mut self, max: usize) -> Self {
        self.max_chars = Some(max);
        self
    }

    /// Require the string to match `pattern`.
    ///
    /// Returns `Err` with the compiler message if `pattern` is not a valid regular
    /// expression, so the caller must `?` or unwrap it.
    #[cfg(feature = "regex")]
    pub fn regex(mut self, pattern: impl Into<String>) -> Result<Self, String> {
        let pattern = pattern.into();
        match regex::Regex::new(&pattern) {
            Ok(compiled) => {
                self.pattern = Some(compiled);
                Ok(self)
            }
            Err(error) => Err(error.to_string()),
        }
    }
}

crate::impl_meta_methods!(Str);

impl Validator for Str {
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

        let length = text.chars().count();
        if let Some(min) = self.min_chars
            && length < min
        {
            return Err(Error::new(ErrorKind::TooShort { len: length, min }));
        }
        if let Some(max) = self.max_chars
            && length > max
        {
            return Err(Error::new(ErrorKind::TooLong { len: length, max }));
        }

        #[cfg(feature = "regex")]
        if let Some(pattern) = &self.pattern
            && !pattern.is_match(text)
        {
            return Err(Error::new(ErrorKind::PatternMismatch {
                pattern: pattern.as_str().to_string(),
            }));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_string() {
        let mut value = Value::String("hi".into());
        assert!(Str::new().validate(&mut value).is_ok());
    }

    #[test]
    fn rejects_non_string() {
        let mut value = Value::Int(1);
        let error = Str::new().validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Type { .. }));
    }

    #[test]
    fn enforces_min_chars() {
        let mut value = Value::String("".into());
        let error = Str::new().min_chars(1).validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::TooShort { .. }));
    }

    #[test]
    fn enforces_max_chars() {
        let mut value = Value::String("toolong".into());
        let error = Str::new().max_chars(3).validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::TooLong { .. }));
    }

    #[cfg(feature = "regex")]
    #[test]
    fn regex_matches_and_rejects() {
        let validator = Str::new().regex("^[a-z]+$").unwrap();
        let mut ok = Value::String("abc".into());
        assert!(validator.validate(&mut ok).is_ok());
        let mut bad = Value::String("abc1".into());
        let error = validator.validate(&mut bad).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::PatternMismatch { .. }));
    }
}
