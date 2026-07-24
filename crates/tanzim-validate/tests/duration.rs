use serde::Deserialize;
use std::time::Duration as StdDuration;
use tanzim_validate::{Duration, Validator};
use tanzim_value::{Value, ValueType};

#[test]
fn coerces_string_to_secs_nanos_map() {
    let mut value = Value::String("1h30m".into());
    Duration::new().validate(&mut value).unwrap();
    let map = value.as_map().unwrap();
    assert_eq!(map.get("secs").unwrap().value().as_int(), Some(5400));
    assert_eq!(map.get("nanos").unwrap().value().as_int(), Some(0));
}

#[test]
fn coerces_int_seconds() {
    let mut value = Value::Int(5400);
    Duration::new().validate(&mut value).unwrap();
    let map = value.as_map().unwrap();
    assert_eq!(map.get("secs").unwrap().value().as_int(), Some(5400));
    assert_eq!(map.get("nanos").unwrap().value().as_int(), Some(0));
}

#[test]
fn coerces_float_seconds() {
    let mut value = Value::Float(1.5);
    Duration::new().validate(&mut value).unwrap();
    let map = value.as_map().unwrap();
    assert_eq!(map.get("secs").unwrap().value().as_int(), Some(1));
    assert_eq!(
        map.get("nanos").unwrap().value().as_int(),
        Some(500_000_000)
    );
}

#[test]
fn preserves_subsecond_nanos() {
    let mut value = Value::String("250ms".into());
    Duration::new().validate(&mut value).unwrap();
    let map = value.as_map().unwrap();
    assert_eq!(map.get("secs").unwrap().value().as_int(), Some(0));
    assert_eq!(
        map.get("nanos").unwrap().value().as_int(),
        Some(250_000_000)
    );
}

#[test]
fn deserializes_to_std_duration() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Cfg {
        timeout: StdDuration,
    }

    let mut timeout = Value::String("1h30m".into());
    Duration::new().validate(&mut timeout).unwrap();
    let mut map = tanzim_value::Map::new();
    map.insert(
        "timeout".into(),
        tanzim_value::LocatedValue::new(
            timeout,
            tanzim_value::Location::at("test", "", None, None, None),
        ),
    );
    let cfg: Cfg = Value::Map(map).try_deserialize().unwrap();
    assert_eq!(cfg.timeout, StdDuration::from_secs(5400));
}

#[test]
fn to_int_whole_seconds() {
    let mut value = Value::String("1h30m".into());
    Duration::new().to_int().validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(5400));
}

#[test]
fn to_int_rejects_subseconds() {
    let mut value = Value::String("250ms".into());
    let error = Duration::new().to_int().validate(&mut value).unwrap_err();
    assert!(matches!(
        error.kind,
        tanzim_validate::ErrorKind::NotConvertible {
            target: ValueType::Int,
            found: ValueType::String,
        }
    ));
}

#[test]
fn to_int_rejects_fractional_float() {
    let mut value = Value::Float(1.5);
    let error = Duration::new().to_int().validate(&mut value).unwrap_err();
    assert!(matches!(
        error.kind,
        tanzim_validate::ErrorKind::NotConvertible {
            target: ValueType::Int,
            found: ValueType::Float,
        }
    ));
}

#[test]
fn to_string_formats_humantime() {
    let mut value = Value::Int(5400);
    Duration::new().to_string().validate(&mut value).unwrap();
    assert_eq!(value, Value::String("1h 30m".into()));
}

#[test]
fn rejects_garbage_string() {
    let mut value = Value::String("soon".into());
    assert!(Duration::new().validate(&mut value).is_err());
}

#[test]
fn rejects_negative_int() {
    let mut value = Value::Int(-1);
    assert!(Duration::new().validate(&mut value).is_err());
}

#[test]
fn rejects_bool() {
    let mut value = Value::Bool(true);
    assert!(Duration::new().validate(&mut value).is_err());
}
