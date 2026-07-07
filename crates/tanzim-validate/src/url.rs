use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`url` feature) Accepts a URL, optionally restricting the scheme and requiring a host.
#[derive(Debug, Clone, Default)]
pub struct Url {
    meta: Meta,
    schemes: Vec<String>,
    require_host: bool,
}

impl Url {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

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

crate::impl_meta_methods!(Url);

impl Validator for Url {
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
