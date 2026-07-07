use tanzim_validate::{ByteSize, Validator};
use tanzim_value::Value;

#[test]
fn coerces_to_bytes() {
    let mut value = Value::String("10 KB".into());
    ByteSize::new().validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(10_000));
}

#[test]
fn rejects_garbage() {
    let mut value = Value::String("lots".into());
    assert!(ByteSize::new().validate(&mut value).is_err());
}
