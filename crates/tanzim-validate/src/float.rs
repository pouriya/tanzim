use crate::error::{Error, ErrorKind};
use crate::number::{Sign, check_sign};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`float` feature) Accepts a float, with optional inclusive bounds and lenient coercion.
///
/// Coercion:
/// - a float stays as-is;
/// - an integer becomes a float (`7` → `7.0`);
/// - a string is parsed as a float (which also accepts integer-looking strings).
#[derive(Debug, Clone, Default)]
pub struct Float {
    meta: Meta,
    min: Option<f64>,
    max: Option<f64>,
    sign: Option<Sign>,
}

impl Float {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

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

impl Validator for Float {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let coerced = match value {
            Value::Float(number) => *number,
            Value::Int(number) => *number as f64,
            Value::String(text) => match text.parse::<f64>() {
                Ok(number) => number,
                Err(_) => {
                    return Err(Error::new(ErrorKind::NotConvertible {
                        target: ValueType::Float,
                        found: ValueType::String,
                    }));
                }
            },
            other => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::Float,
                    found: other.type_name(),
                }));
            }
        };

        if let Some(min) = self.min
            && coerced < min
        {
            return Err(Error::new(ErrorKind::BelowMin {
                value: coerced.to_string(),
                min: min.to_string(),
            }));
        }
        if let Some(max) = self.max
            && coerced > max
        {
            return Err(Error::new(ErrorKind::AboveMax {
                value: coerced.to_string(),
                max: max.to_string(),
            }));
        }

        check_sign(self.sign, coerced)?;

        *value = Value::Float(coerced);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_float() {
        let mut value = Value::Float(1.5);
        assert!(Float::new().validate(&mut value).is_ok());
    }

    #[test]
    fn coerces_integer() {
        let mut value = Value::Int(7);
        Float::new().validate(&mut value).unwrap();
        assert_eq!(value, Value::Float(7.0));
    }

    #[test]
    fn coerces_string() {
        let mut value = Value::String("1.5".into());
        Float::new().validate(&mut value).unwrap();
        assert_eq!(value, Value::Float(1.5));
    }

    #[test]
    fn enforces_range() {
        let mut value = Value::Float(-0.1);
        let error = Float::new()
            .range(0.0, 1.0)
            .validate(&mut value)
            .unwrap_err();
        assert!(matches!(error.kind, ErrorKind::BelowMin { .. }));

        let mut high = Value::Float(2.0);
        let error = Float::new()
            .range(0.0, 1.0)
            .validate(&mut high)
            .unwrap_err();
        assert!(matches!(error.kind, ErrorKind::AboveMax { .. }));
    }

    #[test]
    fn enforces_sign_constraints() {
        let mut zero = Value::Float(0.0);
        assert!(Float::new().positive().validate(&mut zero).is_err());
        let mut negative = Value::Float(-1.0);
        assert!(Float::new().non_negative().validate(&mut negative).is_err());
        let mut positive = Value::Float(1.0);
        assert!(Float::new().negative().validate(&mut positive).is_err());
    }

    #[test]
    fn rejects_wrong_type_and_unparseable_string() {
        let mut list = Value::List(Vec::new());
        let error = Float::new().validate(&mut list).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Type { .. }));

        let mut text = Value::String("not-a-number".into());
        let error = Float::new().validate(&mut text).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::NotConvertible { .. }));
    }
}
