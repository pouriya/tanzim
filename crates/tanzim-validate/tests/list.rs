use tanzim_validate::{ErrorKind, Integer, List, Validator};
use tanzim_value::{LocatedValue, Location, Value};

fn item(value: Value) -> LocatedValue {
    LocatedValue::new(value, Location::at("file", "test", Some(1), Some(1), None))
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
    assert_eq!(*value.as_list().unwrap()[0].value(), Value::Int(5));
}
