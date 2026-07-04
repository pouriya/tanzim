use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
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

/// (`datetime` feature) Accepts an RFC 3339 timestamp such as `2024-01-02T15:04:05Z`.
#[derive(Debug, Clone, Default)]
pub struct DateTime {
    meta: Meta,
}

impl DateTime {
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

crate::impl_meta_methods!(DateTime);

impl Validator for DateTime {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_str(value)?;
        match text.parse::<jiff::Timestamp>() {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Format {
                expected: "RFC 3339 datetime",
            })),
        }
    }
}

/// (`datetime` feature) Accepts a calendar date such as `2024-01-02`.
#[derive(Debug, Clone, Default)]
pub struct Date {
    meta: Meta,
}

impl Date {
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

crate::impl_meta_methods!(Date);

impl Validator for Date {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_str(value)?;
        match text.parse::<jiff::civil::Date>() {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Format { expected: "date" })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_accepts_rfc3339() {
        assert!(
            DateTime::new()
                .validate(&mut Value::String("2024-01-02T15:04:05Z".into()))
                .is_ok()
        );
        assert!(
            DateTime::new()
                .validate(&mut Value::String("yesterday".into()))
                .is_err()
        );
    }

    #[test]
    fn date_accepts_calendar_date() {
        assert!(
            Date::new()
                .validate(&mut Value::String("2024-01-02".into()))
                .is_ok()
        );
        assert!(
            Date::new()
                .validate(&mut Value::String("2024-13-99".into()))
                .is_err()
        );
    }
}
