use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// (`dynamic_map` feature) Accepts a map with arbitrary keys and uniform values.
///
/// Optional entry-count bounds and an optional validator applied to every value.
/// Coercion: an empty list becomes an empty map (the list counterpart of an empty
/// collection). A non-empty list or any other type is rejected.
#[derive(Default)]
pub struct DynamicMap {
    meta: Meta,
    min_len: Option<usize>,
    max_len: Option<usize>,
    values: Option<Box<dyn Validator>>,
}

impl DynamicMap {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn min_len(mut self, min: usize) -> Self {
        self.min_len = Some(min);
        self
    }

    pub fn max_len(mut self, max: usize) -> Self {
        self.max_len = Some(max);
        self
    }

    /// Validate every value with `validator`.
    pub fn values(mut self, validator: impl Into<Box<dyn Validator>>) -> Self {
        self.values = Some(validator.into());
        self
    }
}

impl Validator for DynamicMap {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        match value {
            Value::Map(_) => {}
            Value::List(list) if list.is_empty() => *value = Value::new_map(),
            _ => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::Map,
                    found: value.type_name(),
                }));
            }
        }

        let map = match value.map_mut() {
            Some(map) => map,
            None => unreachable!("value coerced to a map above"),
        };

        let length = map.len();
        if let Some(min) = self.min_len
            && length < min
        {
            return Err(Error::new(ErrorKind::TooShort { len: length, min }));
        }
        if let Some(max) = self.max_len
            && length > max
        {
            return Err(Error::new(ErrorKind::TooLong { len: length, max }));
        }

        if let Some(validator) = &self.values {
            for (key, entry) in map.entries_mut() {
                match validator.validate(entry.value_mut()) {
                    Ok(()) => {}
                    Err(error) => return Err(error.under_key(key, entry.location())),
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Integer;
    use tanzim_value::{LocatedValue, Location, Map};

    fn entry(value: Value) -> LocatedValue {
        LocatedValue::new(value, Location::at("file", "test", Some(1), Some(1), None))
    }

    #[test]
    fn empty_list_becomes_empty_map() {
        let mut value = Value::new_list();
        DynamicMap::new().validate(&mut value).unwrap();
        assert_eq!(value, Value::new_map());
    }

    #[test]
    fn enforces_count_bounds() {
        let mut map = Map::new();
        map.insert("a".into(), entry(Value::Int(1)));
        let mut value = Value::Map(map);
        let error = DynamicMap::new()
            .min_len(2)
            .validate(&mut value)
            .unwrap_err();
        assert!(matches!(error.kind, ErrorKind::TooShort { .. }));
    }

    #[test]
    fn value_validator_reports_key_path() {
        let mut map = Map::new();
        map.insert("a".into(), entry(Value::String("x".into())));
        let mut value = Value::Map(map);
        let error = DynamicMap::new()
            .values(Integer::new())
            .validate(&mut value)
            .unwrap_err();
        assert_eq!(error.path.len(), 1);
        assert!(matches!(error.kind, ErrorKind::NotConvertible { .. }));
    }
}
