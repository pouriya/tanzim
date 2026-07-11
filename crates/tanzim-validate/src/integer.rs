use crate::error::{Error, ErrorKind};
use crate::number::{Sign, check_sign};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// Convert an integral `f64` into an `isize`, or `None` if it has a fraction or
/// falls outside the representable range.
fn f64_to_isize(number: f64) -> Option<isize> {
    if number.fract() != 0.0 {
        return None;
    }
    if number < isize::MIN as f64 || number > isize::MAX as f64 {
        return None;
    }
    Some(number as isize)
}

/// (`integer` feature) Accepts an integer, with optional inclusive bounds and lenient coercion.
///
/// Coercion:
/// - an integer stays as-is;
/// - a string is parsed as an integer, or as an integral float (e.g. `"3.0"`);
/// - a float with no fractional part (e.g. `3.0`) becomes an integer.
#[derive(Debug, Clone, Default)]
pub struct Integer {
    meta: Meta,
    min: Option<isize>,
    max: Option<isize>,
    sign: Option<Sign>,
}

impl Integer {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// A new, unconfigured `Integer` validator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Require the value to be at least `min`.
    pub fn min(mut self, min: isize) -> Self {
        self.min = Some(min);
        self
    }

    /// Require the value to be at most `max`.
    pub fn max(mut self, max: isize) -> Self {
        self.max = Some(max);
        self
    }

    /// Require the value to fall within the inclusive `[start, end]` range.
    pub fn range(mut self, start: isize, end: isize) -> Self {
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

crate::impl_meta_methods!(Integer);

impl Validator for Integer {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let coerced = match value {
            Value::Int(number) => *number,
            Value::Float(number) => match f64_to_isize(*number) {
                Some(number) => number,
                None => {
                    return Err(Error::new(ErrorKind::NotConvertible {
                        target: ValueType::Int,
                        found: ValueType::Float,
                    }));
                }
            },
            Value::String(text) => {
                if let Ok(number) = text.parse::<isize>() {
                    number
                } else if let Ok(number) = text.parse::<f64>() {
                    match f64_to_isize(number) {
                        Some(number) => number,
                        None => {
                            return Err(Error::new(ErrorKind::NotConvertible {
                                target: ValueType::Int,
                                found: ValueType::String,
                            }));
                        }
                    }
                } else {
                    return Err(Error::new(ErrorKind::NotConvertible {
                        target: ValueType::Int,
                        found: ValueType::String,
                    }));
                }
            }
            other => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::Int,
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

        check_sign(self.sign, coerced as f64)?;

        *value = Value::Int(coerced);
        Ok(())
    }
}
