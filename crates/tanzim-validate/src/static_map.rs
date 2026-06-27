use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

struct Field {
    key: String,
    required: bool,
    validator: Option<Box<dyn Validator>>,
}

/// Accepts a map with a known set of keys.
///
/// Each declared key is either required or optional, and may carry a validator for its
/// value. By default keys not declared in the schema are rejected; call
/// [`StaticMap::allow_unknown`] to permit them.
#[derive(Default)]
pub struct StaticMap {
    fields: Vec<Field>,
    deny_unknown: bool,
}

impl StaticMap {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            deny_unknown: true,
        }
    }

    /// Declare a required key whose value is validated by `validator`.
    pub fn required(
        mut self,
        key: impl Into<String>,
        validator: impl Into<Box<dyn Validator>>,
    ) -> Self {
        self.fields.push(Field {
            key: key.into(),
            required: true,
            validator: Some(validator.into()),
        });
        self
    }

    /// Declare an optional key whose value, when present, is validated by `validator`.
    pub fn optional(
        mut self,
        key: impl Into<String>,
        validator: impl Into<Box<dyn Validator>>,
    ) -> Self {
        self.fields.push(Field {
            key: key.into(),
            required: false,
            validator: Some(validator.into()),
        });
        self
    }

    /// Declare a required key whose value is accepted without validation.
    pub fn required_any(mut self, key: impl Into<String>) -> Self {
        self.fields.push(Field {
            key: key.into(),
            required: true,
            validator: None,
        });
        self
    }

    /// Declare an optional key whose value is accepted without validation.
    pub fn optional_any(mut self, key: impl Into<String>) -> Self {
        self.fields.push(Field {
            key: key.into(),
            required: false,
            validator: None,
        });
        self
    }

    /// Reject keys not declared in the schema (the default).
    pub fn deny_unknown(mut self) -> Self {
        self.deny_unknown = true;
        self
    }

    /// Accept keys not declared in the schema, leaving their values untouched.
    pub fn allow_unknown(mut self) -> Self {
        self.deny_unknown = false;
        self
    }
}

impl Validator for StaticMap {
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
        let map = match value.map_mut() {
            Some(map) => map,
            None => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::Map,
                    found: value.type_name(),
                }));
            }
        };

        for field in &self.fields {
            if field.required && !map.contains_key(&field.key) {
                return Err(Error::new(ErrorKind::MissingKey {
                    key: field.key.clone(),
                }));
            }
        }

        for field in &self.fields {
            if let Some(validator) = &field.validator
                && let Some(entry) = map.get_mut(&field.key)
            {
                match validator.validate(&mut entry.value) {
                    Ok(()) => {}
                    Err(error) => return Err(error.under_key(&field.key, &entry.location)),
                }
            }
        }

        if self.deny_unknown {
            for (key, entry) in map.entries() {
                let mut declared = false;
                for field in &self.fields {
                    if &field.key == key {
                        declared = true;
                        break;
                    }
                }
                if !declared {
                    return Err(Error::new(ErrorKind::UnknownKey { key: key.clone() })
                        .with_location(&entry.location));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Integer, Str};
    use tanzim_value::{LocatedValue, Location, Map};

    fn entry(value: Value) -> LocatedValue {
        LocatedValue {
            value,
            location: Location::at("file", "test", Some(1), Some(1), None),
        }
    }

    fn map_of(pairs: &[(&str, Value)]) -> Value {
        let mut map = Map::new();
        for (key, value) in pairs {
            map.insert((*key).to_string(), entry(value.clone()));
        }
        Value::Map(map)
    }

    #[test]
    fn missing_required_key_fails() {
        let schema = StaticMap::new().required("host", Str::new());
        let mut value = map_of(&[]);
        let error = schema.validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::MissingKey { .. }));
    }

    #[test]
    fn optional_absent_is_ok() {
        let schema = StaticMap::new().optional("port", Integer::new());
        let mut value = map_of(&[]);
        assert!(schema.validate(&mut value).is_ok());
    }

    #[test]
    fn value_validator_reports_key_path() {
        let schema = StaticMap::new().required("port", Integer::new());
        let mut value = map_of(&[("port", Value::String("x".into()))]);
        let error = schema.validate(&mut value).unwrap_err();
        assert_eq!(error.path.len(), 1);
        assert!(matches!(error.kind, ErrorKind::NotConvertible { .. }));
    }

    #[test]
    fn unknown_key_denied_by_default() {
        let schema = StaticMap::new().required("host", Str::new());
        let mut value = map_of(&[
            ("host", Value::String("h".into())),
            ("extra", Value::Int(1)),
        ]);
        let error = schema.validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::UnknownKey { .. }));
    }

    #[test]
    fn unknown_key_allowed_when_opted_in() {
        let schema = StaticMap::new()
            .required("host", Str::new())
            .allow_unknown();
        let mut value = map_of(&[
            ("host", Value::String("h".into())),
            ("extra", Value::Int(1)),
        ]);
        assert!(schema.validate(&mut value).is_ok());
    }
}
