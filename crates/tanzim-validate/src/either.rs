use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::Value;

/// (`either` feature) Accepts the value if **either** of two validators accepts it.
///
/// The first validator is tried first; if it succeeds (possibly coercing the value), its
/// result is kept. Otherwise the second validator is tried against the *original* value, so
/// a partial coercion from the first attempt is never observed. If both fail, the two
/// errors are combined into a single [`ErrorKind::Either`] that reports what each expected.
pub struct Either {
    meta: Meta,
    first: Box<dyn Validator + Send + Sync>,
    second: Box<dyn Validator + Send + Sync>,
}

impl Either {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    pub fn new(
        first: impl Into<Box<dyn Validator + Send + Sync>>,
        second: impl Into<Box<dyn Validator + Send + Sync>>,
    ) -> Self {
        Self {
            meta: Meta::default(),
            first: first.into(),
            second: second.into(),
        }
    }
}

crate::impl_meta_methods!(Either);

impl Validator for Either {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        // Validate on a copy so a failing branch never leaves a half-coerced value behind;
        // only the branch that succeeds is committed back.
        let mut candidate = value.clone();
        let first_error = match self.first.validate(&mut candidate) {
            Ok(()) => {
                *value = candidate;
                return Ok(());
            }
            Err(error) => error,
        };

        let mut candidate = value.clone();
        let second_error = match self.second.validate(&mut candidate) {
            Ok(()) => {
                *value = candidate;
                return Ok(());
            }
            Err(error) => error,
        };

        Err(Error::new(ErrorKind::Either {
            first: Box::new(first_error),
            second: Box::new(second_error),
        }))
    }
}
