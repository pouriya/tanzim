use tanzim_validate::{ErrorKind, Float, Validator};
use tanzim_value::Value;

#[test]
fn accepts_float() {
    let mut value = Value::Float(1.5);
    assert!(Float::new().validate(&mut value).is_ok());
}

#[test]
fn coerces_integer() {
    let mut value = Value::Int(7);
    Float::new().validate(&mut value).unwrap();
    assert_eq!(value, Value::Float(7.0));
}

#[test]
fn coerces_string() {
    let mut value = Value::String("1.5".into());
    Float::new().validate(&mut value).unwrap();
    assert_eq!(value, Value::Float(1.5));
}

#[test]
fn enforces_range() {
    let mut value = Value::Float(-0.1);
    let error = Float::new()
        .range(0.0, 1.0)
        .validate(&mut value)
        .unwrap_err();
    assert!(matches!(error.kind, ErrorKind::BelowMin { .. }));

    let mut high = Value::Float(2.0);
    let error = Float::new()
        .range(0.0, 1.0)
        .validate(&mut high)
        .unwrap_err();
    assert!(matches!(error.kind, ErrorKind::AboveMax { .. }));
}

#[test]
fn enforces_sign_constraints() {
    let mut zero = Value::Float(0.0);
    assert!(Float::new().positive().validate(&mut zero).is_err());
    let mut negative = Value::Float(-1.0);
    assert!(Float::new().non_negative().validate(&mut negative).is_err());
    let mut positive = Value::Float(1.0);
    assert!(Float::new().negative().validate(&mut positive).is_err());
}

#[test]
fn rejects_wrong_type_and_unparseable_string() {
    let mut list = Value::List(Vec::new());
    let error = Float::new().validate(&mut list).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Type { .. }));

    let mut text = Value::String("not-a-number".into());
    let error = Float::new().validate(&mut text).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::NotConvertible { .. }));
}
