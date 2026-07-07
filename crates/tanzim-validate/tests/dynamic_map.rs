use tanzim_validate::{DynamicMap, ErrorKind, Integer, Validator};
use tanzim_value::{LocatedValue, Location, Map, Value};

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
