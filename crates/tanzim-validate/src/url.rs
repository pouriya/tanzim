use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

/// Accepts a URL, optionally restricting the scheme and requiring a host.
#[derive(Debug, Clone, Default)]
pub struct Url {
    schemes: Vec<String>,
    require_host: bool,
}

impl Url {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict to the given schemes, e.g. `Url::new().schemes(["http", "https"])`.
    pub fn schemes(mut self, schemes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for scheme in schemes {
            self.schemes.push(scheme.into());
        }
        self
    }

    /// Require the URL to have a host component.
    pub fn require_host(mut self) -> Self {
        self.require_host = true;
        self
    }
}

impl Validator for Url {
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
        let text = match value {
            Value::String(text) => text,
            other => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::String,
                    found: other.type_name(),
                }));
            }
        };

        let parsed = match url::Url::parse(text) {
            Ok(parsed) => parsed,
            Err(_) => return Err(Error::new(ErrorKind::Format { expected: "url" })),
        };

        if !self.schemes.is_empty() {
            let mut allowed = false;
            for scheme in &self.schemes {
                if scheme.eq_ignore_ascii_case(parsed.scheme()) {
                    allowed = true;
                    break;
                }
            }
            if !allowed {
                return Err(Error::new(ErrorKind::Format {
                    expected: "url with an allowed scheme",
                }));
            }
        }

        if self.require_host && parsed.host().is_none() {
            return Err(Error::new(ErrorKind::Format {
                expected: "url with a host",
            }));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(text: &str) -> Value {
        Value::String(text.to_string())
    }

    #[test]
    fn accepts_url() {
        assert!(
            Url::new()
                .validate(&mut string("https://example.com/x"))
                .is_ok()
        );
        assert!(Url::new().validate(&mut string("not a url")).is_err());
    }

    #[test]
    fn restricts_scheme_and_host() {
        let validator = Url::new().schemes(["https"]).require_host();
        assert!(
            validator
                .validate(&mut string("https://example.com"))
                .is_ok()
        );
        assert!(
            validator
                .validate(&mut string("http://example.com"))
                .is_err()
        );
        assert!(validator.validate(&mut string("mailto:a@b.com")).is_err());
    }
}
