use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::Value;

/// A sign constraint shared by [`Number`], [`crate::Integer`], and [`crate::Float`].
#[derive(Debug, Clone, Copy)]
pub(crate) enum Sign {
    Positive,
    NonNegative,
    Negative,
    NonPositive,
}

/// Check a numeric value against an optional sign constraint.
pub(crate) fn check_sign(sign: Option<Sign>, value: f64) -> Result<(), Error> {
    let (ok, expected) = match sign {
        Some(Sign::Positive) => (value > 0.0, "positive number"),
        Some(Sign::NonNegative) => (value >= 0.0, "non-negative number"),
        Some(Sign::Negative) => (value < 0.0, "negative number"),
        Some(Sign::NonPositive) => (value <= 0.0, "non-positive number"),
        None => (true, ""),
    };
    if ok {
        Ok(())
    } else {
        Err(Error::new(ErrorKind::Format { expected }))
    }
}

/// Accepts either an integer or a float, **without converting** between them.
///
/// Use this when a value may legitimately be whole or fractional and you want to keep its
/// original type. Optional inclusive bounds and sign constraints compare the value
/// numerically but never rewrite it.
#[derive(Debug, Clone, Default)]
pub struct Number {
    min: Option<f64>,
    max: Option<f64>,
    sign: Option<Sign>,
}

impl Number {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    pub fn range(mut self, start: f64, end: f64) -> Self {
        self.min = Some(start);
        self.max = Some(end);
        self
    }

    /// Require the value to be strictly greater than zero.
    pub fn positive(mut self) -> Self {
        self.sign = Some(Sign::Positive);
        self
    }

    /// Require the value to be greater than or equal to zero.
    pub fn non_negative(mut self) -> Self {
        self.sign = Some(Sign::NonNegative);
        self
    }

    /// Require the value to be strictly less than zero.
    pub fn negative(mut self) -> Self {
        self.sign = Some(Sign::Negative);
        self
    }

    /// Require the value to be less than or equal to zero.
    pub fn non_positive(mut self) -> Self {
        self.sign = Some(Sign::NonPositive);
        self
    }
}

impl Validator for Number {
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
        let number = match value {
            Value::Int(number) => *number as f64,
            Value::Float(number) => *number,
            _ => return Err(Error::new(ErrorKind::Format { expected: "number" })),
        };

        if let Some(min) = self.min
            && number < min
        {
            return Err(Error::new(ErrorKind::BelowMin {
                value: number.to_string(),
                min: min.to_string(),
            }));
        }
        if let Some(max) = self.max
            && number > max
        {
            return Err(Error::new(ErrorKind::AboveMax {
                value: number.to_string(),
                max: max.to_string(),
            }));
        }

        check_sign(self.sign, number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_int_and_float_without_converting() {
        let mut int_value = Value::Int(3);
        Number::new().validate(&mut int_value).unwrap();
        assert_eq!(int_value, Value::Int(3));

        let mut float_value = Value::Float(3.5);
        Number::new().validate(&mut float_value).unwrap();
        assert_eq!(float_value, Value::Float(3.5));
    }

    #[test]
    fn rejects_non_numbers() {
        assert!(
            Number::new()
                .validate(&mut Value::String("3".into()))
                .is_err()
        );
    }

    #[test]
    fn bounds_and_sign() {
        assert!(
            Number::new()
                .range(0.0, 10.0)
                .validate(&mut Value::Int(5))
                .is_ok()
        );
        assert!(
            Number::new()
                .range(0.0, 10.0)
                .validate(&mut Value::Float(11.0))
                .is_err()
        );
        assert!(
            Number::new()
                .positive()
                .validate(&mut Value::Float(0.0))
                .is_err()
        );
        assert!(
            Number::new()
                .non_negative()
                .validate(&mut Value::Int(0))
                .is_ok()
        );
    }
}
