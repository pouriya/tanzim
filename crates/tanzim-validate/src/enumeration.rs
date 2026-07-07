use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::Value;

/// (`enumeration` feature) Accepts a value drawn from a fixed allow-list. The allowed values may be of any type,
/// and are compared by equality (no coercion).
#[derive(Debug, Clone, Default)]
pub struct Enum {
    meta: Meta,
    allowed: Vec<Value>,
    case_insensitive: bool,
}

impl Enum {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// Build from the allowed values, e.g. `Enum::new([Value::Int(1), Value::Int(2)])`.
    pub fn new(values: impl IntoIterator<Item = Value>) -> Self {
        let mut allowed = Vec::new();
        for value in values {
            allowed.push(value);
        }
        Self {
            meta: Meta::default(),
            allowed,
            case_insensitive: false,
        }
    }

    /// Compare string values ignoring ASCII case (no effect on other types).
    pub fn case_insensitive(mut self) -> Self {
        self.case_insensitive = true;
        self
    }
}

crate::impl_meta_methods!(Enum);

impl Validator for Enum {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        for candidate in &self.allowed {
            let matches = match (candidate, &*value) {
                (Value::String(allowed), Value::String(actual)) if self.case_insensitive => {
                    allowed.eq_ignore_ascii_case(actual)
                }
                (allowed, actual) => allowed == actual,
            };
            if matches {
                return Ok(());
            }
        }

        Err(Error::new(ErrorKind::NotAllowed {
            value: value.to_string(),
        }))
    }
}
