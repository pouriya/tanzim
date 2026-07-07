use tanzim_validate::{Duration, Validator};
use tanzim_value::Value;

#[test]
fn coerces_to_seconds() {
    let mut value = Value::String("1h30m".into());
    Duration::new().validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(5400));
}

#[test]
fn coerces_to_millis() {
    let mut value = Value::String("250ms".into());
    Duration::new().millis().validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(250));
}

#[test]
fn rejects_garbage() {
    let mut value = Value::String("soon".into());
    assert!(Duration::new().validate(&mut value).is_err());
}
