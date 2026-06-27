use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

/// Accepts a list, with optional length bounds, a uniqueness check, and an optional
/// per-item validator.
///
/// Coercion: an empty map becomes an empty list (matching formats that render an empty
/// collection as `{}`). A non-empty map or any other type is rejected.
#[derive(Default)]
pub struct List {
    min_len: Option<usize>,
    max_len: Option<usize>,
    unique: bool,
    items: Option<Box<dyn Validator>>,
}

impl List {
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

    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Validate every item with `validator`.
    pub fn items(mut self, validator: impl Into<Box<dyn Validator>>) -> Self {
        self.items = Some(validator.into());
        self
    }
}

impl Validator for List {
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
        match value {
            Value::List(_) => {}
            Value::Map(map) if map.is_empty() => *value = Value::new_list(),
            _ => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::List,
                    found: value.type_name(),
                }));
            }
        }

        let items = match value.list_mut() {
            Some(items) => items,
            None => unreachable!("value coerced to a list above"),
        };

        let length = items.len();
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

        if let Some(validator) = &self.items {
            for (index, item) in items.iter_mut().enumerate() {
                match validator.validate(&mut item.value) {
                    Ok(()) => {}
                    Err(error) => return Err(error.under_index(index, &item.location)),
                }
            }
        }

        if self.unique {
            let mut seen: Vec<&Value> = Vec::new();
            for (index, item) in items.iter().enumerate() {
                for previous in &seen {
                    if **previous == item.value {
                        return Err(Error::new(ErrorKind::Duplicate { index })
                            .with_location(&item.location));
                    }
                }
                seen.push(&item.value);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Integer;
    use tanzim_value::{LocatedValue, Location};

    fn item(value: Value) -> LocatedValue {
        LocatedValue {
            value,
            location: Location::at("file", "test", Some(1), Some(1), None),
        }
    }

    #[test]
    fn empty_map_becomes_empty_list() {
        let mut value = Value::new_map();
        List::new().validate(&mut value).unwrap();
        assert_eq!(value, Value::new_list());
    }

    #[test]
    fn enforces_length_bounds() {
        let mut value = Value::List(vec![item(Value::Int(1))]);
        let error = List::new().min_len(2).validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::TooShort { .. }));
    }

    #[test]
    fn detects_duplicates() {
        let mut value = Value::List(vec![item(Value::Int(1)), item(Value::Int(1))]);
        let error = List::new().unique().validate(&mut value).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Duplicate { index: 1 }));
    }

    #[test]
    fn item_validator_reports_index_path() {
        let mut value = Value::List(vec![item(Value::Int(1)), item(Value::String("x".into()))]);
        let error = List::new()
            .items(Integer::new())
            .validate(&mut value)
            .unwrap_err();
        assert_eq!(error.path.len(), 1);
        assert!(matches!(error.kind, ErrorKind::NotConvertible { .. }));
    }

    #[test]
    fn item_coercion_persists() {
        let mut value = Value::List(vec![item(Value::String("5".into()))]);
        List::new()
            .items(Integer::new())
            .validate(&mut value)
            .unwrap();
        assert_eq!(value.as_list().unwrap()[0].value, Value::Int(5));
    }
}
