use tanzim_validate::{ErrorKind, Integer, Validator};
use tanzim_value::Value;

#[test]
fn accepts_integer_in_range() {
    let mut value = Value::Int(50);
    assert!(Integer::new().range(0, 100).validate(&mut value).is_ok());
}

#[test]
fn rejects_out_of_range() {
    let mut value = Value::Int(200);
    let error = Integer::new().max(100).validate(&mut value).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::AboveMax { .. }));
}

#[test]
fn coerces_integer_string() {
    let mut value = Value::String("42".into());
    Integer::new().validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(42));
}

#[test]
fn coerces_integral_float_string() {
    let mut value = Value::String("3.0".into());
    Integer::new().validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(3));
}

#[test]
fn coerces_integral_float() {
    let mut value = Value::Float(7.0);
    Integer::new().validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(7));
}

#[test]
fn rejects_fractional_float() {
    let mut value = Value::Float(3.5);
    let error = Integer::new().validate(&mut value).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::NotConvertible { .. }));
}

#[test]
fn rejects_non_numeric_string() {
    let mut value = Value::String("abc".into());
    let error = Integer::new().validate(&mut value).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::NotConvertible { .. }));
}
