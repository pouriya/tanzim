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
    first: Box<dyn Validator>,
    second: Box<dyn Validator>,
}

impl Either {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    pub fn new(
        first: impl Into<Box<dyn Validator>>,
        second: impl Into<Box<dyn Validator>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bool, Integer};

    #[test]
    fn accepts_when_either_matches() {
        let validator = Either::new(Integer::new(), Bool::new());
        assert!(validator.validate(&mut Value::Int(3)).is_ok());
        assert!(validator.validate(&mut Value::Bool(true)).is_ok());
    }

    #[test]
    fn commits_coercion_of_winning_branch() {
        let validator = Either::new(Integer::new(), Bool::new());
        let mut value = Value::String("5".into());
        validator.validate(&mut value).unwrap();
        assert_eq!(value, Value::Int(5));
    }

    #[test]
    fn original_value_preserved_for_second_attempt() {
        // Float would reject a bool and could not coerce it; the second branch must still
        // see the original Bool, not whatever the first branch left behind.
        let validator = Either::new(Integer::new(), Bool::new());
        let mut value = Value::Bool(true);
        validator.validate(&mut value).unwrap();
        assert_eq!(value, Value::Bool(true));
    }

    #[test]
    fn combines_errors_when_both_fail() {
        let validator = Either::new(Bool::new(), Integer::new());
        let mut value = Value::String("nope".into());
        let error = validator.validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Either { .. }));
        // value untouched on total failure
        assert_eq!(value, Value::String("nope".into()));
    }
}
