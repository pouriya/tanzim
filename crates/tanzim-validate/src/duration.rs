use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{LocatedValue, Location, Map, Value, ValueType};

/// (`duration` feature) Accepts an integer (whole seconds), a float (seconds + fraction),
/// or a human duration string (e.g. `"30s"`, `"5m"`, `"1h30m"`).
///
/// Default output is serde's native [`std::time::Duration`] shape: a map with `secs` and
/// `nanos`. Opt into other forms with the usual meta converters:
///
/// - [`Duration::to_int`] — whole seconds; errors if there is a sub-second component
/// - [`Duration::to_string`] — formats with `humantime` (e.g. `"1h 30m"`), not a bare
///   integer string
///
/// ```
/// # #[cfg(feature = "duration")]
/// # {
/// use tanzim_validate::{Duration, Validator};
/// use tanzim_value::Value;
///
/// let mut value = Value::String("1h30m".into());
/// Duration::new().validate(&mut value).unwrap();
/// let map = value.as_map().unwrap();
/// assert_eq!(map.get("secs").unwrap().value().as_int(), Some(5400));
/// assert_eq!(map.get("nanos").unwrap().value().as_int(), Some(0));
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Duration {
    meta: Meta,
}

impl Duration {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// A new, unconfigured `Duration` validator.
    pub fn new() -> Self {
        Self::default()
    }
}

crate::impl_meta_methods!(Duration);

impl Validator for Duration {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let input_type = value.type_name();
        let parsed = match value {
            Value::Int(secs) => {
                let secs = match u64::try_from(*secs) {
                    Ok(secs) => secs,
                    Err(_) => {
                        return Err(Error::new(ErrorKind::Format {
                            expected: "duration",
                        }));
                    }
                };
                std::time::Duration::from_secs(secs)
            }
            Value::Float(secs) => match std::time::Duration::try_from_secs_f64(*secs) {
                Ok(parsed) => parsed,
                Err(_) => {
                    return Err(Error::new(ErrorKind::Format {
                        expected: "duration",
                    }));
                }
            },
            Value::String(text) => match humantime::parse_duration(text) {
                Ok(parsed) => parsed,
                Err(_) => {
                    return Err(Error::new(ErrorKind::Format {
                        expected: "duration",
                    }));
                }
            },
            _ => {
                return Err(Error::new(ErrorKind::Format {
                    expected: "duration",
                }));
            }
        };

        match self.meta().convert {
            Some(ValueType::Int) => {
                if parsed.subsec_nanos() != 0 {
                    return Err(Error::new(ErrorKind::NotConvertible {
                        target: ValueType::Int,
                        found: input_type,
                    }));
                }
                let secs = match isize::try_from(parsed.as_secs()) {
                    Ok(secs) => secs,
                    Err(_) => {
                        return Err(Error::new(ErrorKind::NotConvertible {
                            target: ValueType::Int,
                            found: input_type,
                        }));
                    }
                };
                *value = Value::Int(secs);
            }
            Some(ValueType::String) => {
                // Emit humantime form here so the generic post-check cast does not stringify
                // a bare integer (e.g. "5400") instead of "1h30m".
                *value = Value::String(humantime::format_duration(parsed).to_string());
            }
            _ => {
                let secs = match isize::try_from(parsed.as_secs()) {
                    Ok(secs) => secs,
                    Err(_) => {
                        return Err(Error::new(ErrorKind::NotConvertible {
                            target: ValueType::Int,
                            found: input_type,
                        }));
                    }
                };
                let nanos = match isize::try_from(parsed.subsec_nanos()) {
                    Ok(nanos) => nanos,
                    Err(_) => {
                        return Err(Error::new(ErrorKind::NotConvertible {
                            target: ValueType::Int,
                            found: input_type,
                        }));
                    }
                };
                let location = Location::at("duration", "", None, None, None);
                let mut map = Map::new();
                map.insert(
                    "secs".to_string(),
                    LocatedValue::new(Value::Int(secs), location.clone()),
                );
                map.insert(
                    "nanos".to_string(),
                    LocatedValue::new(Value::Int(nanos), location),
                );
                *value = Value::Map(map);
            }
        }
        Ok(())
    }
}
