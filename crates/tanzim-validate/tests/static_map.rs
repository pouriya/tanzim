use tanzim_validate::{ErrorKind, Integer, StaticMap, Str, Validator};
use tanzim_value::{LocatedValue, Location, Map, Value};

fn entry(value: Value) -> LocatedValue {
    LocatedValue::new(value, Location::at("file", "test", Some(1), Some(1), None))
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
